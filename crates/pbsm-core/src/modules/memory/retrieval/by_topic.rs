use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use crate::modules::memory::cache::lru::LruCache;
use crate::modules::memory::error::Result;
use crate::modules::memory::layers::experience::ExperienceLayer;
use crate::modules::memory::layers::raw_log::RawLogLayer;
use crate::modules::memory::layers::snapshot::SnapshotLayer;
use crate::modules::memory::types::*;

pub struct TopicRetriever {
    raw_log_layer: Arc<RawLogLayer>,
    snapshot_layer: Arc<SnapshotLayer>,
    experience_layer: Arc<ExperienceLayer>,
    cache: parking_lot::Mutex<LruCache<RetrievalResult>>,
}

impl TopicRetriever {
    pub fn new(
        raw_log: Arc<RawLogLayer>,
        snapshot: Arc<SnapshotLayer>,
        experience: Arc<ExperienceLayer>,
    ) -> Self {
        Self {
            raw_log_layer: raw_log,
            snapshot_layer: snapshot,
            experience_layer: experience,
            cache: parking_lot::Mutex::new(LruCache::new(100, Duration::from_secs(300))),
        }
    }

    pub async fn retrieve(
        &self,
        topic: &str,
        confidence_threshold: Option<f64>,
        layer_filter: Option<Vec<MemoryLayer>>,
        limit: usize,
        offset: usize,
        include_raw_logs: bool,
    ) -> Result<RetrievalResult> {
        let cache_key = format!(
            "{}:{:?}:{:?}:{}:{}:{}",
            topic, confidence_threshold, layer_filter, limit, offset, include_raw_logs
        );

        {
            let mut cache = self.cache.lock();
            if let Some(cached) = cache.get(&cache_key) {
                let mut result = cached.clone();
                result.search_metadata.cache_hit = true;
                return Ok(result);
            }
        }

        let start = std::time::Instant::now();
        let layers = layer_filter.unwrap_or_else(|| {
            vec![
                MemoryLayer::Experience,
                MemoryLayer::Snapshot,
                MemoryLayer::RawLog,
            ]
        });

        let mut all_entries: Vec<MemoryEntry> = Vec::new();
        let mut indexes_used = Vec::new();

        if layers.contains(&MemoryLayer::Experience) {
            indexes_used.push("experience_domain_index".to_string());
            let experiences = self
                .experience_layer
                .query_by_domain(topic, confidence_threshold)
                .await?;
            for mut exp in experiences {
                let topic_lower = topic.to_lowercase();
                let relevance = if exp.summary.to_lowercase().contains(&topic_lower) {
                    0.5 + exp.confidence * 0.3
                } else {
                    0.3 + exp.confidence * 0.2
                };
                exp.relevance_score = relevance;
                all_entries.push(exp);
            }
        }

        if layers.contains(&MemoryLayer::Snapshot) {
            indexes_used.push("snapshot_trigger_index".to_string());
            let snapshots = self.snapshot_layer.list_snapshots(None).await?;
            for snap_meta in snapshots {
                let topic_lower = topic.to_lowercase();
                if !snap_meta
                    .trigger_description
                    .to_lowercase()
                    .contains(&topic_lower)
                {
                    continue;
                }
                let relevance = 0.4
                    + 0.3
                        * (snap_meta
                            .trigger_description
                            .to_lowercase()
                            .contains(&topic_lower) as u8 as f64);
                let entry = MemoryEntry {
                    entry_id: snap_meta.snapshot_id.clone(),
                    layer: MemoryLayer::Snapshot,
                    memory_type: format!("{:?}", snap_meta.snapshot_type),
                    relevance_score: relevance,
                    confidence: 0.7,
                    summary: snap_meta.trigger_description.clone(),
                    content: serde_json::to_value(&snap_meta).unwrap_or(serde_json::Value::Null),
                    source_references: vec![SourceReference {
                        ref_type: "snapshot".to_string(),
                        ref_id: snap_meta.snapshot_id.clone(),
                        ref_path: None,
                    }],
                    created_at: chrono::DateTime::from_timestamp_millis(snap_meta.created_at)
                        .unwrap_or(chrono::Utc::now()),
                    access_count: 0,
                };
                if let Some(threshold) = confidence_threshold {
                    if entry.confidence >= threshold {
                        all_entries.push(entry);
                    }
                } else {
                    all_entries.push(entry);
                }
            }
        }

        if layers.contains(&MemoryLayer::RawLog) && include_raw_logs {
            indexes_used.push("raw_log_topic_index".to_string());
            let logs = self
                .raw_log_layer
                .query_by_topic(topic, confidence_threshold, limit)
                .await?;
            for mut log in logs {
                log.relevance_score = 0.3 + log.confidence * 0.2;
                all_entries.push(log);
            }
        }

        if let Some(threshold) = confidence_threshold {
            all_entries.retain(|e| e.confidence >= threshold);
        }

        all_entries.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_matches = all_entries.len();
        let paginated: Vec<MemoryEntry> =
            all_entries.into_iter().skip(offset).take(limit).collect();

        let has_more = offset + limit < total_matches;

        let result = RetrievalResult {
            request_id: Uuid::new_v4().to_string(),
            query_topic: topic.to_string(),
            total_matches,
            results: paginated,
            pagination: PaginationInfo {
                offset,
                limit,
                total_count: total_matches,
                has_more,
            },
            search_metadata: SearchMetadata {
                search_duration_ms: start.elapsed().as_millis() as i64,
                indexes_used,
                cache_hit: false,
            },
        };

        {
            let mut cache = self.cache.lock();
            cache.insert(cache_key, result.clone());
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_retriever_cache_key_format() {
        let key = format!(
            "{}:{:?}:{:?}:{}:{}:{}",
            "test",
            Some(0.5),
            Some(vec![MemoryLayer::Experience]),
            10,
            0,
            false
        );
        assert!(key.starts_with("test:"));
        assert!(key.contains("Experience"));
    }

    #[test]
    fn test_pagination_calculation() {
        let total = 25;
        let offset = 10;
        let limit = 10;
        let has_more = offset + limit < total;
        assert!(has_more);

        let offset = 20;
        let has_more = offset + limit < total;
        assert!(!has_more);
    }

    #[test]
    fn test_confidence_threshold_filtering() {
        let entries = vec![
            MemoryEntry {
                entry_id: "1".to_string(),
                layer: MemoryLayer::Experience,
                memory_type: "pattern".to_string(),
                relevance_score: 0.9,
                confidence: 0.8,
                summary: "high confidence".to_string(),
                content: serde_json::Value::Null,
                source_references: vec![],
                created_at: chrono::Utc::now(),
                access_count: 0,
            },
            MemoryEntry {
                entry_id: "2".to_string(),
                layer: MemoryLayer::RawLog,
                memory_type: "dialogue".to_string(),
                relevance_score: 0.5,
                confidence: 0.3,
                summary: "low confidence".to_string(),
                content: serde_json::Value::Null,
                source_references: vec![],
                created_at: chrono::Utc::now(),
                access_count: 0,
            },
        ];

        let filtered: Vec<&MemoryEntry> = entries.iter().filter(|e| e.confidence >= 0.5).collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].entry_id, "1");
    }

    #[test]
    fn test_relevance_score_sorting() {
        let mut entries = vec![
            MemoryEntry {
                entry_id: "1".to_string(),
                layer: MemoryLayer::Experience,
                memory_type: "pattern".to_string(),
                relevance_score: 0.5,
                confidence: 0.8,
                summary: "medium".to_string(),
                content: serde_json::Value::Null,
                source_references: vec![],
                created_at: chrono::Utc::now(),
                access_count: 0,
            },
            MemoryEntry {
                entry_id: "2".to_string(),
                layer: MemoryLayer::Snapshot,
                memory_type: "snapshot".to_string(),
                relevance_score: 0.9,
                confidence: 0.7,
                summary: "high".to_string(),
                content: serde_json::Value::Null,
                source_references: vec![],
                created_at: chrono::Utc::now(),
                access_count: 0,
            },
            MemoryEntry {
                entry_id: "3".to_string(),
                layer: MemoryLayer::RawLog,
                memory_type: "log".to_string(),
                relevance_score: 0.3,
                confidence: 0.5,
                summary: "low".to_string(),
                content: serde_json::Value::Null,
                source_references: vec![],
                created_at: chrono::Utc::now(),
                access_count: 0,
            },
        ];

        entries.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        assert_eq!(entries[0].entry_id, "2");
        assert_eq!(entries[1].entry_id, "1");
        assert_eq!(entries[2].entry_id, "3");
    }

    #[test]
    fn test_layer_filter_default() {
        let layer_filter: Option<Vec<MemoryLayer>> = None;
        let layers = layer_filter.unwrap_or_else(|| {
            vec![
                MemoryLayer::Experience,
                MemoryLayer::Snapshot,
                MemoryLayer::RawLog,
            ]
        });
        assert_eq!(layers.len(), 3);
    }

    #[test]
    fn test_retrieval_result_construction() {
        let result = RetrievalResult {
            request_id: Uuid::new_v4().to_string(),
            query_topic: "test_topic".to_string(),
            total_matches: 0,
            results: vec![],
            pagination: PaginationInfo {
                offset: 0,
                limit: 10,
                total_count: 0,
                has_more: false,
            },
            search_metadata: SearchMetadata {
                search_duration_ms: 5,
                indexes_used: vec!["experience_domain_index".to_string()],
                cache_hit: false,
            },
        };
        assert!(!result.search_metadata.cache_hit);
        assert_eq!(result.total_matches, 0);
    }
}
