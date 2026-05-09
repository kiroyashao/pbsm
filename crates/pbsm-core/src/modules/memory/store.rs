use crate::modules::memory::cleanup::CleanupEngine;
use crate::modules::memory::config::MemoryConfig;
use crate::modules::memory::error::{MemoryError, Result};
use crate::modules::memory::events::{
    create_cleanup_completed_event, create_experience_created_event,
    create_retrieval_completed_event, create_snapshot_created_event,
    create_snapshot_restored_event, MemoryEventPublisher, NullMemoryEventPublisher,
};
use crate::modules::memory::layers::experience::ExperienceLayer;
use crate::modules::memory::layers::raw_log::RawLogLayer;
use crate::modules::memory::layers::snapshot::SnapshotLayer;
use crate::modules::memory::retrieval::by_context::ContextRetriever;
use crate::modules::memory::retrieval::by_problem::ProblemRetriever;
use crate::modules::memory::retrieval::by_topic::TopicRetriever;
use crate::modules::memory::storage::sled_kv::{SledConfig, SledKvStore};
use crate::modules::memory::storage::sqlite::SqliteStorage;
use crate::modules::memory::storage::transaction::TransactionManager;
use crate::modules::memory::types::*;
use std::sync::Arc;

#[allow(dead_code)]
pub struct ExternalMemoryStore {
    sqlite: Arc<SqliteStorage>,
    sled: Arc<SledKvStore>,
    transaction_manager: Arc<TransactionManager>,
    raw_log_layer: Arc<RawLogLayer>,
    snapshot_layer: Arc<SnapshotLayer>,
    experience_layer: Arc<ExperienceLayer>,
    topic_retriever: Arc<TopicRetriever>,
    context_retriever: Arc<ContextRetriever>,
    problem_retriever: Arc<ProblemRetriever>,
    cleanup_engine: Arc<CleanupEngine>,
    event_publisher: Arc<dyn MemoryEventPublisher>,
    config: MemoryConfig,
}

impl ExternalMemoryStore {
    pub async fn open(config: MemoryConfig) -> Result<Self> {
        Self::open_with_publisher(config, Arc::new(NullMemoryEventPublisher)).await
    }

    pub async fn open_with_publisher(
        config: MemoryConfig,
        publisher: Arc<dyn MemoryEventPublisher>,
    ) -> Result<Self> {
        std::fs::create_dir_all(&config.storage_path).map_err(|e| {
            MemoryError::StorageOpen(format!("failed to create storage directory: {e}"))
        })?;

        let db_path = config.storage_path.join("memory.db");
        let sqlite = SqliteStorage::open(&db_path)?;
        sqlite.init_schema()?;
        let sqlite = Arc::new(sqlite);

        let kv_path = config.storage_path.join("kv");
        let sled = SledKvStore::open(&kv_path, SledConfig::default())?;
        let sled = Arc::new(sled);

        let transaction_manager = Arc::new(TransactionManager::new(&sqlite));

        let raw_log_layer = Arc::new(RawLogLayer::new(Arc::clone(&sqlite), Arc::clone(&sled)));

        let snapshot_layer = Arc::new(SnapshotLayer::new(
            Arc::clone(&sqlite),
            Arc::clone(&sled),
            config.clone(),
        ));

        let experience_layer =
            Arc::new(ExperienceLayer::new(Arc::clone(&sqlite), Arc::clone(&sled)));

        let topic_retriever = Arc::new(TopicRetriever::new(
            Arc::clone(&raw_log_layer),
            Arc::clone(&snapshot_layer),
            Arc::clone(&experience_layer),
        ));

        let context_retriever = Arc::new(ContextRetriever::new(
            Arc::clone(&experience_layer),
            Arc::clone(&snapshot_layer),
        ));

        let problem_retriever = Arc::new(ProblemRetriever::new(Arc::clone(&experience_layer)));

        let cleanup_engine = Arc::new(CleanupEngine::new(
            Arc::clone(&sqlite),
            Arc::clone(&sled),
            config.clone(),
        ));

        Ok(Self {
            sqlite,
            sled,
            transaction_manager,
            raw_log_layer,
            snapshot_layer,
            experience_layer,
            topic_retriever,
            context_retriever,
            problem_retriever,
            cleanup_engine,
            event_publisher: publisher,
            config,
        })
    }

    pub async fn write_snapshot(
        &self,
        session_id: &str,
        snapshot_type: SnapshotType,
        belief_state: BeliefState,
        intention_state: IntentionState,
        attention_state: AttentionState,
        trigger_description: &str,
    ) -> Result<WriteSnapshotResult> {
        let snapshot_id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().timestamp_millis();

        let metadata = SnapshotMetadata {
            snapshot_id,
            session_id: session_id.to_string(),
            version: "1.0".to_string(),
            snapshot_type,
            agent_id: String::new(),
            trigger_description: trigger_description.to_string(),
            created_at,
        };

        let result = self
            .snapshot_layer
            .write_snapshot(
                metadata,
                belief_state,
                intention_state,
                attention_state,
                self.config.compression_type,
            )
            .await?;

        let event = create_snapshot_created_event(
            &result.snapshot_id,
            &format!("{:?}", snapshot_type),
            result.compressed_size as u64,
        );
        let _ = self.event_publisher.publish(event);

        Ok(result)
    }

    pub async fn restore_snapshot(
        &self,
        snapshot_id: &str,
        validate_checksum: bool,
    ) -> Result<RestoreSnapshotResult> {
        let result = self
            .snapshot_layer
            .restore_snapshot(snapshot_id, validate_checksum)
            .await?;

        let event = create_snapshot_restored_event(snapshot_id, result.duration_ms as u64);
        let _ = self.event_publisher.publish(event);

        Ok(result)
    }

    pub async fn retrieve_by_topic(&self, query: MemoryQuery) -> Result<RetrievalResult> {
        let limit = self.config.retrieval_default_limit;
        let result = self
            .topic_retriever
            .retrieve(
                &query.topic,
                query.confidence_threshold,
                query.layer_filter,
                limit,
                0,
                query.include_raw_logs,
            )
            .await?;

        let event = create_retrieval_completed_event(
            "topic",
            result.total_matches,
            result.search_metadata.search_duration_ms as u64,
            result.search_metadata.cache_hit,
        );
        let _ = self.event_publisher.publish(event);

        Ok(result)
    }

    pub async fn retrieve_by_context(
        &self,
        current_beliefs: Vec<BeliefContext>,
        intent_description: &str,
        confidence_gaps: Option<Vec<ConfidenceGap>>,
        retrieval_depth: RetrievalDepth,
    ) -> Result<ContextualRetrievalResult> {
        self.context_retriever
            .retrieve(
                &current_beliefs,
                intent_description,
                confidence_gaps,
                retrieval_depth,
                self.config.retrieval_default_limit,
            )
            .await
    }

    pub async fn retrieve_for_problem(
        &self,
        problem_description: &str,
        problem_type: Option<ProblemType>,
    ) -> Result<ProblemRetrievalResult> {
        self.problem_retriever
            .retrieve(
                problem_description,
                problem_type,
                self.config.retrieval_default_limit,
            )
            .await
    }

    pub async fn write_raw_log(
        &self,
        session_id: &str,
        log_type: LogType,
        payload: serde_json::Value,
        topic: &str,
        confidence: Option<f64>,
    ) -> Result<WriteLogResult> {
        self.raw_log_layer
            .write_log(session_id, log_type, payload, topic, confidence)
            .await
    }

    pub async fn write_experience(
        &self,
        experience: Experience,
        auto_verify: bool,
    ) -> Result<WriteExperienceResult> {
        let pattern_type = format!("{:?}", experience.content.pattern);
        let result = self
            .experience_layer
            .write_experience(experience, auto_verify)
            .await?;

        let event = create_experience_created_event(&result.experience_id, &pattern_type, 0.0);
        let _ = self.event_publisher.publish(event);

        Ok(result)
    }

    pub async fn cleanup_expired(&self, policy: CleanupPolicy) -> Result<CleanupResult> {
        let result = self.cleanup_engine.cleanup_expired(policy).await?;

        let event = create_cleanup_completed_event(
            &format!("{:?}", result.cleanup_type),
            result.statistics.deleted_entries,
            result.statistics.archived_entries,
            result.statistics.freed_space_bytes as u64,
        );
        let _ = self.event_publisher.publish(event);

        Ok(result)
    }

    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        let conn = self.sqlite.get_connection();
        let conn = conn.lock();

        let raw_log_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_index WHERE layer = 'RAW_LOG'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

        let snapshot_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

        let experience_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM experiences", [], |row| row.get(0))
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

        let total_size: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(file_size), 0) FROM snapshots",
                [],
                |row| row.get(0),
            )
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

        Ok(StorageStats {
            total_entries: (raw_log_count + snapshot_count + experience_count) as usize,
            raw_log_count: raw_log_count as usize,
            snapshot_count: snapshot_count as usize,
            experience_count: experience_count as usize,
            total_size_bytes: total_size as usize,
        })
    }

    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }

    pub async fn close(&self) -> Result<()> {
        self.sled.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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

    fn make_experience(id: &str, domain: &str, pattern: PatternType) -> Experience {
        Experience {
            experience_id: id.to_string(),
            metadata: serde_json::json!({"created_by": "test"}),
            content: ExperienceContent {
                title: "Test Experience".to_string(),
                summary: "A test experience entry".to_string(),
                domain: domain.to_string(),
                pattern,
                context: serde_json::Value::Null,
                knowledge: serde_json::Value::Null,
                outcomes: serde_json::Value::Null,
            },
            usage_stats: serde_json::json!({"access_count": 0}),
            relationships: serde_json::json!({}),
        }
    }

    fn temp_config() -> MemoryConfig {
        let uid = uuid::Uuid::new_v4().to_string();
        let path = std::env::temp_dir().join(format!("pbsm_test_{uid}"));
        MemoryConfig {
            storage_path: path,
            ..MemoryConfig::default()
        }
    }

    #[tokio::test]
    async fn test_open_store() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config.clone()).await;
        assert!(store.is_ok(), "store should open successfully");

        let store = store.unwrap();
        assert_eq!(store.config().storage_path, config.storage_path);

        let close_result = store.close().await;
        assert!(close_result.is_ok());
    }

    #[tokio::test]
    async fn test_open_with_publisher() {
        let config = temp_config();
        let publisher: Arc<dyn MemoryEventPublisher> = Arc::new(NullMemoryEventPublisher);
        let store = ExternalMemoryStore::open_with_publisher(config, publisher).await;
        assert!(store.is_ok());

        let store = store.unwrap();
        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_and_restore_snapshot() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let result = store
            .write_snapshot(
                "sess-001",
                SnapshotType::Manual,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "test trigger",
            )
            .await
            .unwrap();

        assert!(!result.snapshot_id.is_empty());
        assert!(result.file_size > 0);
        assert!(result.compressed_size > 0);
        assert!(!result.checksum.is_empty());
        assert!(result.compression_ratio > 0.0);
        assert!(result.write_duration_ms >= 0);

        let restored = store
            .restore_snapshot(&result.snapshot_id, true)
            .await
            .unwrap();

        assert!(restored.restored);
        assert_eq!(restored.snapshot.metadata.snapshot_id, result.snapshot_id);
        assert!(restored.duration_ms >= 0);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_restore_snapshot_not_found() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let result = store.restore_snapshot("nonexistent-snapshot", false).await;
        assert!(result.is_err());

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_snapshot_automatic() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let result = store
            .write_snapshot(
                "sess-002",
                SnapshotType::Automatic,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "automatic trigger",
            )
            .await
            .unwrap();

        assert!(!result.snapshot_id.is_empty());

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_snapshot_error_recovery() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let result = store
            .write_snapshot(
                "sess-003",
                SnapshotType::ErrorRecovery,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "error recovery trigger",
            )
            .await
            .unwrap();

        assert!(!result.snapshot_id.is_empty());

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_raw_log() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let result = store
            .write_raw_log(
                "sess-001",
                LogType::Dialogue,
                serde_json::json!({"message": "hello world"}),
                "greeting",
                Some(0.85),
            )
            .await
            .unwrap();

        assert!(!result.log_id.is_empty());
        assert!(result.timestamp > 0);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_raw_log_without_confidence() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let result = store
            .write_raw_log(
                "sess-001",
                LogType::ToolCall,
                serde_json::json!({"tool": "search", "query": "test"}),
                "tool_usage",
                None,
            )
            .await
            .unwrap();

        assert!(!result.log_id.is_empty());

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_raw_log_multiple_types() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let log_types = vec![
            LogType::Dialogue,
            LogType::ToolCall,
            LogType::BeliefUpdate,
            LogType::ExecutionTrace,
            LogType::SystemEvent,
        ];

        for log_type in log_types {
            let result = store
                .write_raw_log(
                    "sess-multi",
                    log_type,
                    serde_json::json!({"type": format!("{:?}", log_type)}),
                    "multi_type",
                    Some(0.5),
                )
                .await
                .unwrap();
            assert!(!result.log_id.is_empty());
        }

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_experience() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let experience = make_experience("exp-001", "navigation", PatternType::ErrorHandling);
        let result = store.write_experience(experience, false).await.unwrap();

        assert_eq!(result.experience_id, "exp-001");
        assert!(!result.verified);
        assert!(result.timestamp > 0);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_experience_auto_verify() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let experience = make_experience("exp-002", "planning", PatternType::TaskPattern);
        let result = store.write_experience(experience, true).await.unwrap();

        assert_eq!(result.experience_id, "exp-002");
        assert!(result.verified);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_write_experience_multiple_patterns() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let patterns = vec![
            PatternType::ErrorHandling,
            PatternType::TaskPattern,
            PatternType::ToolSequence,
            PatternType::BeliefCorrection,
            PatternType::GoalDecomposition,
        ];

        for (i, pattern) in patterns.into_iter().enumerate() {
            let experience = make_experience(&format!("exp-pattern-{i}"), "testing", pattern);
            let result = store.write_experience(experience, false).await.unwrap();
            assert_eq!(result.experience_id, format!("exp-pattern-{i}"));
        }

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_cleanup_expired_dry_run() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::AllLayers,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: true,
        };

        let result = store.cleanup_expired(policy).await.unwrap();

        assert!(!result.cleanup_id.is_empty());
        assert_eq!(result.cleanup_type, CleanupType::Standard);
        assert_eq!(result.scope, CleanupScope::AllLayers);
        assert_eq!(result.status, CleanupStatus::Completed);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_cleanup_expired_raw_log_only() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        store
            .write_raw_log(
                "sess-001",
                LogType::Dialogue,
                serde_json::json!({"msg": "test"}),
                "test_topic",
                Some(0.5),
            )
            .await
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::RawLogOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: true,
        };

        let result = store.cleanup_expired(policy).await.unwrap();
        assert_eq!(result.status, CleanupStatus::Completed);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_get_storage_stats_empty() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let stats = store.get_storage_stats().await.unwrap();

        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.raw_log_count, 0);
        assert_eq!(stats.snapshot_count, 0);
        assert_eq!(stats.experience_count, 0);
        assert_eq!(stats.total_size_bytes, 0);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_get_storage_stats_with_data() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        store
            .write_snapshot(
                "sess-001",
                SnapshotType::Manual,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "test",
            )
            .await
            .unwrap();

        store
            .write_raw_log(
                "sess-001",
                LogType::Dialogue,
                serde_json::json!({"msg": "hello"}),
                "greeting",
                Some(0.8),
            )
            .await
            .unwrap();

        store
            .write_raw_log(
                "sess-001",
                LogType::ToolCall,
                serde_json::json!({"tool": "search"}),
                "tool_usage",
                Some(0.7),
            )
            .await
            .unwrap();

        let experience = make_experience("exp-001", "testing", PatternType::ErrorHandling);
        store.write_experience(experience, false).await.unwrap();

        let stats = store.get_storage_stats().await.unwrap();

        assert_eq!(stats.snapshot_count, 1);
        assert_eq!(stats.raw_log_count, 2);
        assert_eq!(stats.experience_count, 1);
        assert_eq!(stats.total_entries, 4);
        assert!(stats.total_size_bytes > 0);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_config_getter() {
        let config = temp_config();
        let original_path = config.storage_path.clone();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let retrieved_config = store.config();
        assert_eq!(retrieved_config.storage_path, original_path);
        assert_eq!(retrieved_config.cache_size, 100);
        assert_eq!(retrieved_config.max_log_age_days, 90);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_close() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let result = store.close().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_snapshots_same_session() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let snap1 = store
            .write_snapshot(
                "sess-multi",
                SnapshotType::Manual,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "first snapshot",
            )
            .await
            .unwrap();

        let snap2 = store
            .write_snapshot(
                "sess-multi",
                SnapshotType::Automatic,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "second snapshot",
            )
            .await
            .unwrap();

        assert_ne!(snap1.snapshot_id, snap2.snapshot_id);

        let restored1 = store
            .restore_snapshot(&snap1.snapshot_id, false)
            .await
            .unwrap();
        assert!(restored1.restored);

        let restored2 = store
            .restore_snapshot(&snap2.snapshot_id, false)
            .await
            .unwrap();
        assert!(restored2.restored);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_snapshot_with_checksum_validation() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let result = store
            .write_snapshot(
                "sess-checksum",
                SnapshotType::Scheduled,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "checksum test",
            )
            .await
            .unwrap();

        let restored_with_validation = store
            .restore_snapshot(&result.snapshot_id, true)
            .await
            .unwrap();
        assert!(restored_with_validation.restored);

        let restored_without_validation = store
            .restore_snapshot(&result.snapshot_id, false)
            .await
            .unwrap();
        assert!(restored_without_validation.restored);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_stats_after_snapshot_write() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let stats_before = store.get_storage_stats().await.unwrap();
        assert_eq!(stats_before.snapshot_count, 0);

        store
            .write_snapshot(
                "sess-stats",
                SnapshotType::Manual,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "stats test",
            )
            .await
            .unwrap();

        let stats_after = store.get_storage_stats().await.unwrap();
        assert_eq!(stats_after.snapshot_count, 1);
        assert!(stats_after.total_size_bytes > 0);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_stats_after_experience_write() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let stats_before = store.get_storage_stats().await.unwrap();
        assert_eq!(stats_before.experience_count, 0);

        let experience = make_experience("exp-stats", "stats_domain", PatternType::TaskPattern);
        store.write_experience(experience, false).await.unwrap();

        let stats_after = store.get_storage_stats().await.unwrap();
        assert_eq!(stats_after.experience_count, 1);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_stats_after_raw_log_write() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let stats_before = store.get_storage_stats().await.unwrap();
        assert_eq!(stats_before.raw_log_count, 0);

        store
            .write_raw_log(
                "sess-stats",
                LogType::BeliefUpdate,
                serde_json::json!({"key": "value"}),
                "stats_topic",
                Some(0.9),
            )
            .await
            .unwrap();

        let stats_after = store.get_storage_stats().await.unwrap();
        assert_eq!(stats_after.raw_log_count, 1);

        let _ = store.close().await;
    }

    #[tokio::test]
    async fn test_full_workflow() {
        let config = temp_config();
        let store = ExternalMemoryStore::open(config).await.unwrap();

        let snap_result = store
            .write_snapshot(
                "sess-workflow",
                SnapshotType::Manual,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "workflow test snapshot",
            )
            .await
            .unwrap();

        let restored = store
            .restore_snapshot(&snap_result.snapshot_id, true)
            .await
            .unwrap();
        assert!(restored.restored);

        let log_result = store
            .write_raw_log(
                "sess-workflow",
                LogType::Dialogue,
                serde_json::json!({"step": "workflow test"}),
                "workflow",
                Some(0.75),
            )
            .await
            .unwrap();
        assert!(!log_result.log_id.is_empty());

        let experience = make_experience(
            "exp-workflow",
            "workflow_domain",
            PatternType::ErrorHandling,
        );
        let exp_result = store.write_experience(experience, true).await.unwrap();
        assert!(exp_result.verified);

        let stats = store.get_storage_stats().await.unwrap();
        assert_eq!(stats.snapshot_count, 1);
        assert_eq!(stats.raw_log_count, 1);
        assert_eq!(stats.experience_count, 1);
        assert_eq!(stats.total_entries, 3);

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::AllLayers,
            max_age_days: Some(365),
            min_importance: None,
            dry_run: true,
        };
        let cleanup_result = store.cleanup_expired(policy).await.unwrap();
        assert_eq!(cleanup_result.status, CleanupStatus::Completed);

        let close_result = store.close().await;
        assert!(close_result.is_ok());
    }
}
