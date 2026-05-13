use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::*;

#[derive(Clone, Debug)]
pub struct Conflict {
    pub conflict_id: String,
    pub conflict_type: ConflictType,
    pub affected_entities: Vec<AffectedEntity>,
    pub divergence: Divergence,
    pub context: ConflictContext,
    pub detected_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConflictType {
    AttributeMismatch,
    RelationMismatch,
    IntentMismatch,
    ValueConfidenceConflict,
}

#[derive(Clone, Debug)]
pub struct AffectedEntity {
    pub local_belief: BeliefState,
    pub remote_belief: BeliefState,
}

#[derive(Clone, Debug)]
pub struct BeliefState {
    pub node_id: String,
    pub attributes: HashMap<String, serde_json::Value>,
    pub confidence: f64,
    pub source: Option<String>,
    pub last_updated: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct Divergence {
    pub attribute_name: String,
    pub local_value: serde_json::Value,
    pub remote_value: serde_json::Value,
    pub deviation_metric: f64,
}

#[derive(Clone, Debug)]
pub struct ConflictContext {
    pub scope: String,
    pub intent_relevance: f64,
    pub impact_assessment: ImpactAssessment,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImpactAssessment {
    Low,
    Medium,
    High,
    Critical,
}

pub struct ConflictDetector;

const CONFIDENCE_DIFF_THRESHOLD: f64 = 0.1;

impl ConflictDetector {
    pub fn detect_conflicts(
        local: &EntityBelief,
        remote: &EntityBelief,
    ) -> Result<Option<Conflict>, CommunicationError> {
        let local_attrs = match &local.key_attributes {
            Some(attrs) => attrs,
            None => return Ok(None),
        };
        let remote_attrs = match &remote.key_attributes {
            Some(attrs) => attrs,
            None => return Ok(None),
        };

        for (key, local_val) in local_attrs {
            if let Some(remote_val) = remote_attrs.get(key) {
                if local_val.value != remote_val.value {
                    let confidence_diff = (local_val.confidence - remote_val.confidence).abs();
                    if confidence_diff > CONFIDENCE_DIFF_THRESHOLD {
                        let deviation_metric = confidence_diff;
                        let impact = Self::assess_impact(confidence_diff);

                        let local_belief = BeliefState {
                            node_id: local.node_id.clone(),
                            attributes: local_attrs
                                .iter()
                                .map(|(k, v)| (k.clone(), v.value.clone()))
                                .collect(),
                            confidence: local_val.confidence,
                            source: local_val.source.clone(),
                            last_updated: local_val.last_updated,
                        };

                        let remote_belief = BeliefState {
                            node_id: remote.node_id.clone(),
                            attributes: remote_attrs
                                .iter()
                                .map(|(k, v)| (k.clone(), v.value.clone()))
                                .collect(),
                            confidence: remote_val.confidence,
                            source: remote_val.source.clone(),
                            last_updated: remote_val.last_updated,
                        };

                        return Ok(Some(Conflict {
                            conflict_id: Uuid::new_v4().to_string(),
                            conflict_type: ConflictType::AttributeMismatch,
                            affected_entities: vec![AffectedEntity {
                                local_belief,
                                remote_belief,
                            }],
                            divergence: Divergence {
                                attribute_name: key.clone(),
                                local_value: local_val.value.clone(),
                                remote_value: remote_val.value.clone(),
                                deviation_metric,
                            },
                            context: ConflictContext {
                                scope: format!("entity:{}", local.node_id),
                                intent_relevance: 1.0 - confidence_diff,
                                impact_assessment: impact,
                            },
                            detected_at: Utc::now(),
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn detect_conflicts_in_snapshot(
        snapshot: &CommunicationSnapshot,
    ) -> Result<Vec<Conflict>, CommunicationError> {
        let mut conflicts = Vec::new();

        let mut groups: HashMap<String, Vec<&EntityBelief>> = HashMap::new();
        for belief in &snapshot.entity_beliefs {
            groups
                .entry(belief.node_id.clone())
                .or_default()
                .push(belief);
        }

        for beliefs in groups.values() {
            if beliefs.len() < 2 {
                continue;
            }
            for i in 0..beliefs.len() {
                for j in (i + 1)..beliefs.len() {
                    if let Some(conflict) = Self::detect_conflicts(beliefs[i], beliefs[j])? {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        let mut relation_groups: HashMap<(String, String), Vec<&RelationBelief>> = HashMap::new();
        for relation in &snapshot.relation_beliefs {
            let key = (
                relation.source_entity.node_id.clone(),
                relation.target_entity.node_id.clone(),
            );
            relation_groups.entry(key).or_default().push(relation);
        }

        for relations in relation_groups.values() {
            if relations.len() < 2 {
                continue;
            }
            for i in 0..relations.len() {
                for j in (i + 1)..relations.len() {
                    let a = relations[i];
                    let b = relations[j];
                    let edge_mismatch = a.edge_type != b.edge_type;
                    let confidence_diff = (a.confidence - b.confidence).abs();
                    let confidence_conflict = confidence_diff > CONFIDENCE_DIFF_THRESHOLD;

                    if edge_mismatch || confidence_conflict {
                        let deviation_metric = if edge_mismatch {
                            1.0
                        } else {
                            confidence_diff
                        };
                        let impact = Self::assess_impact(deviation_metric);

                        let local_belief = BeliefState {
                            node_id: format!(
                                "{}->{}",
                                a.source_entity.node_id, a.target_entity.node_id
                            ),
                            attributes: a.attributes.clone().unwrap_or_default(),
                            confidence: a.confidence,
                            source: None,
                            last_updated: None,
                        };

                        let remote_belief = BeliefState {
                            node_id: format!(
                                "{}->{}",
                                b.source_entity.node_id, b.target_entity.node_id
                            ),
                            attributes: b.attributes.clone().unwrap_or_default(),
                            confidence: b.confidence,
                            source: None,
                            last_updated: None,
                        };

                        conflicts.push(Conflict {
                            conflict_id: Uuid::new_v4().to_string(),
                            conflict_type: ConflictType::RelationMismatch,
                            affected_entities: vec![AffectedEntity {
                                local_belief,
                                remote_belief,
                            }],
                            divergence: Divergence {
                                attribute_name: if edge_mismatch {
                                    "edge_type".to_string()
                                } else {
                                    "confidence".to_string()
                                },
                                local_value: if edge_mismatch {
                                    serde_json::json!(format!("{:?}", a.edge_type))
                                } else {
                                    serde_json::json!(a.confidence)
                                },
                                remote_value: if edge_mismatch {
                                    serde_json::json!(format!("{:?}", b.edge_type))
                                } else {
                                    serde_json::json!(b.confidence)
                                },
                                deviation_metric,
                            },
                            context: ConflictContext {
                                scope: format!(
                                    "relation:{}->{}",
                                    a.source_entity.node_id, a.target_entity.node_id
                                ),
                                intent_relevance: 1.0 - deviation_metric,
                                impact_assessment: impact,
                            },
                            detected_at: Utc::now(),
                        });
                    }
                }
            }
        }

        Ok(conflicts)
    }

    fn assess_impact(confidence_diff: f64) -> ImpactAssessment {
        if confidence_diff < 0.2 {
            ImpactAssessment::Low
        } else if confidence_diff < 0.4 {
            ImpactAssessment::Medium
        } else if confidence_diff < 0.6 {
            ImpactAssessment::High
        } else {
            ImpactAssessment::Critical
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity_belief(
        node_id: &str,
        attrs: HashMap<String, CommAttributeValue>,
    ) -> EntityBelief {
        EntityBelief {
            node_id: node_id.to_string(),
            node_type: CommNodeType::User,
            name: None,
            key_attributes: Some(attrs),
            tags: Vec::new(),
        }
    }

    fn make_attr(value: &str, confidence: f64) -> CommAttributeValue {
        CommAttributeValue {
            value: serde_json::json!(value),
            confidence,
            source: None,
            last_updated: None,
        }
    }

    #[test]
    fn test_no_conflict_when_same() {
        let mut attrs = HashMap::new();
        attrs.insert("role".to_string(), make_attr("admin", 0.9));

        let local = make_entity_belief("node-1", attrs.clone());
        let remote = make_entity_belief("node-1", attrs);

        let result = ConflictDetector::detect_conflicts(&local, &remote).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_conflict_on_attribute_mismatch() {
        let mut local_attrs = HashMap::new();
        local_attrs.insert("role".to_string(), make_attr("admin", 0.9));

        let mut remote_attrs = HashMap::new();
        remote_attrs.insert("role".to_string(), make_attr("viewer", 0.7));

        let local = make_entity_belief("node-1", local_attrs);
        let remote = make_entity_belief("node-1", remote_attrs);

        let result = ConflictDetector::detect_conflicts(&local, &remote).unwrap();
        assert!(result.is_some());

        let conflict = result.unwrap();
        assert_eq!(conflict.conflict_type, ConflictType::AttributeMismatch);
        assert_eq!(conflict.divergence.attribute_name, "role");
        assert_eq!(conflict.divergence.local_value, serde_json::json!("admin"));
        assert_eq!(
            conflict.divergence.remote_value,
            serde_json::json!("viewer")
        );
        assert!((conflict.divergence.deviation_metric - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_no_conflict_below_threshold() {
        let mut local_attrs = HashMap::new();
        local_attrs.insert("role".to_string(), make_attr("admin", 0.9));

        let mut remote_attrs = HashMap::new();
        remote_attrs.insert("role".to_string(), make_attr("viewer", 0.95));

        let local = make_entity_belief("node-1", local_attrs);
        let remote = make_entity_belief("node-1", remote_attrs);

        let result = ConflictDetector::detect_conflicts(&local, &remote).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_impact_assessment_levels() {
        assert_eq!(ConflictDetector::assess_impact(0.15), ImpactAssessment::Low);
        assert_eq!(
            ConflictDetector::assess_impact(0.25),
            ImpactAssessment::Medium
        );
        assert_eq!(
            ConflictDetector::assess_impact(0.45),
            ImpactAssessment::High
        );
        assert_eq!(
            ConflictDetector::assess_impact(0.7),
            ImpactAssessment::Critical
        );
    }

    #[test]
    fn test_detect_conflicts_in_snapshot_stub() {
        let snapshot = CommunicationSnapshot {
            snapshot_id: "test".to_string(),
            snapshot_metadata: SnapshotMetadata {
                version: "1.0".to_string(),
                timestamp: Utc::now(),
                source_agent: SourceAgent {
                    agent_id: "a1".to_string(),
                    agent_type: None,
                    capabilities: Vec::new(),
                },
                scope: CommSnapshotScope::default(),
                purpose: SnapshotPurpose::Sync,
                priority: Priority::Normal,
                expires_at: None,
            },
            entity_beliefs: Vec::new(),
            relation_beliefs: Vec::new(),
            intention_summary: None,
            prediction_residual_summary: Vec::new(),
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        };

        let result = ConflictDetector::detect_conflicts_in_snapshot(&snapshot).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_no_conflict_when_no_key_attributes() {
        let local = EntityBelief {
            node_id: "node-1".to_string(),
            node_type: CommNodeType::User,
            name: None,
            key_attributes: None,
            tags: Vec::new(),
        };
        let remote = EntityBelief {
            node_id: "node-1".to_string(),
            node_type: CommNodeType::User,
            name: None,
            key_attributes: None,
            tags: Vec::new(),
        };

        let result = ConflictDetector::detect_conflicts(&local, &remote).unwrap();
        assert!(result.is_none());
    }
}
