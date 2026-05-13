use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::modules::communication::types::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldFilter {
    pub allowed_fields: HashSet<String>,
    pub blocked_tags: HashSet<String>,
    pub min_confidence: Option<f64>,
}

impl FieldFilter {
    pub fn new() -> Self {
        let mut blocked_tags = HashSet::new();
        blocked_tags.insert("CREDENTIAL".to_string());
        blocked_tags.insert("PII".to_string());
        blocked_tags.insert("CONFIDENTIAL".to_string());
        blocked_tags.insert("SECURITY_CONFIG".to_string());

        Self {
            allowed_fields: HashSet::new(),
            blocked_tags,
            min_confidence: None,
        }
    }

    pub fn with_allowed_fields(mut self, fields: Vec<String>) -> Self {
        self.allowed_fields = fields.into_iter().collect();
        self
    }

    pub fn with_min_confidence(mut self, min: f64) -> Self {
        self.min_confidence = Some(min);
        self
    }

    pub fn apply_to_snapshot(&self, snapshot: &mut CommunicationSnapshot) -> FilterReport {
        let mut report = FilterReport::default();
        let mut filtered_types: HashSet<String> = HashSet::new();

        for entity in &mut snapshot.entity_beliefs {
            if let Some(ref mut attrs) = entity.key_attributes {
                let original_len = attrs.len();

                let keys_to_remove: Vec<String> = attrs
                    .iter()
                    .filter(|(key, value)| {
                        if !self.allowed_fields.is_empty() && !self.allowed_fields.contains(*key) {
                            return true;
                        }
                        if let Some(min_conf) = self.min_confidence {
                            if value.confidence < min_conf {
                                return true;
                            }
                        }
                        false
                    })
                    .map(|(k, _)| k.clone())
                    .collect();

                for key in &keys_to_remove {
                    attrs.remove(key);
                    report.filtered_fields.push(key.clone());
                }

                if keys_to_remove.len() == original_len {
                    report.entities_filtered += 1;
                }
            }

            let original_tags_len = entity.tags.len();
            entity.tags.retain(|tag| !self.blocked_tags.contains(tag));
            let removed_tags = original_tags_len - entity.tags.len();
            if removed_tags > 0 {
                filtered_types.insert("blocked_tag".to_string());
            }
        }

        for relation in &mut snapshot.relation_beliefs {
            if let Some(ref mut attrs) = relation.attributes {
                let has_blocked = attrs.keys().any(|k| self.blocked_tags.contains(k));
                if has_blocked {
                    attrs.clear();
                    report.relations_filtered += 1;
                    filtered_types.insert("blocked_relation_attribute".to_string());
                }
            }
        }

        if let Some(ref mut intention) = snapshot.intention_summary {
            let original_blockers = intention.blockers.len();
            intention.blockers.retain(|b| {
                let severity_tag = format!("{:?}", b.severity);
                !self.blocked_tags.contains(&severity_tag)
            });
            if intention.blockers.len() < original_blockers {
                filtered_types.insert("blocked_intention_blocker".to_string());
            }
        }

        report.filtered_types = filtered_types.into_iter().collect();
        report
    }
}

impl Default for FieldFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct FilterReport {
    pub entities_filtered: usize,
    pub relations_filtered: usize,
    pub filtered_fields: Vec<String>,
    pub filtered_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_agent: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldFilterRule {
    pub rule_id: String,
    pub rule_type: FilterRuleType,
    pub action: FilterAction,
    pub target: FilterTarget,
    pub condition: FilterCondition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FilterRuleType {
    FieldAllow,
    FieldBlock,
    TagBlock,
    ConfidenceThreshold,
    Custom,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FilterAction {
    Remove,
    Redact,
    Replace(serde_json::Value),
    Warn,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FilterTarget {
    EntityAttribute(String),
    EntityTag,
    RelationAttribute(String),
    IntentionBlocker,
    All,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilterCondition {
    pub field_name: Option<String>,
    pub tag_name: Option<String>,
    pub min_confidence: Option<f64>,
    pub pattern: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilteredSnapshot {
    pub snapshot: CommunicationSnapshot,
    pub filter_report: FilterReport,
    pub applied_rules: Vec<FieldFilterRule>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_snapshot_with_tags(tags: Vec<String>) -> CommunicationSnapshot {
        CommunicationSnapshot {
            snapshot_id: "test-filter-001".to_string(),
            snapshot_metadata: SnapshotMetadata {
                version: "1.0".to_string(),
                timestamp: Utc::now(),
                source_agent: SourceAgent {
                    agent_id: "agent-test".to_string(),
                    agent_type: None,
                    capabilities: vec![],
                },
                scope: CommSnapshotScope::default(),
                purpose: SnapshotPurpose::Sync,
                priority: Priority::Normal,
                expires_at: None,
            },
            entity_beliefs: vec![EntityBelief {
                node_id: "node-1".to_string(),
                node_type: CommNodeType::User,
                name: Some("Alice".to_string()),
                key_attributes: Some({
                    let mut map = HashMap::new();
                    map.insert(
                        "role".to_string(),
                        CommAttributeValue {
                            value: serde_json::json!("admin"),
                            confidence: 0.95,
                            source: None,
                            last_updated: None,
                        },
                    );
                    map.insert(
                        "secret_key".to_string(),
                        CommAttributeValue {
                            value: serde_json::json!("abc123"),
                            confidence: 0.8,
                            source: None,
                            last_updated: None,
                        },
                    );
                    map
                }),
                tags,
            }],
            relation_beliefs: vec![RelationBelief {
                edge_id: "edge-1".to_string(),
                edge_type: CommEdgeType::Owns,
                source_entity: EntityReference {
                    node_id: "node-1".to_string(),
                    name: None,
                },
                target_entity: EntityReference {
                    node_id: "node-2".to_string(),
                    name: None,
                },
                attributes: Some({
                    let mut map = HashMap::new();
                    map.insert("CREDENTIAL".to_string(), serde_json::json!("sensitive"));
                    map
                }),
                confidence: 0.9,
                directional: true,
            }],
            intention_summary: Some(IntentionSummary {
                top_goal: TopGoal {
                    description: "Complete task".to_string(),
                    target_state: None,
                },
                key_subtasks: vec![],
                blockers: vec![Blocker {
                    blocker_id: "b1".to_string(),
                    description: "Missing access".to_string(),
                    severity: ResidualSeverity::Critical,
                }],
                estimated_completion_steps: None,
            }),
            prediction_residual_summary: vec![],
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        }
    }

    #[test]
    fn test_default_filter_blocks_sensitive_tags() {
        let mut snapshot = create_test_snapshot_with_tags(vec![
            "important".to_string(),
            "CREDENTIAL".to_string(),
            "PII".to_string(),
        ]);
        let filter = FieldFilter::new();
        let report = filter.apply_to_snapshot(&mut snapshot);

        assert!(!snapshot.entity_beliefs[0]
            .tags
            .contains(&"CREDENTIAL".to_string()));
        assert!(!snapshot.entity_beliefs[0].tags.contains(&"PII".to_string()));
        assert!(snapshot.entity_beliefs[0]
            .tags
            .contains(&"important".to_string()));
        assert!(report.filtered_types.contains(&"blocked_tag".to_string()));
    }

    #[test]
    fn test_allowed_fields_filter() {
        let mut snapshot = create_test_snapshot_with_tags(vec![]);
        let filter = FieldFilter::new().with_allowed_fields(vec!["role".to_string()]);
        let report = filter.apply_to_snapshot(&mut snapshot);

        let attrs = snapshot.entity_beliefs[0].key_attributes.as_ref().unwrap();
        assert!(attrs.contains_key("role"));
        assert!(!attrs.contains_key("secret_key"));
        assert!(report.filtered_fields.contains(&"secret_key".to_string()));
    }

    #[test]
    fn test_min_confidence_filter() {
        let mut snapshot = create_test_snapshot_with_tags(vec![]);
        let filter = FieldFilter::new().with_min_confidence(0.9);
        let report = filter.apply_to_snapshot(&mut snapshot);

        let attrs = snapshot.entity_beliefs[0].key_attributes.as_ref().unwrap();
        assert!(attrs.contains_key("role"));
        assert!(!attrs.contains_key("secret_key"));
        assert!(report.filtered_fields.contains(&"secret_key".to_string()));
    }

    #[test]
    fn test_apply_to_snapshot_blocks_relation_attributes() {
        let mut snapshot = create_test_snapshot_with_tags(vec![]);
        let filter = FieldFilter::new();
        let report = filter.apply_to_snapshot(&mut snapshot);

        let relation = &snapshot.relation_beliefs[0];
        assert!(relation.attributes.as_ref().unwrap().is_empty());
        assert_eq!(report.relations_filtered, 1);
    }
}
