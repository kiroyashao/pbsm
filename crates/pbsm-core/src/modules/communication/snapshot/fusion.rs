use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use crate::modules::belief_graph::graph::BeliefGraph;
use crate::modules::belief_graph::operations::BeliefGraphOperations;
use crate::modules::belief_graph::types::{AttributeValue, SourceType, UpdateStrategy};
use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::{
    belief_node_type_from_comm, relation_edge_type_from_comm, CommunicationSnapshot,
    ConflictResolutionStrategy, EntityBelief, EntityFusionResult, FusionAction, FusionChanges,
    FusionMetrics, FusionOptions, FusionResult as CommFusionResult, MappingType, NodeMapping,
    RelationFusionAction, RelationFusionResult,
};

pub struct SnapshotFusion {
    belief_graph: Arc<BeliefGraph>,
}

impl SnapshotFusion {
    pub fn new(belief_graph: Arc<BeliefGraph>) -> Self {
        Self { belief_graph }
    }

    pub fn fuse_snapshot(
        &self,
        snapshot: &CommunicationSnapshot,
        fusion_options: Option<FusionOptions>,
    ) -> Result<CommFusionResult, CommunicationError> {
        self.fuse_snapshot_with_graph(snapshot, "", fusion_options)
    }

    pub fn fuse_snapshot_with_graph(
        &self,
        snapshot: &CommunicationSnapshot,
        target_agent: &str,
        fusion_options: Option<FusionOptions>,
    ) -> Result<CommFusionResult, CommunicationError> {
        let start = Instant::now();
        let opts = fusion_options.unwrap_or(FusionOptions {
            conflict_resolution_strategy: None,
            auto_merge_threshold: None,
            max_conflicts: None,
            preserve_local_history: None,
            trigger_metacognition: None,
        });
        let strategy = opts
            .conflict_resolution_strategy
            .unwrap_or(ConflictResolutionStrategy::AutoMerge);
        let auto_merge_threshold = opts.auto_merge_threshold.unwrap_or(0.7);

        let mut node_mappings: Vec<NodeMapping> = Vec::new();
        let mut entity_fusion_results: Vec<EntityFusionResult> = Vec::new();
        let mut relation_fusion_results: Vec<RelationFusionResult> = Vec::new();
        let mut triggered_residuals: Vec<String> = Vec::new();

        let mut nodes_added = 0usize;
        let mut nodes_updated = 0usize;
        let mut nodes_skipped = 0usize;
        let mut conflicts_detected = 0usize;
        let mut conflicts_resolved = 0usize;

        let local_nodes = self.belief_graph.nodes().read();
        let mut local_name_index: std::collections::HashMap<String, uuid::Uuid> =
            std::collections::HashMap::new();
        let mut local_id_set: std::collections::HashSet<uuid::Uuid> =
            std::collections::HashSet::new();

        for (id, node) in local_nodes.iter() {
            local_name_index.insert(node.name.clone(), *id);
            local_id_set.insert(*id);
        }
        drop(local_nodes);

        let is_self_fusion = target_agent == snapshot.snapshot_metadata.source_agent.agent_id;

        for entity in &snapshot.entity_beliefs {
            let parsed_uuid = uuid::Uuid::parse_str(&entity.node_id).ok();
            let local_node_id = if let Some(uid) = parsed_uuid {
                if local_id_set.contains(&uid) {
                    Some(uid)
                } else {
                    local_name_index
                        .get(entity.name.as_deref().unwrap_or(""))
                        .copied()
                }
            } else {
                local_name_index
                    .get(entity.name.as_deref().unwrap_or(""))
                    .copied()
            };

            if let Some(local_id) = local_node_id {
                let has_conflict = self.check_entity_conflict(local_id, entity);

                if has_conflict {
                    conflicts_detected += 1;

                    if is_self_fusion {
                        let _ =
                            self.merge_entity(local_id, entity, ConflictResolutionStrategy::LastWriteWins, auto_merge_threshold);
                        conflicts_resolved += 1;
                        nodes_updated += 1;

                        let (added, updated, unchanged) =
                            self.compute_attribute_changes(local_id, entity);

                        entity_fusion_results.push(EntityFusionResult {
                            node_id: entity.node_id.clone(),
                            action: FusionAction::Update,
                            changes: FusionChanges {
                                added_attributes: added,
                                updated_attributes: updated,
                                unchanged_attributes: unchanged,
                            },
                            confidence_after_fusion: self
                                .compute_merged_confidence(local_id, entity),
                        });

                        node_mappings.push(NodeMapping {
                            external_node_id: entity.node_id.clone(),
                            local_node_id: Some(local_id.to_string()),
                            mapping_type: MappingType::IdMatch,
                        });
                        continue;
                    }

                    match strategy {
                        ConflictResolutionStrategy::AutoMerge
                        | ConflictResolutionStrategy::ConfidenceBased => {
                            let merged =
                                self.merge_entity(local_id, entity, strategy, auto_merge_threshold);
                            if merged {
                                conflicts_resolved += 1;
                                nodes_updated += 1;

                                let (added, updated, unchanged) =
                                    self.compute_attribute_changes(local_id, entity);

                                entity_fusion_results.push(EntityFusionResult {
                                    node_id: entity.node_id.clone(),
                                    action: FusionAction::Update,
                                    changes: FusionChanges {
                                        added_attributes: added,
                                        updated_attributes: updated,
                                        unchanged_attributes: unchanged,
                                    },
                                    confidence_after_fusion: self
                                        .compute_merged_confidence(local_id, entity),
                                });

                                node_mappings.push(NodeMapping {
                                    external_node_id: entity.node_id.clone(),
                                    local_node_id: Some(local_id.to_string()),
                                    mapping_type: MappingType::IdMatch,
                                });
                            } else {
                                nodes_skipped += 1;
                                entity_fusion_results.push(EntityFusionResult {
                                    node_id: entity.node_id.clone(),
                                    action: FusionAction::Conflict,
                                    changes: FusionChanges {
                                        added_attributes: vec![],
                                        updated_attributes: vec![],
                                        unchanged_attributes: vec![],
                                    },
                                    confidence_after_fusion: 0.0,
                                });
                                triggered_residuals.push(format!(
                                    "Unresolved conflict for node {}",
                                    entity.node_id
                                ));
                            }
                        }
                        ConflictResolutionStrategy::LastWriteWins => {
                            let _ =
                                self.merge_entity(local_id, entity, strategy, auto_merge_threshold);
                            conflicts_resolved += 1;
                            nodes_updated += 1;

                            let (added, updated, unchanged) =
                                self.compute_attribute_changes(local_id, entity);

                            entity_fusion_results.push(EntityFusionResult {
                                node_id: entity.node_id.clone(),
                                action: FusionAction::Update,
                                changes: FusionChanges {
                                    added_attributes: added,
                                    updated_attributes: updated,
                                    unchanged_attributes: unchanged,
                                },
                                confidence_after_fusion: self
                                    .compute_merged_confidence(local_id, entity),
                            });

                            node_mappings.push(NodeMapping {
                                external_node_id: entity.node_id.clone(),
                                local_node_id: Some(local_id.to_string()),
                                mapping_type: MappingType::IdMatch,
                            });
                        }
                        ConflictResolutionStrategy::AuthorityRuling
                        | ConflictResolutionStrategy::Negotiate => {
                            nodes_skipped += 1;
                            entity_fusion_results.push(EntityFusionResult {
                                node_id: entity.node_id.clone(),
                                action: FusionAction::Conflict,
                                changes: FusionChanges {
                                    added_attributes: vec![],
                                    updated_attributes: vec![],
                                    unchanged_attributes: vec![],
                                },
                                confidence_after_fusion: 0.0,
                            });
                            triggered_residuals.push(format!(
                                "Conflict requires negotiation for node {}",
                                entity.node_id
                            ));
                        }
                    }
                } else {
                    let _ = self.merge_entity(local_id, entity, strategy, auto_merge_threshold);
                    nodes_updated += 1;

                    let (added, updated, unchanged) =
                        self.compute_attribute_changes(local_id, entity);

                    entity_fusion_results.push(EntityFusionResult {
                        node_id: entity.node_id.clone(),
                        action: FusionAction::Update,
                        changes: FusionChanges {
                            added_attributes: added,
                            updated_attributes: updated,
                            unchanged_attributes: unchanged,
                        },
                        confidence_after_fusion: self.compute_merged_confidence(local_id, entity),
                    });

                    node_mappings.push(NodeMapping {
                        external_node_id: entity.node_id.clone(),
                        local_node_id: Some(local_id.to_string()),
                        mapping_type: MappingType::IdMatch,
                    });
                }
            } else {
                let belief_type = belief_node_type_from_comm(entity.node_type);
                let name = entity
                    .name
                    .clone()
                    .unwrap_or_else(|| entity.node_id.clone());

                let mut attributes = std::collections::HashMap::new();
                if let Some(ref key_attrs) = entity.key_attributes {
                    for (key, val) in key_attrs {
                        attributes.insert(
                            key.clone(),
                            AttributeValue::new(
                                val.value.clone(),
                                val.confidence,
                                val.source.clone().unwrap_or_else(|| "external".to_string()),
                                SourceType::AgentSync,
                            ),
                        );
                    }
                }

                let tags = if entity.tags.is_empty() {
                    None
                } else {
                    Some(entity.tags.clone())
                };

                match BeliefGraphOperations::create_belief(
                    &self.belief_graph,
                    belief_type,
                    name,
                    attributes,
                    "communication_fusion".to_string(),
                    SourceType::AgentSync,
                    tags,
                    None,
                ) {
                    Ok(new_id) => {
                        nodes_added += 1;
                        entity_fusion_results.push(EntityFusionResult {
                            node_id: entity.node_id.clone(),
                            action: FusionAction::Add,
                            changes: FusionChanges {
                                added_attributes: entity
                                    .key_attributes
                                    .as_ref()
                                    .map(|a| a.keys().cloned().collect())
                                    .unwrap_or_default(),
                                updated_attributes: vec![],
                                unchanged_attributes: vec![],
                            },
                            confidence_after_fusion: entity
                                .key_attributes
                                .as_ref()
                                .map(|a| {
                                    a.values().map(|v| v.confidence).sum::<f64>()
                                        / a.len().max(1) as f64
                                })
                                .unwrap_or(0.5),
                        });

                        node_mappings.push(NodeMapping {
                            external_node_id: entity.node_id.clone(),
                            local_node_id: Some(new_id.to_string()),
                            mapping_type: MappingType::Created,
                        });
                    }
                    Err(_) => {
                        nodes_skipped += 1;
                        entity_fusion_results.push(EntityFusionResult {
                            node_id: entity.node_id.clone(),
                            action: FusionAction::Skip,
                            changes: FusionChanges {
                                added_attributes: vec![],
                                updated_attributes: vec![],
                                unchanged_attributes: vec![],
                            },
                            confidence_after_fusion: 0.0,
                        });
                    }
                }
            }
        }

        for relation in &snapshot.relation_beliefs {
            let source_parsed = uuid::Uuid::parse_str(&relation.source_entity.node_id).ok();
            let target_parsed = uuid::Uuid::parse_str(&relation.target_entity.node_id).ok();

            match (source_parsed, target_parsed) {
                (Some(source_id), Some(target_id)) => {
                    if let Some(comm_edge_type) = relation_edge_type_from_comm(relation.edge_type) {
                        match BeliefGraphOperations::create_edge(
                            &self.belief_graph,
                            comm_edge_type,
                            source_id,
                            target_id,
                            relation.confidence,
                        ) {
                            Ok(_) => {
                                relation_fusion_results.push(RelationFusionResult {
                                    edge_id: relation.edge_id.clone(),
                                    action: RelationFusionAction::Add,
                                });
                            }
                            Err(_) => {
                                relation_fusion_results.push(RelationFusionResult {
                                    edge_id: relation.edge_id.clone(),
                                    action: RelationFusionAction::Skip,
                                });
                            }
                        }
                    } else {
                        relation_fusion_results.push(RelationFusionResult {
                            edge_id: relation.edge_id.clone(),
                            action: RelationFusionAction::Skip,
                        });
                    }
                }
                _ => {
                    relation_fusion_results.push(RelationFusionResult {
                        edge_id: relation.edge_id.clone(),
                        action: RelationFusionAction::Skip,
                    });
                }
            }
        }

        let processing_time_ms = start.elapsed().as_millis() as u64;

        Ok(CommFusionResult {
            fusion_id: Uuid::new_v4().to_string(),
            snapshot_id: snapshot.snapshot_id.clone(),
            node_mappings,
            entity_fusion_results,
            relation_fusion_results,
            triggered_residuals,
            metrics: FusionMetrics {
                nodes_added,
                nodes_updated,
                nodes_skipped,
                conflicts_detected,
                conflicts_resolved,
                processing_time_ms,
            },
        })
    }

    fn check_entity_conflict(&self, local_id: uuid::Uuid, entity: &EntityBelief) -> bool {
        let local_node = self.belief_graph.get_node(local_id);
        if let Some(local) = local_node {
            if let Some(ref key_attrs) = entity.key_attributes {
                for (key, val) in key_attrs {
                    if let Some(local_attr) = local.attributes.get(key) {
                        if local_attr.value != val.value {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn merge_entity(
        &self,
        local_id: uuid::Uuid,
        entity: &EntityBelief,
        strategy: ConflictResolutionStrategy,
        auto_merge_threshold: f64,
    ) -> bool {
        let local_node = self.belief_graph.get_node(local_id);
        if let Some(local) = local_node {
            let mut updates = std::collections::HashMap::new();

            if let Some(ref key_attrs) = entity.key_attributes {
                for (key, val) in key_attrs {
                    let should_update = match strategy {
                        ConflictResolutionStrategy::AutoMerge => true,
                        ConflictResolutionStrategy::ConfidenceBased => {
                            if let Some(local_attr) = local.attributes.get(key) {
                                val.confidence >= local_attr.confidence
                            } else {
                                true
                            }
                        }
                        ConflictResolutionStrategy::LastWriteWins => true,
                        _ => val.confidence >= auto_merge_threshold,
                    };

                    if should_update {
                        updates.insert(
                            key.clone(),
                            AttributeValue::new(
                                val.value.clone(),
                                val.confidence,
                                val.source.clone().unwrap_or_else(|| "external".to_string()),
                                SourceType::AgentSync,
                            ),
                        );
                    }
                }
            }

            if updates.is_empty() {
                return false;
            }

            let update_strategy = match strategy {
                ConflictResolutionStrategy::ConfidenceBased => UpdateStrategy::ConditionalReplace,
                _ => UpdateStrategy::ConditionalReplace,
            };

            BeliefGraphOperations::update_belief(
                &self.belief_graph,
                local_id,
                updates,
                update_strategy,
            )
            .is_ok()
        } else {
            false
        }
    }

    fn compute_attribute_changes(
        &self,
        local_id: uuid::Uuid,
        entity: &EntityBelief,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let local_node = self.belief_graph.get_node(local_id);
        let mut added = Vec::new();
        let mut updated = Vec::new();
        let mut unchanged = Vec::new();

        if let Some(local) = local_node {
            if let Some(ref key_attrs) = entity.key_attributes {
                for key in key_attrs.keys() {
                    if let Some(local_attr) = local.attributes.get(key) {
                        if let Some(val) = key_attrs.get(key) {
                            if local_attr.value != val.value {
                                updated.push(key.clone());
                            } else {
                                unchanged.push(key.clone());
                            }
                        }
                    } else {
                        added.push(key.clone());
                    }
                }
            }
        }

        (added, updated, unchanged)
    }

    fn compute_merged_confidence(&self, local_id: uuid::Uuid, entity: &EntityBelief) -> f64 {
        let local_node = self.belief_graph.get_node(local_id);
        if let Some(local) = local_node {
            let local_conf = local.average_confidence();
            let entity_conf = entity
                .key_attributes
                .as_ref()
                .map(|a| a.values().map(|v| v.confidence).sum::<f64>() / a.len().max(1) as f64)
                .unwrap_or(0.5);
            (local_conf + entity_conf) / 2.0
        } else {
            0.5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::belief_graph::types::{BeliefNodeType, SourceType};
    use crate::modules::communication::types::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_graph_with_nodes() -> Arc<BeliefGraph> {
        let graph = Arc::new(BeliefGraph::with_default_config());

        BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "Alice".to_string(),
            {
                let mut attrs = HashMap::new();
                attrs.insert(
                    "role".to_string(),
                    AttributeValue::new(
                        serde_json::json!("admin"),
                        0.9,
                        "local".to_string(),
                        SourceType::DirectObservation,
                    ),
                );
                attrs
            },
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();

        graph
    }

    fn create_snapshot_with_new_nodes() -> CommunicationSnapshot {
        CommunicationSnapshot {
            snapshot_id: "fusion-test-001".to_string(),
            snapshot_metadata: crate::modules::communication::types::SnapshotMetadata {
                version: "1.0".to_string(),
                timestamp: Utc::now(),
                source_agent: crate::modules::communication::types::SourceAgent {
                    agent_id: "agent-external".to_string(),
                    agent_type: None,
                    capabilities: vec![],
                },
                scope: crate::modules::communication::types::CommSnapshotScope::default(),
                purpose: crate::modules::communication::types::SnapshotPurpose::Sync,
                priority: crate::modules::communication::types::Priority::Normal,
                expires_at: None,
            },
            entity_beliefs: vec![EntityBelief {
                node_id: uuid::Uuid::new_v4().to_string(),
                node_type: crate::modules::communication::types::CommNodeType::File,
                name: Some("new_file.txt".to_string()),
                key_attributes: Some({
                    let mut map = HashMap::new();
                    map.insert(
                        "size".to_string(),
                        crate::modules::communication::types::CommAttributeValue {
                            value: serde_json::json!(1024),
                            confidence: 0.8,
                            source: None,
                            last_updated: None,
                        },
                    );
                    map
                }),
                tags: vec![],
            }],
            relation_beliefs: vec![],
            intention_summary: None,
            prediction_residual_summary: vec![],
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        }
    }

    fn create_snapshot_with_existing_nodes(graph: &BeliefGraph) -> CommunicationSnapshot {
        let nodes = graph.nodes().read();
        let alice_id = nodes
            .values()
            .find(|n| n.name == "Alice")
            .map(|n| n.node_id.to_string())
            .unwrap();
        drop(nodes);

        CommunicationSnapshot {
            snapshot_id: "fusion-test-002".to_string(),
            snapshot_metadata: crate::modules::communication::types::SnapshotMetadata {
                version: "1.0".to_string(),
                timestamp: Utc::now(),
                source_agent: crate::modules::communication::types::SourceAgent {
                    agent_id: "agent-external".to_string(),
                    agent_type: None,
                    capabilities: vec![],
                },
                scope: crate::modules::communication::types::CommSnapshotScope::default(),
                purpose: crate::modules::communication::types::SnapshotPurpose::Sync,
                priority: crate::modules::communication::types::Priority::Normal,
                expires_at: None,
            },
            entity_beliefs: vec![EntityBelief {
                node_id: alice_id.clone(),
                node_type: crate::modules::communication::types::CommNodeType::User,
                name: Some("Alice".to_string()),
                key_attributes: Some({
                    let mut map = HashMap::new();
                    map.insert(
                        "role".to_string(),
                        crate::modules::communication::types::CommAttributeValue {
                            value: serde_json::json!("user"),
                            confidence: 0.95,
                            source: None,
                            last_updated: None,
                        },
                    );
                    map
                }),
                tags: vec![],
            }],
            relation_beliefs: vec![],
            intention_summary: None,
            prediction_residual_summary: vec![],
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        }
    }

    #[test]
    fn test_fuse_with_new_nodes() {
        let graph = Arc::new(BeliefGraph::with_default_config());
        let fusion = SnapshotFusion::new(graph.clone());

        let snapshot = create_snapshot_with_new_nodes();
        let result = fusion.fuse_snapshot(&snapshot, None);

        assert!(result.is_ok());
        let fusion_result = result.unwrap();
        assert_eq!(fusion_result.metrics.nodes_added, 1);
        assert_eq!(fusion_result.metrics.nodes_updated, 0);
    }

    #[test]
    fn test_fuse_with_existing_nodes_merge() {
        let graph = create_test_graph_with_nodes();
        let fusion = SnapshotFusion::new(graph.clone());

        let snapshot = create_snapshot_with_existing_nodes(&graph);
        let opts = FusionOptions {
            conflict_resolution_strategy: Some(ConflictResolutionStrategy::AutoMerge),
            auto_merge_threshold: None,
            max_conflicts: None,
            preserve_local_history: None,
            trigger_metacognition: None,
        };

        let result = fusion.fuse_snapshot(&snapshot, Some(opts));

        assert!(result.is_ok());
        let fusion_result = result.unwrap();
        assert!(fusion_result.metrics.nodes_updated >= 1);
    }

    #[test]
    fn test_fuse_with_conflicts() {
        let graph = create_test_graph_with_nodes();
        let fusion = SnapshotFusion::new(graph.clone());

        let snapshot = create_snapshot_with_existing_nodes(&graph);
        let opts = FusionOptions {
            conflict_resolution_strategy: Some(ConflictResolutionStrategy::Negotiate),
            auto_merge_threshold: None,
            max_conflicts: None,
            preserve_local_history: None,
            trigger_metacognition: None,
        };

        let result = fusion.fuse_snapshot(&snapshot, Some(opts));

        assert!(result.is_ok());
        let fusion_result = result.unwrap();
        assert!(fusion_result.metrics.conflicts_detected > 0);
    }
}
