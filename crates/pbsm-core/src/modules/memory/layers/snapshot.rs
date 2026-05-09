use crate::modules::memory::config::MemoryConfig;
use crate::modules::memory::error::{MemoryError, Result};
use crate::modules::memory::storage::sled_kv::SledKvStore;
use crate::modules::memory::storage::sqlite::SqliteStorage;
use crate::modules::memory::types::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub struct SnapshotLayer {
    sqlite: Arc<SqliteStorage>,
    sled: Arc<SledKvStore>,
    config: MemoryConfig,
}

impl SnapshotLayer {
    pub fn new(sqlite: Arc<SqliteStorage>, sled: Arc<SledKvStore>, config: MemoryConfig) -> Self {
        Self {
            sqlite,
            sled,
            config,
        }
    }

    pub async fn write_snapshot(
        &self,
        metadata: SnapshotMetadata,
        belief_state: BeliefState,
        intention_state: IntentionState,
        attention_state: AttentionState,
        compression: CompressionType,
    ) -> Result<WriteSnapshotResult> {
        let start = std::time::Instant::now();

        let snapshot = FullSnapshot {
            metadata: metadata.clone(),
            belief_state,
            intention_state,
            attention_state,
            memory_index: serde_json::Value::Null,
        };

        let json = serde_json::to_vec(&snapshot)?;
        let file_size = json.len();

        let compressed = Self::compress(&json, compression)?;
        let compressed_size = compressed.len();

        let checksum = Self::compute_checksum(&compressed);

        let file_path = self
            .config
            .storage_path
            .join("snapshots")
            .join(format!("{}.bin", metadata.snapshot_id))
            .to_string_lossy()
            .to_string();

        self.sled
            .insert_snapshot(&metadata.snapshot_id, &compressed)?;

        self.sqlite.insert_snapshot_meta(
            &metadata.snapshot_id,
            &metadata.session_id,
            metadata.created_at,
            serde_json::to_string(&metadata.snapshot_type)
                .unwrap_or_default()
                .trim_matches('"'),
            &file_path,
            compressed_size as i64,
            &checksum,
            snapshot.belief_state.nodes.len() as i32,
            snapshot.belief_state.edges.len() as i32,
            &metadata.trigger_description,
        )?;

        let duration_ms = start.elapsed().as_millis() as i64;
        let compression_ratio = if file_size > 0 {
            compressed_size as f64 / file_size as f64
        } else {
            1.0
        };

        Ok(WriteSnapshotResult {
            snapshot_id: metadata.snapshot_id,
            file_path,
            file_size,
            compressed_size,
            checksum,
            compression_ratio,
            node_count: snapshot.belief_state.nodes.len(),
            edge_count: snapshot.belief_state.edges.len(),
            write_duration_ms: duration_ms,
        })
    }

    pub async fn restore_snapshot(
        &self,
        snapshot_id: &str,
        validate_checksum: bool,
    ) -> Result<RestoreSnapshotResult> {
        let start = std::time::Instant::now();

        let compressed = self
            .sled
            .get_snapshot(snapshot_id)?
            .ok_or_else(|| MemoryError::SnapshotNotFound(snapshot_id.to_string()))?;

        if validate_checksum {
            let actual_checksum = Self::compute_checksum(&compressed);
            let meta = self.sqlite.get_snapshot_meta(snapshot_id)?;
            if let Some(row) = meta {
                if row.checksum != actual_checksum {
                    return Err(MemoryError::ChecksumMismatch {
                        expected: row.checksum,
                        actual: actual_checksum,
                    });
                }
            }
        }

        let json = Self::decompress(&compressed)?;
        let snapshot: FullSnapshot = serde_json::from_slice(&json)?;

        let duration_ms = start.elapsed().as_millis() as i64;

        Ok(RestoreSnapshotResult {
            snapshot,
            restored: true,
            duration_ms,
        })
    }

    pub async fn list_snapshots(&self, session_id: Option<&str>) -> Result<Vec<SnapshotMetadata>> {
        let rows = match session_id {
            Some(sid) => self.sqlite.query_snapshots_by_session(sid)?,
            None => {
                let conn = self.sqlite.get_connection();
                let conn = conn.lock();
                let mut stmt = conn
                    .prepare("SELECT snapshot_id, session_id, created_at, snapshot_type, file_path, file_size, checksum, node_count, edge_count, parent_snapshot_id, trigger_description FROM snapshots ORDER BY created_at DESC")
                    .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
                let row_iter = stmt
                    .query_map([], |row| {
                        Ok(crate::modules::memory::storage::sqlite::SnapshotRow {
                            snapshot_id: row.get(0)?,
                            session_id: row.get(1)?,
                            created_at: row.get(2)?,
                            snapshot_type: row.get(3)?,
                            file_path: row.get(4)?,
                            file_size: row.get(5)?,
                            checksum: row.get(6)?,
                            node_count: row.get(7)?,
                            edge_count: row.get(8)?,
                            parent_snapshot_id: row.get(9)?,
                            trigger_description: row.get(10)?,
                        })
                    })
                    .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
                let mut result = Vec::new();
                for row in row_iter {
                    result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
                }
                result
            }
        };

        let mut snapshots = Vec::with_capacity(rows.len());
        for row in rows {
            let snapshot_type: SnapshotType =
                serde_json::from_str(&format!("\"{}\"", row.snapshot_type))
                    .unwrap_or(SnapshotType::Manual);
            snapshots.push(SnapshotMetadata {
                snapshot_id: row.snapshot_id,
                session_id: row.session_id,
                version: "1.0".to_string(),
                snapshot_type,
                agent_id: String::new(),
                trigger_description: row.trigger_description.unwrap_or_default(),
                created_at: row.created_at,
            });
        }
        Ok(snapshots)
    }

    pub async fn delete_snapshot(&self, snapshot_id: &str) -> Result<bool> {
        let sled_removed = self.sled.remove_snapshot(snapshot_id)?;
        let sqlite_deleted = self.sqlite.delete_snapshot(snapshot_id)?;
        Ok(sled_removed || sqlite_deleted)
    }

    fn compress(data: &[u8], compression: CompressionType) -> Result<Vec<u8>> {
        match compression {
            CompressionType::None => Ok(data.to_vec()),
            CompressionType::Lz4 => Ok(lz4_flex::compress_prepend_size(data)),
            CompressionType::Zstd => zstd::encode_all(data, 0).map_err(|e| {
                MemoryError::CompressionFailed(format!("ZSTD compression failed: {e}"))
            }),
        }
    }

    fn decompress(data: &[u8]) -> Result<Vec<u8>> {
        if serde_json::from_slice::<serde_json::Value>(data).is_ok() {
            return Ok(data.to_vec());
        }

        if let Ok(decompressed) = lz4_flex::decompress_size_prepended(data) {
            return Ok(decompressed);
        }

        zstd::decode_all(data)
            .map_err(|e| MemoryError::CompressionFailed(format!("Decompression failed: {e}")))
    }

    fn compute_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn setup() -> (Arc<SqliteStorage>, Arc<SledKvStore>, MemoryConfig) {
        let sqlite = SqliteStorage::open_in_memory().unwrap();
        sqlite.init_schema().unwrap();
        let sled = SledKvStore::open_temp().unwrap();
        let config = MemoryConfig::default();
        (Arc::new(sqlite), Arc::new(sled), config)
    }

    fn make_metadata(snapshot_id: &str, session_id: &str) -> SnapshotMetadata {
        SnapshotMetadata {
            snapshot_id: snapshot_id.to_string(),
            session_id: session_id.to_string(),
            version: "1.0".to_string(),
            snapshot_type: SnapshotType::Manual,
            agent_id: "agent-1".to_string(),
            trigger_description: "test snapshot".to_string(),
            created_at: Utc::now().timestamp_millis(),
        }
    }

    fn make_belief_state() -> BeliefState {
        BeliefState {
            nodes: vec![],
            edges: vec![],
            active_predictions: vec![],
            unresolved_residuals: vec![],
        }
    }

    fn make_intention_state() -> IntentionState {
        IntentionState {
            stack: vec![],
            active_goal_pointer: 0,
            execution_depth: 0,
        }
    }

    fn make_attention_state() -> AttentionState {
        AttentionState {
            parameter: 0.5,
            mode: AttentionMode::Moderate,
            focus_areas: vec![],
        }
    }

    #[tokio::test]
    async fn test_write_snapshot_no_compression() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        let result = layer
            .write_snapshot(
                make_metadata("snap-001", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::None,
            )
            .await
            .unwrap();

        assert_eq!(result.snapshot_id, "snap-001");
        assert!(!result.checksum.is_empty());
        assert!(result.compression_ratio > 0.0);
    }

    #[tokio::test]
    async fn test_write_snapshot_lz4() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        let result = layer
            .write_snapshot(
                make_metadata("snap-002", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::Lz4,
            )
            .await
            .unwrap();

        assert_eq!(result.snapshot_id, "snap-002");
        assert!(result.compressed_size > 0);
    }

    #[tokio::test]
    async fn test_write_snapshot_zstd() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        let result = layer
            .write_snapshot(
                make_metadata("snap-003", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::Zstd,
            )
            .await
            .unwrap();

        assert_eq!(result.snapshot_id, "snap-003");
        assert!(result.compressed_size > 0);
    }

    #[tokio::test]
    async fn test_restore_snapshot_with_checksum_validation() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        layer
            .write_snapshot(
                make_metadata("snap-010", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::None,
            )
            .await
            .unwrap();

        let result = layer.restore_snapshot("snap-010", true).await.unwrap();
        assert!(result.restored);
        assert_eq!(result.snapshot.metadata.snapshot_id, "snap-010");
    }

    #[tokio::test]
    async fn test_restore_snapshot_without_checksum() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        layer
            .write_snapshot(
                make_metadata("snap-011", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::Lz4,
            )
            .await
            .unwrap();

        let result = layer.restore_snapshot("snap-011", false).await.unwrap();
        assert!(result.restored);
    }

    #[tokio::test]
    async fn test_restore_snapshot_not_found() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        let result = layer.restore_snapshot("nonexistent", false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_snapshots_by_session() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        layer
            .write_snapshot(
                make_metadata("snap-020", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::None,
            )
            .await
            .unwrap();
        layer
            .write_snapshot(
                make_metadata("snap-021", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::None,
            )
            .await
            .unwrap();
        layer
            .write_snapshot(
                make_metadata("snap-022", "sess-002"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::None,
            )
            .await
            .unwrap();

        let snapshots = layer.list_snapshots(Some("sess-001")).await.unwrap();
        assert_eq!(snapshots.len(), 2);

        let snapshots = layer.list_snapshots(Some("sess-002")).await.unwrap();
        assert_eq!(snapshots.len(), 1);
    }

    #[tokio::test]
    async fn test_list_all_snapshots() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        layer
            .write_snapshot(
                make_metadata("snap-030", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::None,
            )
            .await
            .unwrap();
        layer
            .write_snapshot(
                make_metadata("snap-031", "sess-002"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::None,
            )
            .await
            .unwrap();

        let snapshots = layer.list_snapshots(None).await.unwrap();
        assert_eq!(snapshots.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_snapshot() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        layer
            .write_snapshot(
                make_metadata("snap-040", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::None,
            )
            .await
            .unwrap();

        let deleted = layer.delete_snapshot("snap-040").await.unwrap();
        assert!(deleted);

        let deleted_again = layer.delete_snapshot("snap-040").await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_write_snapshot_duration() {
        let (sqlite, sled, config) = setup();
        let layer = SnapshotLayer::new(sqlite, sled, config);

        let result = layer
            .write_snapshot(
                make_metadata("snap-050", "sess-001"),
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                CompressionType::Lz4,
            )
            .await
            .unwrap();

        assert!(result.write_duration_ms >= 0);
    }
}
