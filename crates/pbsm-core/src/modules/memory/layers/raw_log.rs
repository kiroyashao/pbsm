use crate::modules::memory::error::Result;
use crate::modules::memory::storage::sled_kv::SledKvStore;
use crate::modules::memory::storage::sqlite::SqliteStorage;
use crate::modules::memory::types::{
    LogType, MemoryEntry, MemoryLayer, RawLogEntry, SourceReference, WriteLogResult,
};
use chrono::Utc;
use serde_json;
use std::sync::Arc;
use uuid::Uuid;

pub struct RawLogLayer {
    sqlite: Arc<SqliteStorage>,
    sled: Arc<SledKvStore>,
}

impl RawLogLayer {
    pub fn new(sqlite: Arc<SqliteStorage>, sled: Arc<SledKvStore>) -> Self {
        Self { sqlite, sled }
    }

    pub async fn write_log(
        &self,
        session_id: &str,
        log_type: LogType,
        payload: serde_json::Value,
        topic: &str,
        confidence: Option<f64>,
    ) -> Result<WriteLogResult> {
        let log_id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().timestamp_millis();

        let entry = RawLogEntry {
            log_id: log_id.clone(),
            session_id: session_id.to_string(),
            log_type,
            timestamp,
            sequence_number: 0,
            payload,
            references: serde_json::Value::Null,
        };

        let json = serde_json::to_vec(&entry)?;
        self.sled.insert_raw_log(session_id, &log_id, &json)?;

        let confidence_value = confidence.unwrap_or(0.0);
        self.sqlite
            .insert_memory_index(&log_id, "RAW_LOG", topic, confidence_value, timestamp)?;

        Ok(WriteLogResult { log_id, timestamp })
    }

    pub async fn query_by_session(&self, session_id: &str) -> Result<Vec<RawLogEntry>> {
        let pairs = self.sled.scan_logs_by_session(session_id)?;
        let mut entries = Vec::with_capacity(pairs.len());
        for (_key, data) in pairs {
            let entry: RawLogEntry = serde_json::from_slice(&data)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    pub async fn query_by_topic(
        &self,
        topic: &str,
        confidence_threshold: Option<f64>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let rows = self
            .sqlite
            .query_memory_index_by_topic(topic, confidence_threshold, limit)?;
        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            if row.layer != "RAW_LOG" {
                continue;
            }
            let memory_entry = MemoryEntry {
                entry_id: row.entry_id.clone(),
                layer: MemoryLayer::RawLog,
                memory_type: "raw_log".to_string(),
                relevance_score: row.confidence.unwrap_or(0.0),
                confidence: row.confidence.unwrap_or(0.0),
                summary: format!("Raw log entry for topic: {}", row.topic),
                content: serde_json::json!({
                    "topic": row.topic,
                    "layer": row.layer,
                    "created_at": row.created_at,
                }),
                source_references: vec![SourceReference {
                    ref_type: "raw_log".to_string(),
                    ref_id: row.entry_id.clone(),
                    ref_path: None,
                }],
                created_at: chrono::DateTime::from_timestamp_millis(row.created_at)
                    .unwrap_or_default(),
                access_count: row.access_count as usize,
            };
            entries.push(memory_entry);
        }
        Ok(entries)
    }

    pub async fn delete_log(&self, session_id: &str, log_id: &str) -> Result<bool> {
        let sled_removed = self.sled.remove_raw_log(session_id, log_id)?;
        let sqlite_deleted = self.sqlite.delete_memory_index(log_id)?;
        Ok(sled_removed || sqlite_deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Arc<SqliteStorage>, Arc<SledKvStore>) {
        let sqlite = SqliteStorage::open_in_memory().unwrap();
        sqlite.init_schema().unwrap();
        let sled = SledKvStore::open_temp().unwrap();
        (Arc::new(sqlite), Arc::new(sled))
    }

    #[tokio::test]
    async fn test_write_log() {
        let (sqlite, sled) = setup();
        let layer = RawLogLayer::new(sqlite, sled);

        let result = layer
            .write_log(
                "sess-001",
                LogType::Dialogue,
                serde_json::json!({"message": "hello"}),
                "greeting",
                Some(0.8),
            )
            .await
            .unwrap();

        assert!(!result.log_id.is_empty());
        assert!(result.timestamp > 0);
    }

    #[tokio::test]
    async fn test_query_by_session() {
        let (sqlite, sled) = setup();
        let layer = RawLogLayer::new(sqlite, sled);

        layer
            .write_log(
                "sess-001",
                LogType::Dialogue,
                serde_json::json!({"m": "a"}),
                "topic-a",
                None,
            )
            .await
            .unwrap();
        layer
            .write_log(
                "sess-001",
                LogType::ToolCall,
                serde_json::json!({"m": "b"}),
                "topic-b",
                None,
            )
            .await
            .unwrap();
        layer
            .write_log(
                "sess-002",
                LogType::SystemEvent,
                serde_json::json!({"m": "c"}),
                "topic-c",
                None,
            )
            .await
            .unwrap();

        let entries = layer.query_by_session("sess-001").await.unwrap();
        assert_eq!(entries.len(), 2);

        let entries = layer.query_by_session("sess-002").await.unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn test_query_by_topic() {
        let (sqlite, sled) = setup();
        let layer = RawLogLayer::new(sqlite, sled);

        layer
            .write_log(
                "sess-001",
                LogType::Dialogue,
                serde_json::json!({}),
                "greeting",
                Some(0.9),
            )
            .await
            .unwrap();
        layer
            .write_log(
                "sess-001",
                LogType::ToolCall,
                serde_json::json!({}),
                "greeting",
                Some(0.5),
            )
            .await
            .unwrap();

        let entries = layer
            .query_by_topic("greeting", Some(0.7), 10)
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);

        let entries = layer.query_by_topic("greeting", None, 10).await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_log() {
        let (sqlite, sled) = setup();
        let layer = RawLogLayer::new(sqlite, sled);

        let result = layer
            .write_log(
                "sess-001",
                LogType::Dialogue,
                serde_json::json!({}),
                "topic",
                None,
            )
            .await
            .unwrap();

        let deleted = layer.delete_log("sess-001", &result.log_id).await.unwrap();
        assert!(deleted);

        let deleted_again = layer.delete_log("sess-001", &result.log_id).await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_write_log_without_confidence() {
        let (sqlite, sled) = setup();
        let layer = RawLogLayer::new(sqlite, sled);

        let result = layer
            .write_log(
                "sess-001",
                LogType::BeliefUpdate,
                serde_json::json!({"key": "val"}),
                "belief",
                None,
            )
            .await
            .unwrap();

        assert!(!result.log_id.is_empty());
    }

    #[tokio::test]
    async fn test_query_by_session_empty() {
        let (sqlite, sled) = setup();
        let layer = RawLogLayer::new(sqlite, sled);

        let entries = layer.query_by_session("nonexistent").await.unwrap();
        assert!(entries.is_empty());
    }
}
