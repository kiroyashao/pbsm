use std::sync::Arc;

use crate::modules::memory::config::MemoryConfig;
use crate::modules::memory::error::{MemoryError, Result};
use crate::modules::memory::storage::sled_kv::SledKvStore;
use crate::modules::memory::storage::sqlite::SqliteStorage;
use crate::modules::memory::types::*;
use chrono::Utc;
use uuid::Uuid;

pub struct CleanupEngine {
    sqlite: Arc<SqliteStorage>,
    sled: Arc<SledKvStore>,
    config: MemoryConfig,
}

struct CleanupStats {
    scanned: usize,
    deleted: usize,
    archived: usize,
    freed_bytes: usize,
}

impl CleanupEngine {
    pub fn new(sqlite: Arc<SqliteStorage>, sled: Arc<SledKvStore>, config: MemoryConfig) -> Self {
        Self {
            sqlite,
            sled,
            config,
        }
    }

    pub fn calculate_cleanup_score(
        &self,
        age_days: f64,
        access_count: usize,
        importance: f64,
        relevance: f64,
        has_residual_dependency: bool,
    ) -> f64 {
        let w_age = 0.2;
        let w_freq = 0.15;
        let w_importance = 0.25;
        let w_relevance = 0.3;

        let age_score = (age_days / 90.0).min(1.0);
        let freq_score = if access_count == 0 {
            1.0
        } else {
            1.0 / (1.0 + (access_count as f64).ln())
        };
        let importance_score = 1.0 - importance;
        let relevance_score = 1.0 - relevance;
        let residual_penalty = if has_residual_dependency { 10.0 } else { 0.0 };

        w_age * age_score
            + w_freq * freq_score
            + w_importance * importance_score
            + w_relevance * relevance_score
            + residual_penalty
    }

    pub async fn cleanup_expired(&self, policy: CleanupPolicy) -> Result<CleanupResult> {
        let cleanup_id = Uuid::new_v4().to_string();
        let start_time = Utc::now().timestamp_millis();

        let max_age_days = policy.max_age_days.unwrap_or(self.config.max_log_age_days);
        let cutoff_time = Utc::now().timestamp_millis() - (max_age_days as i64 * 86_400_000);

        let mut total_scanned = 0usize;
        let mut total_deleted = 0usize;
        let mut total_archived = 0usize;
        let mut total_freed_bytes = 0usize;
        let errors: Vec<CleanupError> = Vec::new();

        match policy.scope {
            CleanupScope::RawLogOnly => {
                let stats = self.cleanup_raw_logs(cutoff_time, &policy).await?;
                total_scanned += stats.scanned;
                total_deleted += stats.deleted;
                total_archived += stats.archived;
                total_freed_bytes += stats.freed_bytes;
            }
            CleanupScope::SnapshotOnly => {
                let stats = self.cleanup_snapshots(cutoff_time, &policy).await?;
                total_scanned += stats.scanned;
                total_deleted += stats.deleted;
                total_archived += stats.archived;
                total_freed_bytes += stats.freed_bytes;
            }
            CleanupScope::ExperienceOnly => {
                let stats = self.cleanup_experiences(cutoff_time, &policy).await?;
                total_scanned += stats.scanned;
                total_deleted += stats.deleted;
                total_archived += stats.archived;
                total_freed_bytes += stats.freed_bytes;
            }
            CleanupScope::AllLayers | CleanupScope::AllLayersPlusDeep => {
                let raw_stats = self.cleanup_raw_logs(cutoff_time, &policy).await?;
                total_scanned += raw_stats.scanned;
                total_deleted += raw_stats.deleted;
                total_archived += raw_stats.archived;
                total_freed_bytes += raw_stats.freed_bytes;

                let snap_stats = self.cleanup_snapshots(cutoff_time, &policy).await?;
                total_scanned += snap_stats.scanned;
                total_deleted += snap_stats.deleted;
                total_archived += snap_stats.archived;
                total_freed_bytes += snap_stats.freed_bytes;

                let exp_stats = self.cleanup_experiences(cutoff_time, &policy).await?;
                total_scanned += exp_stats.scanned;
                total_deleted += exp_stats.deleted;
                total_archived += exp_stats.archived;
                total_freed_bytes += exp_stats.freed_bytes;
            }
        }

        let end_time = Utc::now().timestamp_millis();
        let duration_ms = end_time - start_time;

        Ok(CleanupResult {
            cleanup_id,
            cleanup_type: policy.cleanup_type,
            scope: policy.scope,
            status: CleanupStatus::Completed,
            statistics: CleanupStatistics {
                scanned_entries: total_scanned,
                deleted_entries: total_deleted,
                archived_entries: total_archived,
                freed_space_bytes: total_freed_bytes,
                execution_duration_ms: duration_ms,
            },
            errors,
            start_time,
            end_time: Some(end_time),
        })
    }

    async fn cleanup_raw_logs(
        &self,
        cutoff_time: i64,
        policy: &CleanupPolicy,
    ) -> Result<CleanupStats> {
        #[allow(clippy::type_complexity)]
        let entries: Vec<(String, Option<f64>, i64, Option<i64>, i32)> = {
            let conn = self.sqlite.get_connection();
            let conn_guard = conn.lock();

            let mut stmt = conn_guard
                .prepare(
                    "SELECT entry_id, confidence, created_at, last_accessed, access_count \
                     FROM memory_index WHERE layer = 'RAW_LOG' AND created_at < ?1",
                )
                .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

            let rows = stmt
                .query_map(rusqlite::params![cutoff_time], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<f64>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, i32>(4)?,
                    ))
                })
                .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

            let mut result = Vec::new();
            for row in rows {
                result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
            }
            result
        };

        let mut stats = CleanupStats {
            scanned: entries.len(),
            deleted: 0,
            archived: 0,
            freed_bytes: 0,
        };

        let now_ms = Utc::now().timestamp_millis();

        for (entry_id, confidence, created_at, _last_accessed, access_count) in &entries {
            let age_days = (now_ms - *created_at) as f64 / 86_400_000.0;
            let importance = confidence.unwrap_or(0.5);
            let relevance = confidence.unwrap_or(0.5);

            let score = self.calculate_cleanup_score(
                age_days,
                *access_count as usize,
                importance,
                relevance,
                false,
            );

            if score > 0.5 {
                if !policy.dry_run {
                    self.sqlite.delete_memory_index(entry_id)?;
                }
                stats.deleted += 1;
                stats.freed_bytes += 256;
            }
        }

        Ok(stats)
    }

    async fn cleanup_snapshots(
        &self,
        cutoff_time: i64,
        policy: &CleanupPolicy,
    ) -> Result<CleanupStats> {
        let entries: Vec<(String, String, i64, i64)> = {
            let conn = self.sqlite.get_connection();
            let conn_guard = conn.lock();

            let mut stmt = conn_guard
                .prepare(
                    "SELECT snapshot_id, snapshot_type, created_at, file_size \
                     FROM snapshots WHERE created_at < ?1 AND snapshot_type != 'ERROR_RECOVERY'",
                )
                .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

            let rows = stmt
                .query_map(rusqlite::params![cutoff_time], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

            let mut result = Vec::new();
            for row in rows {
                result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
            }
            result
        };

        let mut stats = CleanupStats {
            scanned: entries.len(),
            deleted: 0,
            archived: 0,
            freed_bytes: 0,
        };

        let now_ms = Utc::now().timestamp_millis();
        let sixty_days_ms: i64 = 60 * 86_400_000;

        for (snapshot_id, _snapshot_type, created_at, file_size) in &entries {
            let age_ms = now_ms - *created_at;
            if age_ms > sixty_days_ms {
                if !policy.dry_run {
                    self.sqlite.delete_snapshot(snapshot_id)?;
                    let _ = self.sled.remove_snapshot(snapshot_id);
                }
                stats.deleted += 1;
                stats.freed_bytes += *file_size as usize;
            }
        }

        Ok(stats)
    }

    async fn cleanup_experiences(
        &self,
        cutoff_time: i64,
        policy: &CleanupPolicy,
    ) -> Result<CleanupStats> {
        let entries: Vec<(String, f64, i64, Option<i64>, i32)> = {
            let conn = self.sqlite.get_connection();
            let conn_guard = conn.lock();

            let mut stmt = conn_guard
                .prepare(
                    "SELECT experience_id, confidence, created_at, last_used_at, verification_count \
                     FROM experiences WHERE (verification_count < 2 OR last_used_at < ?1)",
                )
                .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

            let rows = stmt
                .query_map(rusqlite::params![cutoff_time], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, f64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, i32>(4)?,
                    ))
                })
                .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;

            let mut result = Vec::new();
            for row in rows {
                result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
            }
            result
        };

        let mut stats = CleanupStats {
            scanned: entries.len(),
            deleted: 0,
            archived: 0,
            freed_bytes: 0,
        };

        let now_ms = Utc::now().timestamp_millis();

        for (experience_id, confidence, created_at, last_used_at, verification_count) in &entries {
            let age_days = (now_ms - *created_at) as f64 / 86_400_000.0;
            let last_used_age_days = last_used_at
                .map(|t| (now_ms - t) as f64 / 86_400_000.0)
                .unwrap_or(age_days);

            if *confidence < 0.3 && last_used_age_days > 90.0 {
                if !policy.dry_run {
                    self.sqlite.delete_experience(experience_id)?;
                    let _ = self.sled.remove_experience(experience_id);
                }
                stats.deleted += 1;
                stats.freed_bytes += 512;
            } else if *verification_count < 2
                || last_used_age_days > self.config.archive_threshold_days as f64
            {
                stats.archived += 1;
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_engine() -> (CleanupEngine, Arc<SqliteStorage>, Arc<SledKvStore>) {
        let sqlite = SqliteStorage::open_in_memory().unwrap();
        sqlite.init_schema().unwrap();
        let sled_store = SledKvStore::open_temp().unwrap();
        let sqlite_arc = Arc::new(sqlite);
        let sled_arc = Arc::new(sled_store);
        let config = MemoryConfig::default();
        let engine = CleanupEngine::new(Arc::clone(&sqlite_arc), Arc::clone(&sled_arc), config);
        (engine, sqlite_arc, sled_arc)
    }

    fn days_ago_ms(days: i64) -> i64 {
        Utc::now().timestamp_millis() - days * 86_400_000
    }

    #[test]
    fn test_calculate_cleanup_score_fresh_high_importance() {
        let (engine, _, _) = setup_engine();
        let score = engine.calculate_cleanup_score(0.0, 0, 0.9, 0.9, false);
        let expected = 0.2 * 0.0 + 0.15 * 1.0 + 0.25 * 0.1 + 0.3 * 0.1;
        assert!((score - expected).abs() < 1e-10);
        assert!(score < 0.5);
    }

    #[test]
    fn test_calculate_cleanup_score_old_unaccessed_low_value() {
        let (engine, _, _) = setup_engine();
        let score = engine.calculate_cleanup_score(90.0, 0, 0.1, 0.1, false);
        let expected = 0.2 * 1.0 + 0.15 * 1.0 + 0.25 * 0.9 + 0.3 * 0.9;
        assert!((score - expected).abs() < 1e-10);
        assert!(score > 0.5);
    }

    #[test]
    fn test_calculate_cleanup_score_with_residual_dependency() {
        let (engine, _, _) = setup_engine();
        let score = engine.calculate_cleanup_score(0.0, 10, 0.9, 0.9, true);
        let base = 0.2 * 0.0 + 0.15 * (1.0 / (1.0 + 10f64.ln())) + 0.25 * 0.1 + 0.3 * 0.1;
        let expected = base + 10.0;
        assert!((score - expected).abs() < 1e-10);
        assert!(score > 10.0);
    }

    #[test]
    fn test_calculate_cleanup_score_frequently_accessed() {
        let (engine, _, _) = setup_engine();
        let score_low = engine.calculate_cleanup_score(45.0, 1, 0.5, 0.5, false);
        let score_high = engine.calculate_cleanup_score(45.0, 100, 0.5, 0.5, false);
        assert!(score_high < score_low);
    }

    #[test]
    fn test_calculate_cleanup_score_age_capped_at_90() {
        let (engine, _, _) = setup_engine();
        let score_90 = engine.calculate_cleanup_score(90.0, 0, 0.5, 0.5, false);
        let score_180 = engine.calculate_cleanup_score(180.0, 0, 0.5, 0.5, false);
        assert!((score_90 - score_180).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_cleanup_score_zero_access_count() {
        let (engine, _, _) = setup_engine();
        let score = engine.calculate_cleanup_score(30.0, 0, 0.5, 0.5, false);
        let freq_component = 0.15 * 1.0;
        let other = 0.2 * (30.0 / 90.0) + 0.25 * 0.5 + 0.3 * 0.5;
        let expected = other + freq_component;
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_cleanup_score_high_importance_reduces_score() {
        let (engine, _, _) = setup_engine();
        let score_low = engine.calculate_cleanup_score(45.0, 5, 0.1, 0.5, false);
        let score_high = engine.calculate_cleanup_score(45.0, 5, 0.9, 0.5, false);
        assert!(score_high < score_low);
    }

    #[test]
    fn test_calculate_cleanup_score_high_relevance_reduces_score() {
        let (engine, _, _) = setup_engine();
        let score_low = engine.calculate_cleanup_score(45.0, 5, 0.5, 0.1, false);
        let score_high = engine.calculate_cleanup_score(45.0, 5, 0.5, 0.9, false);
        assert!(score_high < score_low);
    }

    #[test]
    fn test_calculate_cleanup_score_no_residual() {
        let (engine, _, _) = setup_engine();
        let score = engine.calculate_cleanup_score(45.0, 3, 0.5, 0.5, false);
        assert!(score < 1.0);
    }

    #[test]
    fn test_calculate_cleanup_score_medium_profile() {
        let (engine, _, _) = setup_engine();
        let score = engine.calculate_cleanup_score(45.0, 10, 0.5, 0.5, false);
        let age_score = 45.0 / 90.0;
        let freq_score = 1.0 / (1.0 + 10f64.ln());
        let expected = 0.2 * age_score + 0.15 * freq_score + 0.25 * 0.5 + 0.3 * 0.5;
        assert!((score - expected).abs() < 1e-10);
        assert!(score < 0.5);
    }

    #[tokio::test]
    async fn test_cleanup_expired_dry_run_raw_log_only() {
        let (engine, sqlite, _sled) = setup_engine();

        sqlite
            .insert_memory_index("log-001", "RAW_LOG", "topic-a", 0.1, days_ago_ms(200))
            .unwrap();
        sqlite
            .insert_memory_index("log-002", "RAW_LOG", "topic-b", 0.8, days_ago_ms(200))
            .unwrap();
        sqlite
            .insert_memory_index("log-003", "SNAPSHOT", "topic-c", 0.5, days_ago_ms(200))
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::RawLogOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: true,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.status, CleanupStatus::Completed);
        assert_eq!(result.scope, CleanupScope::RawLogOnly);
        assert_eq!(result.statistics.scanned_entries, 2);
        assert!(result.statistics.deleted_entries > 0);
        assert!(result.end_time.is_some());
        assert!(!result.cleanup_id.is_empty());

        assert!(sqlite.get_snapshot_meta("log-001").unwrap().is_none());
        let rows = sqlite
            .query_memory_index_by_topic("topic-a", None, 10)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entry_id, "log-001");
    }

    #[tokio::test]
    async fn test_cleanup_expired_dry_run_snapshot_only() {
        let (engine, sqlite, sled_store) = setup_engine();

        sqlite
            .insert_snapshot_meta(
                "snap-001",
                "sess-001",
                days_ago_ms(120),
                "AUTOMATIC",
                "/data/snap-001.bin",
                4096,
                "abc",
                10,
                5,
                "auto",
            )
            .unwrap();
        sled_store
            .insert_snapshot("snap-001", b"snapshot data")
            .unwrap();

        sqlite
            .insert_snapshot_meta(
                "snap-002",
                "sess-001",
                days_ago_ms(120),
                "ERROR_RECOVERY",
                "/data/snap-002.bin",
                2048,
                "def",
                5,
                2,
                "error",
            )
            .unwrap();
        sled_store
            .insert_snapshot("snap-002", b"recovery data")
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::SnapshotOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: true,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.status, CleanupStatus::Completed);
        assert_eq!(result.statistics.scanned_entries, 1);
        assert_eq!(result.statistics.deleted_entries, 1);

        assert!(sqlite.get_snapshot_meta("snap-001").unwrap().is_some());
        assert!(sled_store.get_snapshot("snap-001").unwrap().is_some());
        assert!(sqlite.get_snapshot_meta("snap-002").unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cleanup_expired_dry_run_experience_only() {
        let (engine, sqlite, sled_store) = setup_engine();

        sqlite
            .insert_experience(
                "exp-001",
                "nav",
                "recovery",
                0.1,
                days_ago_ms(200),
                r#"{"action":"retry"}"#,
            )
            .unwrap();
        sled_store
            .insert_experience("exp-001", b"experience data")
            .unwrap();

        sqlite
            .insert_experience(
                "exp-002",
                "nav",
                "recovery",
                0.8,
                days_ago_ms(200),
                r#"{"action":"cache"}"#,
            )
            .unwrap();
        sled_store
            .insert_experience("exp-002", b"experience data 2")
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::ExperienceOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: true,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.status, CleanupStatus::Completed);
        assert!(result.statistics.scanned_entries >= 1);

        let rows = sqlite.query_experiences_by_domain("nav", None).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(sled_store.get_experience("exp-001").unwrap().is_some());
        assert!(sled_store.get_experience("exp-002").unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cleanup_expired_dry_run_all_layers() {
        let (engine, sqlite, sled_store) = setup_engine();

        sqlite
            .insert_memory_index("log-001", "RAW_LOG", "topic-a", 0.1, days_ago_ms(200))
            .unwrap();

        sqlite
            .insert_snapshot_meta(
                "snap-001",
                "sess-001",
                days_ago_ms(120),
                "AUTOMATIC",
                "/data/snap.bin",
                4096,
                "abc",
                10,
                5,
                "auto",
            )
            .unwrap();
        sled_store.insert_snapshot("snap-001", b"data").unwrap();

        sqlite
            .insert_experience("exp-001", "nav", "recovery", 0.1, days_ago_ms(200), "{}")
            .unwrap();
        sled_store
            .insert_experience("exp-001", b"exp data")
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Aggressive,
            scope: CleanupScope::AllLayers,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: true,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.status, CleanupStatus::Completed);
        assert_eq!(result.scope, CleanupScope::AllLayers);
        assert!(result.statistics.scanned_entries >= 3);

        let log_rows = sqlite
            .query_memory_index_by_topic("topic-a", None, 10)
            .unwrap();
        assert_eq!(log_rows.len(), 1);

        assert!(sqlite.get_snapshot_meta("snap-001").unwrap().is_some());
        assert!(sled_store.get_snapshot("snap-001").unwrap().is_some());

        let exp_rows = sqlite.query_experiences_by_domain("nav", None).unwrap();
        assert_eq!(exp_rows.len(), 1);
        assert!(sled_store.get_experience("exp-001").unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cleanup_expired_actual_deletion_raw_log() {
        let (engine, sqlite, _sled) = setup_engine();

        sqlite
            .insert_memory_index("log-001", "RAW_LOG", "topic-a", 0.1, days_ago_ms(200))
            .unwrap();
        sqlite
            .insert_memory_index("log-002", "RAW_LOG", "topic-b", 0.9, days_ago_ms(200))
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::RawLogOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: false,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.status, CleanupStatus::Completed);
        assert!(result.statistics.deleted_entries >= 1);
        assert!(result.statistics.freed_space_bytes > 0);

        let low_rows = sqlite
            .query_memory_index_by_topic("topic-a", None, 10)
            .unwrap();
        assert!(low_rows.is_empty());

        let high_rows = sqlite
            .query_memory_index_by_topic("topic-b", None, 10)
            .unwrap();
        assert_eq!(high_rows.len(), 1);
    }

    #[tokio::test]
    async fn test_cleanup_expired_actual_deletion_snapshot() {
        let (engine, sqlite, sled_store) = setup_engine();

        sqlite
            .insert_snapshot_meta(
                "snap-old",
                "sess-001",
                days_ago_ms(120),
                "AUTOMATIC",
                "/data/old.bin",
                4096,
                "abc",
                10,
                5,
                "auto",
            )
            .unwrap();
        sled_store.insert_snapshot("snap-old", b"old data").unwrap();

        sqlite
            .insert_snapshot_meta(
                "snap-recent",
                "sess-001",
                days_ago_ms(30),
                "AUTOMATIC",
                "/data/recent.bin",
                2048,
                "def",
                5,
                2,
                "auto",
            )
            .unwrap();
        sled_store
            .insert_snapshot("snap-recent", b"recent data")
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::SnapshotOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: false,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.status, CleanupStatus::Completed);
        assert_eq!(result.statistics.deleted_entries, 1);
        assert_eq!(result.statistics.freed_space_bytes, 4096);

        assert!(sqlite.get_snapshot_meta("snap-old").unwrap().is_none());
        assert!(sled_store.get_snapshot("snap-old").unwrap().is_none());

        assert!(sqlite.get_snapshot_meta("snap-recent").unwrap().is_some());
        assert!(sled_store.get_snapshot("snap-recent").unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cleanup_expired_error_recovery_snapshot_preserved() {
        let (engine, sqlite, sled_store) = setup_engine();

        sqlite
            .insert_snapshot_meta(
                "snap-err",
                "sess-001",
                days_ago_ms(120),
                "ERROR_RECOVERY",
                "/data/err.bin",
                8192,
                "xyz",
                20,
                10,
                "error",
            )
            .unwrap();
        sled_store
            .insert_snapshot("snap-err", b"error recovery data")
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Aggressive,
            scope: CleanupScope::SnapshotOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: false,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.statistics.scanned_entries, 0);
        assert_eq!(result.statistics.deleted_entries, 0);

        assert!(sqlite.get_snapshot_meta("snap-err").unwrap().is_some());
        assert!(sled_store.get_snapshot("snap-err").unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cleanup_expired_experience_archive_vs_delete() {
        let (engine, sqlite, sled_store) = setup_engine();

        sqlite
            .insert_experience(
                "exp-low-conf",
                "nav",
                "recovery",
                0.1,
                days_ago_ms(200),
                "{}",
            )
            .unwrap();
        sled_store
            .insert_experience("exp-low-conf", b"low confidence data")
            .unwrap();

        sqlite
            .insert_experience(
                "exp-med-conf",
                "nav",
                "recovery",
                0.6,
                days_ago_ms(200),
                "{}",
            )
            .unwrap();
        sled_store
            .insert_experience("exp-med-conf", b"medium confidence data")
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::ExperienceOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: false,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert!(result.statistics.deleted_entries >= 1);
        assert!(result.statistics.archived_entries >= 1);

        assert!(sled_store.get_experience("exp-low-conf").unwrap().is_none());
        assert!(sled_store.get_experience("exp-med-conf").unwrap().is_some());
    }

    #[tokio::test]
    async fn test_cleanup_expired_no_expired_entries() {
        let (engine, sqlite, _sled) = setup_engine();

        sqlite
            .insert_memory_index("log-recent", "RAW_LOG", "topic-a", 0.5, days_ago_ms(1))
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::RawLogOnly,
            max_age_days: Some(90),
            min_importance: None,
            dry_run: false,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.statistics.scanned_entries, 0);
        assert_eq!(result.statistics.deleted_entries, 0);
        assert_eq!(result.statistics.freed_space_bytes, 0);

        let rows = sqlite
            .query_memory_index_by_topic("topic-a", None, 10)
            .unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn test_cleanup_expired_uses_config_default_age() {
        let (engine, sqlite, _sled) = setup_engine();

        sqlite
            .insert_memory_index("log-old", "RAW_LOG", "topic-a", 0.1, days_ago_ms(100))
            .unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::RawLogOnly,
            max_age_days: None,
            min_importance: None,
            dry_run: false,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert!(result.statistics.scanned_entries >= 1);
    }

    #[tokio::test]
    async fn test_cleanup_expired_all_layers_plus_deep() {
        let (engine, sqlite, sled_store) = setup_engine();

        sqlite
            .insert_memory_index("log-001", "RAW_LOG", "topic-a", 0.1, days_ago_ms(200))
            .unwrap();
        sqlite
            .insert_snapshot_meta(
                "snap-001",
                "sess-001",
                days_ago_ms(120),
                "MANUAL",
                "/data/snap.bin",
                2048,
                "abc",
                5,
                2,
                "manual",
            )
            .unwrap();
        sled_store.insert_snapshot("snap-001", b"data").unwrap();
        sqlite
            .insert_experience("exp-001", "nav", "recovery", 0.1, days_ago_ms(200), "{}")
            .unwrap();
        sled_store.insert_experience("exp-001", b"exp").unwrap();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Aggressive,
            scope: CleanupScope::AllLayersPlusDeep,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: true,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert_eq!(result.scope, CleanupScope::AllLayersPlusDeep);
        assert!(result.statistics.scanned_entries >= 3);

        let log_rows = sqlite
            .query_memory_index_by_topic("topic-a", None, 10)
            .unwrap();
        assert_eq!(log_rows.len(), 1);
        assert!(sqlite.get_snapshot_meta("snap-001").unwrap().is_some());
        let exp_rows = sqlite.query_experiences_by_domain("nav", None).unwrap();
        assert_eq!(exp_rows.len(), 1);
    }

    #[tokio::test]
    async fn test_cleanup_expired_result_fields() {
        let (engine, _sqlite, _sled) = setup_engine();

        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Manual,
            scope: CleanupScope::RawLogOnly,
            max_age_days: Some(7),
            min_importance: None,
            dry_run: true,
        };

        let result = engine.cleanup_expired(policy).await.unwrap();

        assert!(!result.cleanup_id.is_empty());
        assert_eq!(result.cleanup_type, CleanupType::Manual);
        assert_eq!(result.scope, CleanupScope::RawLogOnly);
        assert_eq!(result.status, CleanupStatus::Completed);
        assert!(result.start_time > 0);
        assert!(result.end_time.is_some());
        assert!(result.end_time.unwrap() >= result.start_time);
        assert!(result.errors.is_empty());
    }
}
