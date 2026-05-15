use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::modules::memory::error::Result;
use crate::modules::memory::layers::experience::ExperienceLayer;
use crate::modules::memory::layers::snapshot::SnapshotLayer;
use crate::modules::memory::types::*;

pub struct ContextRetriever {
    experience_layer: Arc<ExperienceLayer>,
    snapshot_layer: Arc<SnapshotLayer>,
}

impl ContextRetriever {
    pub fn new(experience: Arc<ExperienceLayer>, snapshot: Arc<SnapshotLayer>) -> Self {
        Self {
            experience_layer: experience,
            snapshot_layer: snapshot,
        }
    }

    pub fn retrieve(
        &self,
        current_beliefs: &[BeliefContext],
        _intent_description: &str,
        confidence_gaps: Option<Vec<ConfidenceGap>>,
        retrieval_depth: RetrievalDepth,
        max_results_per_gap: usize,
    ) -> Result<ContextualRetrievalResult> {
        let gaps = confidence_gaps.unwrap_or_else(|| self.derive_gaps(current_beliefs));

        let mut retrieved_knowledge = Vec::new();
        let mut confidence_predictions = HashMap::new();
        let mut total_improvement = 0.0f64;

        for gap in &gaps {
            let effective_limit = match retrieval_depth {
                RetrievalDepth::Shallow => 1.min(max_results_per_gap),
                RetrievalDepth::Standard => 3.min(max_results_per_gap),
                RetrievalDepth::Deep => max_results_per_gap,
            };

            let experiences = self
                .experience_layer
                .query_by_domain(&gap.topic, None)?;

            let snapshots = self
                .snapshot_layer
                .list_snapshots(None)
                .unwrap_or_default();

            let mut assertions = Vec::new();
            let mut suggestions = Vec::new();
            let mut source_experience_ids = Vec::new();
            let mut source_snapshot_ids = Vec::new();

            for exp in experiences.iter().take(effective_limit) {
                source_experience_ids.push(exp.entry_id.clone());

                if let Some(obj) = exp.content.as_object() {
                    for (key, value) in obj.iter().take(3) {
                        assertions.push(StructuredAssertion {
                            assertion_type: "derived_from_experience".to_string(),
                            subject_id: gap.topic.clone(),
                            predicate: key.clone(),
                            object_value: value.clone(),
                            confidence: exp.confidence * 0.5 + 0.3,
                            source: exp.entry_id.clone(),
                        });
                    }
                }

                suggestions.push(IntegrationSuggestion {
                    target_node_id: Some(gap.topic.clone()),
                    action: "augment_knowledge".to_string(),
                    priority: ((gap.required_confidence - gap.current_confidence) * 10.0) as u32,
                    conflict_notes: None,
                });
            }

            let matching_snapshots: Vec<_> = snapshots
                .iter()
                .filter(|s| {
                    s.trigger
                        .description
                        .to_lowercase()
                        .contains(&gap.topic.to_lowercase())
                })
                .take(effective_limit)
                .collect();

            for snap in matching_snapshots {
                source_snapshot_ids.push(snap.snapshot_id.clone());
            }

            let improvement = (gap.required_confidence - gap.current_confidence) * 0.6;
            confidence_predictions.insert(gap.topic.clone(), gap.current_confidence + improvement);
            total_improvement += improvement;

            let bundle = KnowledgeBundle {
                bundle_id: Uuid::new_v4().to_string(),
                source_experience_ids,
                source_snapshot_ids,
                structured_assertions: assertions,
                integration_suggestions: suggestions,
            };
            retrieved_knowledge.push(bundle);
        }

        let confidence_improvement_estimate = if gaps.is_empty() {
            0.0
        } else {
            total_improvement / gaps.len() as f64
        };

        Ok(ContextualRetrievalResult {
            request_id: Uuid::new_v4().to_string(),
            identified_gaps: gaps,
            retrieved_knowledge,
            confidence_predictions,
            confidence_improvement_estimate,
        })
    }

    fn derive_gaps(&self, beliefs: &[BeliefContext]) -> Vec<ConfidenceGap> {
        let threshold = 0.7;
        beliefs
            .iter()
            .filter(|b| b.current_confidence < threshold)
            .map(|b| {
                let urgency = if b.current_confidence < 0.3 {
                    GapUrgency::High
                } else if b.current_confidence < 0.5 {
                    GapUrgency::Medium
                } else {
                    GapUrgency::Low
                };
                ConfidenceGap {
                    topic: b.topic.clone(),
                    required_confidence: threshold,
                    current_confidence: b.current_confidence,
                    urgency,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_belief(topic: &str, confidence: f64) -> BeliefContext {
        BeliefContext {
            topic: topic.to_string(),
            current_confidence: confidence,
            related_entities: vec![],
            last_access_time: Utc::now(),
        }
    }

    fn make_retriever() -> ContextRetriever {
        let sqlite = Arc::new(
            crate::modules::memory::storage::sqlite::SqliteStorage::open_in_memory().unwrap(),
        );
        sqlite.init_schema().unwrap();
        let sled =
            Arc::new(crate::modules::memory::storage::sled_kv::SledKvStore::open_temp().unwrap());
        let config = crate::modules::memory::config::MemoryConfig::default();
        ContextRetriever::new(
            Arc::new(ExperienceLayer::new(sqlite.clone(), sled.clone())),
            Arc::new(SnapshotLayer::new(sqlite, sled, config)),
        )
    }

    #[test]
    fn test_derive_gaps_filters_low_confidence() {
        let retriever = make_retriever();

        let beliefs = vec![
            make_belief("rust_ownership", 0.2),
            make_belief("rust_borrowing", 0.5),
            make_belief("rust_lifetimes", 0.9),
        ];

        let gaps = retriever.derive_gaps(&beliefs);
        assert_eq!(gaps.len(), 2);

        let gap_topics: Vec<&str> = gaps.iter().map(|g| g.topic.as_str()).collect();
        assert!(gap_topics.contains(&"rust_ownership"));
        assert!(gap_topics.contains(&"rust_borrowing"));
        assert!(!gap_topics.contains(&"rust_lifetimes"));
    }

    #[test]
    fn test_derive_gaps_urgency_classification() {
        let retriever = make_retriever();

        let beliefs = vec![
            make_belief("low", 0.1),
            make_belief("medium", 0.4),
            make_belief("borderline", 0.6),
        ];

        let gaps = retriever.derive_gaps(&beliefs);
        assert_eq!(gaps.len(), 3);

        assert_eq!(gaps[0].urgency, GapUrgency::High);
        assert_eq!(gaps[1].urgency, GapUrgency::Medium);
        assert_eq!(gaps[2].urgency, GapUrgency::Low);
    }

    #[test]
    fn test_derive_gaps_empty_beliefs() {
        let retriever = make_retriever();
        let gaps = retriever.derive_gaps(&[]);
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_derive_gaps_all_high_confidence() {
        let retriever = make_retriever();

        let beliefs = vec![make_belief("a", 0.8), make_belief("b", 0.95)];

        let gaps = retriever.derive_gaps(&beliefs);
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_confidence_improvement_estimate_empty_gaps() {
        let gaps: Vec<ConfidenceGap> = vec![];
        let total_improvement = 0.0f64;
        let estimate = if gaps.is_empty() {
            0.0
        } else {
            total_improvement / gaps.len() as f64
        };
        assert!((estimate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_confidence_improvement_calculation() {
        let gap = ConfidenceGap {
            topic: "test".to_string(),
            required_confidence: 0.9,
            current_confidence: 0.3,
            urgency: GapUrgency::High,
        };
        let improvement = (gap.required_confidence - gap.current_confidence) * 0.6;
        assert!((improvement - 0.36).abs() < f64::EPSILON);
    }

    #[test]
    fn test_contextual_retrieval_result_construction() {
        let result = ContextualRetrievalResult {
            request_id: Uuid::new_v4().to_string(),
            identified_gaps: vec![ConfidenceGap {
                topic: "test".to_string(),
                required_confidence: 0.8,
                current_confidence: 0.3,
                urgency: GapUrgency::Medium,
            }],
            retrieved_knowledge: vec![],
            confidence_predictions: {
                let mut map = HashMap::new();
                map.insert("test".to_string(), 0.7);
                map
            },
            confidence_improvement_estimate: 0.4,
        };
        assert_eq!(result.identified_gaps.len(), 1);
        assert_eq!(result.retrieved_knowledge.len(), 0);
        assert_eq!(result.confidence_predictions.get("test"), Some(&0.7));
    }
}
