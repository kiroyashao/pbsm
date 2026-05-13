use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use uuid::Uuid;

use crate::modules::belief_graph::graph::BeliefGraph;
use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::{
    comm_edge_type_from_relation, comm_node_type_from_belief, CommAttributeValue,
    CommSnapshotScope, CommunicationSnapshot, ConstructedSnapshot, ConstructionOptions,
    ConstructionReport, EntityBelief, EntityReference, IntentionSummary, Priority,
    RelationBelief, ResidualDetail, SnapshotMetadata as CommSnapshotMetadata, SnapshotPurpose,
    SourceAgent,
};

use super::filter::FieldFilter;

pub struct SnapshotConstructor {
    belief_graph: Arc<BeliefGraph>,
    agent_id: String,
    agent_type: String,
    capabilities: Vec<String>,
    /// 存储可选的意图摘要数据
    intention_data: Option<IntentionSummary>,
    /// 存储预测残差明细
    prediction_residuals: Vec<ResidualDetail>,
}

impl SnapshotConstructor {
    pub fn new(
        belief_graph: Arc<BeliefGraph>,
        agent_id: String,
        agent_type: String,
        capabilities: Vec<String>,
    ) -> Self {
        Self {
            belief_graph,
            agent_id,
            agent_type,
            capabilities,
            intention_data: None,
            prediction_residuals: vec![],
        }
    }

    pub fn with_intention_summary(mut self, summary: IntentionSummary) -> Self {
        self.intention_data = Some(summary);
        self
    }

    pub fn with_prediction_residuals(mut self, residuals: Vec<ResidualDetail>) -> Self {
        self.prediction_residuals = residuals;
        self
    }

    pub fn construct_snapshot(
        &self,
        scope: CommSnapshotScope,
        purpose: SnapshotPurpose,
        options: Option<ConstructionOptions>,
    ) -> Result<ConstructedSnapshot, CommunicationError> {
        self.construct_snapshot_for_agent(scope, purpose, "", options)
    }

    pub fn construct_snapshot_for_agent(
        &self,
        scope: CommSnapshotScope,
        purpose: SnapshotPurpose,
        target_agent: &str,
        options: Option<ConstructionOptions>,
    ) -> Result<ConstructedSnapshot, CommunicationError> {
        let start = Instant::now();
        let opts = options.unwrap_or_default();
        let snapshot_id = Uuid::new_v4().to_string();

        let nodes = self.belief_graph.nodes().read();
        let edges = self.belief_graph.edges().read();

        let mut entity_beliefs = Vec::new();
        let mut relation_beliefs = Vec::new();
        let mut entities_filtered = 0usize;
        let mut relations_filtered = 0usize;

        for node in nodes.values() {
            if let Some(comm_type) = comm_node_type_from_belief(node.node_type) {
                if !scope.entity_types.is_empty() && !scope.entity_types.contains(&comm_type) {
                    entities_filtered += 1;
                    continue;
                }

                if !scope.topics.is_empty() {
                    let has_matching_tag =
                        node.metadata.tags.iter().any(|t| scope.topics.contains(t));
                    if !has_matching_tag {
                        entities_filtered += 1;
                        continue;
                    }
                }

                if let Some(max_nodes) = scope.max_nodes {
                    if entity_beliefs.len() >= max_nodes {
                        entities_filtered += 1;
                        continue;
                    }
                }

                let key_attributes: std::collections::HashMap<String, CommAttributeValue> = node
                    .attributes
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            CommAttributeValue {
                                value: v.value.clone(),
                                confidence: v.confidence,
                                source: Some(v.source.clone()),
                                last_updated: Some(v.last_updated),
                            },
                        )
                    })
                    .collect();

                let entity = EntityBelief {
                    node_id: node.node_id.to_string(),
                    node_type: comm_type,
                    name: Some(node.name.clone()),
                    key_attributes: if key_attributes.is_empty() {
                        None
                    } else {
                        Some(key_attributes)
                    },
                    tags: node.metadata.tags.clone(),
                };

                entity_beliefs.push(entity);
            } else {
                entities_filtered += 1;
            }
        }

        let node_id_set: std::collections::HashSet<String> =
            entity_beliefs.iter().map(|e| e.node_id.clone()).collect();

        for edge in edges.values() {
            if let Some(comm_edge) = comm_edge_type_from_relation(edge.edge_type) {
                if !scope.relationship_types.is_empty()
                    && !scope.relationship_types.contains(&comm_edge)
                {
                    relations_filtered += 1;
                    continue;
                }

                let source_id = edge.source_node.to_string();
                let target_id = edge.target_node.to_string();

                if !node_id_set.contains(&source_id) || !node_id_set.contains(&target_id) {
                    relations_filtered += 1;
                    continue;
                }

                let source_name = nodes.get(&edge.source_node).map(|n| n.name.clone());
                let target_name = nodes.get(&edge.target_node).map(|n| n.name.clone());

                let relation = RelationBelief {
                    edge_id: edge.edge_id.to_string(),
                    edge_type: comm_edge,
                    source_entity: EntityReference {
                        node_id: source_id,
                        name: source_name,
                    },
                    target_entity: EntityReference {
                        node_id: target_id,
                        name: target_name,
                    },
                    attributes: if edge.attributes.is_empty() {
                        None
                    } else {
                        Some(edge.attributes.clone())
                    },
                    confidence: edge.confidence,
                    directional: edge.metadata.is_directional,
                };

                relation_beliefs.push(relation);
            } else {
                relations_filtered += 1;
            }
        }

        drop(nodes);
        drop(edges);

        let intention_summary = self.intention_data.clone();
        let prediction_residual_summary = self.prediction_residuals.clone();

        let mut snapshot = CommunicationSnapshot {
            snapshot_id: snapshot_id.clone(),
            snapshot_metadata: CommSnapshotMetadata {
                version: "1.0".to_string(),
                timestamp: Utc::now(),
                source_agent: SourceAgent {
                    agent_id: self.agent_id.clone(),
                    agent_type: Some(self.agent_type.clone()),
                    capabilities: self.capabilities.clone(),
                },
                scope: scope.clone(),
                purpose,
                priority: opts.priority.unwrap_or(Priority::Normal),
                expires_at: opts
                    .ttl
                    .map(|ttl| Utc::now() + chrono::Duration::seconds(ttl as i64)),
            },
            entity_beliefs,
            relation_beliefs,
            intention_summary,
            prediction_residual_summary,
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        };

        if !target_agent.is_empty() {
            let filter = FieldFilter::new();
            filter.apply_to_snapshot(&mut snapshot);
        }

        let nodes_included = snapshot.entity_beliefs.len();
        let relations_included = snapshot.relation_beliefs.len();
        let processing_time_ms = start.elapsed().as_millis() as u64;

        let compression_ratio = if let Some(ref compression_info) = snapshot.compression_info {
            if compression_info.original_size > 0 {
                Some(
                    compression_info.compressed_size as f64 / compression_info.original_size as f64,
                )
            } else {
                None
            }
        } else {
            None
        };

        let construction_report = ConstructionReport {
            nodes_included,
            relations_included,
            entities_filtered,
            relations_filtered,
            compression_ratio,
            processing_time_ms,
            warnings: vec![],
        };

        Ok(ConstructedSnapshot {
            snapshot_id,
            snapshot,
            construction_report,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::belief_graph::operations::BeliefGraphOperations;
    use crate::modules::belief_graph::types::{BeliefNodeType, SourceType};
    use crate::modules::communication::types::CommNodeType;
    use std::collections::HashMap;

    fn create_test_constructor() -> SnapshotConstructor {
        let graph = Arc::new(BeliefGraph::with_default_config());

        BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "Alice".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            Some(vec!["important".to_string()]),
            None,
        )
        .unwrap();

        BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::File,
            "doc.txt".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::DirectObservation,
            None,
            None,
        )
        .unwrap();

        SnapshotConstructor::new(
            graph,
            "agent-001".to_string(),
            "coordinator".to_string(),
            vec!["query".to_string()],
        )
    }

    #[test]
    fn test_basic_construction() {
        let constructor = create_test_constructor();
        let scope = CommSnapshotScope::default();
        let result = constructor.construct_snapshot(scope, SnapshotPurpose::Sync, None);

        assert!(result.is_ok());
        let constructed = result.unwrap();
        assert!(!constructed.snapshot.snapshot_id.is_empty());
        assert_eq!(constructed.snapshot.entity_beliefs.len(), 2);
        assert_eq!(constructed.construction_report.nodes_included, 2);
    }

    #[test]
    fn test_empty_graph() {
        let graph = Arc::new(BeliefGraph::with_default_config());
        let constructor = SnapshotConstructor::new(
            graph,
            "agent-empty".to_string(),
            "coordinator".to_string(),
            vec![],
        );

        let scope = CommSnapshotScope::default();
        let result = constructor.construct_snapshot(scope, SnapshotPurpose::Query, None);

        assert!(result.is_ok());
        let constructed = result.unwrap();
        assert!(constructed.snapshot.entity_beliefs.is_empty());
        assert!(constructed.snapshot.relation_beliefs.is_empty());
        assert_eq!(constructed.construction_report.nodes_included, 0);
    }

    #[test]
    fn test_with_scope_filtering() {
        let constructor = create_test_constructor();
        let scope = CommSnapshotScope {
            entity_types: vec![CommNodeType::User],
            ..CommSnapshotScope::default()
        };

        let result = constructor.construct_snapshot(scope, SnapshotPurpose::Query, None);

        assert!(result.is_ok());
        let constructed = result.unwrap();
        assert_eq!(constructed.snapshot.entity_beliefs.len(), 1);
        assert_eq!(
            constructed.snapshot.entity_beliefs[0].node_type,
            CommNodeType::User
        );
        assert!(constructed.construction_report.entities_filtered > 0);
    }
}
