use crate::modules::memory::error::Result;
use crate::modules::memory::storage::sled_kv::SledKvStore;
use crate::modules::memory::storage::sqlite::SqliteStorage;
use crate::modules::memory::types::*;
use chrono::Utc;
use serde_json;
use std::sync::Arc;

pub struct ExperienceLayer {
    sqlite: Arc<SqliteStorage>,
    sled: Arc<SledKvStore>,
}

impl ExperienceLayer {
    pub fn new(sqlite: Arc<SqliteStorage>, sled: Arc<SledKvStore>) -> Self {
        Self { sqlite, sled }
    }

    pub fn write_experience(
        &self,
        experience: Experience,
        auto_verify: bool,
    ) -> Result<WriteExperienceResult> {
        let experience_id = experience.experience_id.clone();
        let timestamp = Utc::now().timestamp_millis();

        let json = serde_json::to_vec(&experience)?;
        self.sled.insert_experience(&experience_id, &json)?;

        let content_json = serde_json::to_string(&experience.content)?;
        let pattern_type_str = serde_json::to_string(&experience.content.pattern)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();

        self.sqlite.insert_experience(
            &experience_id,
            &experience.content.domain,
            &pattern_type_str,
            experience.content.confidence,
            timestamp,
            &content_json,
        )?;

        let verified = if auto_verify {
            self.sqlite.update_experience_usage(&experience_id).is_ok()
        } else {
            false
        };

        Ok(WriteExperienceResult {
            experience_id,
            verified,
            timestamp,
        })
    }

    pub fn query_by_domain(
        &self,
        domain: &str,
        confidence_threshold: Option<f64>,
    ) -> Result<Vec<MemoryEntry>> {
        let rows = self
            .sqlite
            .query_experiences_by_domain(domain, confidence_threshold)?;
        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let sled_data = self.sled.get_experience(&row.experience_id)?;
            let content = match sled_data {
                Some(data) => {
                    let exp: Experience = serde_json::from_slice(&data)?;
                    serde_json::to_value(&exp.content).unwrap_or(serde_json::Value::Null)
                }
                None => serde_json::from_str(&row.content_json).unwrap_or(serde_json::Value::Null),
            };

            let memory_entry = MemoryEntry {
                entry_id: row.experience_id.clone(),
                layer: MemoryLayer::Experience,
                memory_type: "experience".to_string(),
                relevance_score: row.confidence,
                confidence: row.confidence,
                importance: 0.0,
                recency_score: 0.0,
                summary: format!("Experience in domain: {}", row.domain),
                content,
                source_references: vec![SourceReference {
                    ref_type: "experience".to_string(),
                    ref_id: row.experience_id.clone(),
                    ref_path: None,
                }],
                created_at: chrono::DateTime::from_timestamp_millis(row.created_at)
                    .unwrap_or_default(),
                access_count: row.verification_count as usize,
            };
            entries.push(memory_entry);
        }
        Ok(entries)
    }

    pub fn query_by_pattern_type(&self, pattern_type: &str) -> Result<Vec<MemoryEntry>> {
        let rows = self.sqlite.query_experiences_by_pattern(pattern_type)?;
        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let sled_data = self.sled.get_experience(&row.experience_id)?;
            let content = match sled_data {
                Some(data) => {
                    let exp: Experience = serde_json::from_slice(&data)?;
                    serde_json::to_value(&exp.content).unwrap_or(serde_json::Value::Null)
                }
                None => serde_json::from_str(&row.content_json).unwrap_or(serde_json::Value::Null),
            };

            let memory_entry = MemoryEntry {
                entry_id: row.experience_id.clone(),
                layer: MemoryLayer::Experience,
                memory_type: "experience".to_string(),
                relevance_score: row.confidence,
                confidence: row.confidence,
                importance: 0.0,
                recency_score: 0.0,
                summary: format!("Experience with pattern: {}", row.pattern_type),
                content,
                source_references: vec![SourceReference {
                    ref_type: "experience".to_string(),
                    ref_id: row.experience_id.clone(),
                    ref_path: None,
                }],
                created_at: chrono::DateTime::from_timestamp_millis(row.created_at)
                    .unwrap_or_default(),
                access_count: row.verification_count as usize,
            };
            entries.push(memory_entry);
        }
        Ok(entries)
    }

    pub fn update_usage(&self, experience_id: &str) -> Result<()> {
        self.sqlite.update_experience_usage(experience_id)?;
        Ok(())
    }

    pub fn delete_experience(&self, experience_id: &str) -> Result<bool> {
        let sled_removed = self.sled.remove_experience(experience_id)?;
        let sqlite_deleted = self.sqlite.delete_experience(experience_id)?;
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

    fn make_experience(id: &str, domain: &str, pattern: PatternType) -> Experience {
        Experience {
            experience_id: id.to_string(),
            metadata: ExperienceMetadata {
                source_type: "test".to_string(),
                source_snapshot_ids: None,
                source_log_ids: None,
                verification_count: 0,
                last_used_at: None,
                tags: None,
            },
            content: ExperienceContent {
                title: "Test Experience".to_string(),
                summary: "A test experience entry".to_string(),
                domain: domain.to_string(),
                pattern,
                confidence: 0.9,
                context: serde_json::Value::Null,
                knowledge: serde_json::Value::Null,
                outcomes: serde_json::Value::Null,
            },
            usage_stats: ExperienceUsageStats {
                access_count: 0,
                last_accessed_at: None,
                verification_count: 0,
            },
            relationships: ExperienceRelationships {
                related_experience_ids: None,
                contradicts_experience_ids: None,
                refines_experience_ids: None,
            },
        }
    }

    #[test]
    fn test_write_experience() {
        let (sqlite, sled) = setup();
        let layer = ExperienceLayer::new(sqlite, sled);

        let exp = make_experience("exp-001", "navigation", PatternType::ErrorHandling);
        let result = layer.write_experience(exp, false).unwrap();

        assert_eq!(result.experience_id, "exp-001");
        assert!(!result.verified);
    }

    #[test]
    fn test_write_experience_auto_verify() {
        let (sqlite, sled) = setup();
        let layer = ExperienceLayer::new(sqlite, sled);

        let exp = make_experience("exp-002", "planning", PatternType::TaskPattern);
        let result = layer.write_experience(exp, true).unwrap();

        assert_eq!(result.experience_id, "exp-002");
        assert!(result.verified);
    }

    #[test]
    fn test_query_by_domain() {
        let (sqlite, sled) = setup();
        let layer = ExperienceLayer::new(sqlite, sled);

        layer
            .write_experience(
                make_experience("exp-010", "navigation", PatternType::ErrorHandling),
                false,
            )
            .unwrap();
        layer
            .write_experience(
                make_experience("exp-011", "navigation", PatternType::TaskPattern),
                false,
            )
            .unwrap();
        layer
            .write_experience(
                make_experience("exp-012", "planning", PatternType::GoalDecomposition),
                false,
            )
            .unwrap();

        let entries = layer.query_by_domain("navigation", None).unwrap();
        assert_eq!(entries.len(), 2);

        let entries = layer.query_by_domain("planning", None).unwrap();
        assert_eq!(entries.len(), 1);

        let entries = layer.query_by_domain("nonexistent", None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_query_by_pattern_type() {
        let (sqlite, sled) = setup();
        let layer = ExperienceLayer::new(sqlite, sled);

        layer
            .write_experience(
                make_experience("exp-020", "nav", PatternType::ErrorHandling),
                false,
            )
            .unwrap();
        layer
            .write_experience(
                make_experience("exp-021", "plan", PatternType::ErrorHandling),
                false,
            )
            .unwrap();
        layer
            .write_experience(
                make_experience("exp-022", "nav", PatternType::ToolSequence),
                false,
            )
            .unwrap();

        let entries = layer.query_by_pattern_type("ERROR_HANDLING").unwrap();
        assert_eq!(entries.len(), 2);

        let entries = layer.query_by_pattern_type("TOOL_SEQUENCE").unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_update_usage() {
        let (sqlite, sled) = setup();
        let layer = ExperienceLayer::new(sqlite, sled);

        layer
            .write_experience(
                make_experience("exp-030", "nav", PatternType::ErrorHandling),
                false,
            )
            .unwrap();

        layer.update_usage("exp-030").unwrap();
        layer.update_usage("exp-030").unwrap();

        let entries = layer.query_by_domain("nav", None).unwrap();
        assert_eq!(entries[0].access_count, 2);
    }

    #[test]
    fn test_delete_experience() {
        let (sqlite, sled) = setup();
        let layer = ExperienceLayer::new(sqlite, sled);

        layer
            .write_experience(
                make_experience("exp-040", "nav", PatternType::ErrorHandling),
                false,
            )
            .unwrap();

        let deleted = layer.delete_experience("exp-040").unwrap();
        assert!(deleted);

        let deleted_again = layer.delete_experience("exp-040").unwrap();
        assert!(!deleted_again);

        let entries = layer.query_by_domain("nav", None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_query_by_domain_returns_memory_entries() {
        let (sqlite, sled) = setup();
        let layer = ExperienceLayer::new(sqlite, sled);

        layer
            .write_experience(
                make_experience("exp-050", "testing", PatternType::BeliefCorrection),
                false,
            )
            .unwrap();

        let entries = layer.query_by_domain("testing", None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].layer, MemoryLayer::Experience);
        assert_eq!(entries[0].entry_id, "exp-050");
    }
}
