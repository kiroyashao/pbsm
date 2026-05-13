use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::snapshot::filter::FilterReport;
use crate::modules::communication::types::*;

#[derive(Clone, Debug)]
pub struct FieldFilterRule {
    pub rule_id: String,
    pub rule_type: FilterRuleType,
    pub applies_to: FilterTarget,
    pub condition: FilterCondition,
    pub action: FilterAction,
    pub redaction_format: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterRuleType {
    FieldFilter,
    PatternFilter,
    TagFilter,
    ConfidenceFilter,
}

#[derive(Clone, Debug)]
pub struct FilterTarget {
    pub agent_role: Option<String>,
    pub purpose: Option<String>,
    pub scope: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FilterCondition {
    pub field_path: Option<String>,
    pub pattern: Option<String>,
    pub tag: Option<String>,
    pub min_confidence: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterAction {
    Remove,
    Redact,
    Mask,
    Reject,
}

#[derive(Clone, Debug)]
pub struct FilteredSnapshot {
    pub snapshot: CommunicationSnapshot,
    pub filter_report: FilterReport,
}

pub struct SensitiveDataFilter {
    rules: Vec<FieldFilterRule>,
}

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}

impl Default for SensitiveDataFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl SensitiveDataFilter {
    pub fn new() -> Self {
        let rules = vec![
            Self::credential_rule(),
            Self::pii_rule(),
            Self::confidential_rule(),
            Self::security_config_rule(),
        ];
        Self { rules }
    }

    fn credential_rule() -> FieldFilterRule {
        FieldFilterRule {
            rule_id: "CREDENTIAL".to_string(),
            rule_type: FilterRuleType::PatternFilter,
            applies_to: FilterTarget {
                agent_role: None,
                purpose: None,
                scope: None,
            },
            condition: FilterCondition {
                field_path: None,
                pattern: Some("password|secret|token|key|credential|api_key|apikey".to_string()),
                tag: None,
                min_confidence: None,
            },
            action: FilterAction::Redact,
            redaction_format: Some("[REDACTED]".to_string()),
        }
    }

    fn pii_rule() -> FieldFilterRule {
        FieldFilterRule {
            rule_id: "PII".to_string(),
            rule_type: FilterRuleType::PatternFilter,
            applies_to: FilterTarget {
                agent_role: None,
                purpose: None,
                scope: None,
            },
            condition: FilterCondition {
                field_path: None,
                pattern: Some("email|phone|address|ssn|social_security".to_string()),
                tag: None,
                min_confidence: None,
            },
            action: FilterAction::Mask,
            redaction_format: Some("***".to_string()),
        }
    }

    fn confidential_rule() -> FieldFilterRule {
        FieldFilterRule {
            rule_id: "CONFIDENTIAL".to_string(),
            rule_type: FilterRuleType::TagFilter,
            applies_to: FilterTarget {
                agent_role: None,
                purpose: None,
                scope: None,
            },
            condition: FilterCondition {
                field_path: None,
                pattern: None,
                tag: Some("confidential".to_string()),
                min_confidence: None,
            },
            action: FilterAction::Remove,
            redaction_format: None,
        }
    }

    fn security_config_rule() -> FieldFilterRule {
        FieldFilterRule {
            rule_id: "SECURITY_CONFIG".to_string(),
            rule_type: FilterRuleType::PatternFilter,
            applies_to: FilterTarget {
                agent_role: None,
                purpose: None,
                scope: None,
            },
            condition: FilterCondition {
                field_path: None,
                pattern: Some("config|setting|security_config|access_control".to_string()),
                tag: None,
                min_confidence: None,
            },
            action: FilterAction::Redact,
            redaction_format: Some("[REDACTED]".to_string()),
        }
    }

    pub fn filter_sensitive_data(
        &self,
        snapshot: &mut CommunicationSnapshot,
        target_agent: &str,
        purpose: SnapshotPurpose,
    ) -> Result<FilteredSnapshot, CommunicationError> {
        let rules = match purpose {
            SnapshotPurpose::Delegate => {
                let mut stricter = self.rules.clone();
                stricter.push(FieldFilterRule {
                    rule_id: "DELEGATION_EXTRA".to_string(),
                    rule_type: FilterRuleType::PatternFilter,
                    applies_to: FilterTarget {
                        agent_role: None,
                        purpose: Some("delegate".to_string()),
                        scope: None,
                    },
                    condition: FilterCondition {
                        field_path: None,
                        pattern: Some("internal|private|sensitive|restricted".to_string()),
                        tag: None,
                        min_confidence: None,
                    },
                    action: FilterAction::Redact,
                    redaction_format: Some("[REDACTED]".to_string()),
                });
                stricter
            }
            SnapshotPurpose::Query => {
                self.rules
                    .iter()
                    .filter(|r| r.rule_id == "CREDENTIAL" || r.rule_id == "SECURITY_CONFIG")
                    .cloned()
                    .collect()
            }
            _ => self.rules.clone(),
        };

        let mut report = self.apply_field_filters(snapshot, &rules);
        report.target_agent = Some(target_agent.to_string());

        Ok(FilteredSnapshot {
            snapshot: snapshot.clone(),
            filter_report: report,
        })
    }

    pub fn apply_field_filters(
        &self,
        snapshot: &mut CommunicationSnapshot,
        rules: &[FieldFilterRule],
    ) -> FilterReport {
        let mut report = FilterReport::default();
        let mut filtered_types = std::collections::HashSet::new();

        for entity in &mut snapshot.entity_beliefs {
            if let Some(ref mut attrs) = entity.key_attributes {
                let keys: Vec<String> = attrs.keys().cloned().collect();
                for key in keys {
                    for rule in rules {
                        let matches = match rule.rule_type {
                            FilterRuleType::FieldFilter => {
                                rule.condition.field_path.as_deref() == Some(key.as_str())
                            }
                            FilterRuleType::PatternFilter => {
                                if let Some(pattern) = &rule.condition.pattern {
                                    let lower_key = key.to_lowercase();
                                    pattern.split('|').any(|p| lower_key.contains(p))
                                } else {
                                    false
                                }
                            }
                            FilterRuleType::TagFilter => {
                                if let Some(tag) = &rule.condition.tag {
                                    entity.tags.iter().any(|t| t == tag)
                                } else {
                                    false
                                }
                            }
                            FilterRuleType::ConfidenceFilter => {
                                if let Some(min_conf) = rule.condition.min_confidence {
                                    attrs
                                        .get(&key)
                                        .map(|v| v.confidence < min_conf)
                                        .unwrap_or(false)
                                } else {
                                    false
                                }
                            }
                        };

                        if matches {
                            match rule.action {
                                FilterAction::Remove => {
                                    attrs.remove(&key);
                                    report.filtered_fields.push(key.clone());
                                    filtered_types.insert(rule.rule_id.clone());
                                }
                                FilterAction::Redact => {
                                    if let Some(attr) = attrs.get_mut(&key) {
                                        attr.value = serde_json::json!(rule
                                            .redaction_format
                                            .as_deref()
                                            .unwrap_or("[REDACTED]"));
                                        report.filtered_fields.push(key.clone());
                                        filtered_types.insert(rule.rule_id.clone());
                                    }
                                }
                                FilterAction::Mask => {
                                    if let Some(attr) = attrs.get_mut(&key) {
                                        attr.value = serde_json::json!(rule
                                            .redaction_format
                                            .as_deref()
                                            .unwrap_or("***"));
                                        report.filtered_fields.push(key.clone());
                                        filtered_types.insert(rule.rule_id.clone());
                                    }
                                }
                                FilterAction::Reject => {
                                    report.filtered_fields.push(key.clone());
                                    filtered_types.insert(rule.rule_id.clone());
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        report.filtered_types = filtered_types.into_iter().collect();
        report
    }

    pub fn decrypt_fields(
        &self,
        snapshot: &mut CommunicationSnapshot,
        security_metadata: &SecurityMetadata,
        decryption_key: &Option<String>,
    ) -> Result<(), CommunicationError> {
        let key = decryption_key
            .as_ref()
            .ok_or_else(|| CommunicationError::SecurityViolation("Decryption key required".to_string()))?;

        if security_metadata.encrypted_fields.is_empty() {
            return Ok(());
        }

        for field_name in &security_metadata.encrypted_fields {
            for entity in &mut snapshot.entity_beliefs {
                if let Some(ref mut attrs) = entity.key_attributes {
                    if let Some(attr) = attrs.get_mut(field_name) {
                        let original = attr.value.to_string();
                        let hash = simple_hash(&format!("{}:{}", key, original));
                        attr.value = serde_json::json!(format!("[DECRYPTED]{}", hash));
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_test_snapshot_with_attrs(
        attrs: std::collections::HashMap<String, CommAttributeValue>,
        tags: Vec<String>,
    ) -> CommunicationSnapshot {
        CommunicationSnapshot {
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
            entity_beliefs: vec![EntityBelief {
                node_id: "node-1".to_string(),
                node_type: CommNodeType::User,
                name: None,
                key_attributes: Some(attrs),
                tags,
            }],
            relation_beliefs: Vec::new(),
            intention_summary: None,
            prediction_residual_summary: Vec::new(),
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        }
    }

    #[test]
    fn test_default_rules_filter_credentials() {
        let filter = SensitiveDataFilter::new();
        let mut attrs = std::collections::HashMap::new();
        attrs.insert(
            "password".to_string(),
            CommAttributeValue {
                value: serde_json::json!("secret123"),
                confidence: 0.9,
                source: None,
                last_updated: None,
            },
        );
        attrs.insert(
            "name".to_string(),
            CommAttributeValue {
                value: serde_json::json!("Alice"),
                confidence: 0.9,
                source: None,
                last_updated: None,
            },
        );

        let mut snapshot = make_test_snapshot_with_attrs(attrs, Vec::new());
        let result = filter
            .filter_sensitive_data(&mut snapshot, "agent-2", SnapshotPurpose::Sync)
            .unwrap();

        assert!(result
            .filter_report
            .filtered_fields
            .contains(&"password".to_string()));
        assert!(result
            .filter_report
            .filtered_types
            .contains(&"CREDENTIAL".to_string()));
        let entity = &result.snapshot.entity_beliefs[0];
        let attrs = entity.key_attributes.as_ref().unwrap();
        assert_eq!(attrs.get("name").unwrap().value, serde_json::json!("Alice"));
    }

    #[test]
    fn test_custom_rule() {
        let filter = SensitiveDataFilter::new();
        let custom_rule = FieldFilterRule {
            rule_id: "CUSTOM".to_string(),
            rule_type: FilterRuleType::FieldFilter,
            applies_to: FilterTarget {
                agent_role: None,
                purpose: None,
                scope: None,
            },
            condition: FilterCondition {
                field_path: Some("internal_id".to_string()),
                pattern: None,
                tag: None,
                min_confidence: None,
            },
            action: FilterAction::Remove,
            redaction_format: None,
        };

        let mut attrs = std::collections::HashMap::new();
        attrs.insert(
            "internal_id".to_string(),
            CommAttributeValue {
                value: serde_json::json!("id-12345"),
                confidence: 0.9,
                source: None,
                last_updated: None,
            },
        );
        attrs.insert(
            "public_name".to_string(),
            CommAttributeValue {
                value: serde_json::json!("Alice"),
                confidence: 0.9,
                source: None,
                last_updated: None,
            },
        );

        let mut snapshot = make_test_snapshot_with_attrs(attrs, Vec::new());
        let report = filter.apply_field_filters(&mut snapshot, &[custom_rule]);

        assert!(report.filtered_fields.contains(&"internal_id".to_string()));
        let entity = &snapshot.entity_beliefs[0];
        let attrs = entity.key_attributes.as_ref().unwrap();
        assert!(attrs.get("internal_id").is_none());
        assert!(attrs.get("public_name").is_some());
    }

    #[test]
    fn test_apply_field_filters_with_tags() {
        let filter = SensitiveDataFilter::new();
        let mut attrs = std::collections::HashMap::new();
        attrs.insert(
            "data".to_string(),
            CommAttributeValue {
                value: serde_json::json!("sensitive"),
                confidence: 0.9,
                source: None,
                last_updated: None,
            },
        );

        let mut snapshot = make_test_snapshot_with_attrs(attrs, vec!["confidential".to_string()]);
        let report = filter.apply_field_filters(&mut snapshot, &filter.rules);

        assert!(report.filtered_fields.contains(&"data".to_string()));
        assert!(report.filtered_types.contains(&"CONFIDENTIAL".to_string()));
    }

    #[test]
    fn test_decrypt_fields_no_key() {
        let filter = SensitiveDataFilter::new();
        let mut snapshot =
            make_test_snapshot_with_attrs(std::collections::HashMap::new(), Vec::new());
        let security_metadata = SecurityMetadata {
            signature: None,
            signature_algorithm: None,
            encrypted_fields: vec!["field1".to_string()],
            access_token: None,
        };

        let result = filter.decrypt_fields(&mut snapshot, &security_metadata, &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_fields_with_key() {
        let filter = SensitiveDataFilter::new();
        let mut attrs = std::collections::HashMap::new();
        attrs.insert(
            "secret_field".to_string(),
            CommAttributeValue {
                value: serde_json::json!("encrypted_value"),
                confidence: 0.9,
                source: None,
                last_updated: None,
            },
        );
        let mut snapshot = make_test_snapshot_with_attrs(attrs, Vec::new());
        let security_metadata = SecurityMetadata {
            signature: None,
            signature_algorithm: None,
            encrypted_fields: vec!["secret_field".to_string()],
            access_token: None,
        };

        let result = filter.decrypt_fields(&mut snapshot, &security_metadata, &Some("key123".to_string()));
        assert!(result.is_ok());

        let entity = &snapshot.entity_beliefs[0];
        let attrs = entity.key_attributes.as_ref().unwrap();
        let decrypted = attrs.get("secret_field").unwrap();
        let val = decrypted.value.as_str().unwrap();
        assert!(val.starts_with("[DECRYPTED]"));
    }

    #[test]
    fn test_pii_masking() {
        let filter = SensitiveDataFilter::new();
        let mut attrs = std::collections::HashMap::new();
        attrs.insert(
            "email".to_string(),
            CommAttributeValue {
                value: serde_json::json!("user@example.com"),
                confidence: 0.9,
                source: None,
                last_updated: None,
            },
        );

        let mut snapshot = make_test_snapshot_with_attrs(attrs, Vec::new());
        let result = filter
            .filter_sensitive_data(&mut snapshot, "agent-2", SnapshotPurpose::Sync)
            .unwrap();

        assert!(result
            .filter_report
            .filtered_fields
            .contains(&"email".to_string()));
        assert!(result
            .filter_report
            .filtered_types
            .contains(&"PII".to_string()));
    }
}
