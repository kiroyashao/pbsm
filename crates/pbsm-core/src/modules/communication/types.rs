use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::modules::belief_graph::types::{BeliefNodeType, RelationEdgeType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationSnapshot {
    pub snapshot_id: String,
    pub snapshot_metadata: SnapshotMetadata,
    #[serde(default)]
    pub entity_beliefs: Vec<EntityBelief>,
    #[serde(default)]
    pub relation_beliefs: Vec<RelationBelief>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intention_summary: Option<IntentionSummary>,
    #[serde(default)]
    pub prediction_residual_summary: Vec<ResidualDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegation_context: Option<DelegationContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_metadata: Option<SecurityMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_info: Option<CompressionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub version: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub source_agent: SourceAgent,
    pub scope: CommSnapshotScope,
    pub purpose: SnapshotPurpose,
    pub priority: Priority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceAgent {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommSnapshotScope {
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub entity_types: Vec<CommNodeType>,
    #[serde(default)]
    pub relationship_types: Vec<CommEdgeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_nodes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_intentions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_residuals: Option<Vec<ResidualSeverity>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SnapshotPurpose {
    Query,
    Response,
    Sync,
    Delegate,
    Notify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommNodeType {
    User,
    File,
    Tool,
    Variable,
    Concept,
    Event,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommEdgeType {
    Owns,
    DependsOn,
    Authorizes,
    Calls,
    Contains,
    RelatedTo,
    DelegatesTo,
    SyncsWith,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityBelief {
    pub node_id: String,
    pub node_type: CommNodeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_attributes: Option<HashMap<String, CommAttributeValue>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommAttributeValue {
    pub value: serde_json::Value,
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationBelief {
    pub edge_id: String,
    pub edge_type: CommEdgeType,
    pub source_entity: EntityReference,
    pub target_entity: EntityReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, serde_json::Value>>,
    pub confidence: f64,
    #[serde(default)]
    pub directional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityReference {
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionSummary {
    pub top_goal: TopGoal,
    #[serde(default)]
    pub key_subtasks: Vec<KeySubtask>,
    #[serde(default)]
    pub blockers: Vec<Blocker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_completion_steps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopGoal {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_state: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeySubtask {
    pub subtask_id: String,
    pub description: String,
    pub status: SubtaskStatus,
    pub progress: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubtaskStatus {
    Pending,
    InProgress,
    WaitingFeedback,
    Completed,
    Abandoned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocker {
    pub blocker_id: String,
    pub description: String,
    pub severity: ResidualSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResidualSeverity {
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualDetail {
    pub residual_id: String,
    pub associated_prediction: AssociatedPrediction,
    pub issue: String,
    pub expected_value: serde_json::Value,
    pub actual_value: serde_json::Value,
    pub deviation_degree: f64,
    pub severity: ResidualSeverity,
    #[serde(default)]
    pub affected_nodes: Vec<AffectedNode>,
    #[serde(default)]
    pub hypothesized_causes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssociatedPrediction {
    pub prediction_id: String,
    pub action_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedNode {
    pub node_id: String,
    pub node_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationContext {
    pub delegation_id: String,
    pub task_description: String,
    pub required_capabilities: Vec<String>,
    pub constraints: DelegationConstraints,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_result_format: Option<String>,
    pub fallback_strategy: FallbackStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationConstraints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_threshold: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FallbackStrategy {
    ReturnError,
    UseDefault,
    NotifySender,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_algorithm: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub encrypted_fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionInfo {
    pub algorithm: CompressionAlgorithm,
    pub original_size: usize,
    pub compressed_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompressionAlgorithm {
    None,
    Lz4,
    Zstd,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConstructionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<CompressionAlgorithm>,
    #[serde(default)]
    pub encryption: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructionReport {
    pub nodes_included: usize,
    pub relations_included: usize,
    pub entities_filtered: usize,
    pub relations_filtered: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_ratio: Option<f64>,
    pub processing_time_ms: u64,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructedSnapshot {
    pub snapshot_id: String,
    pub snapshot: CommunicationSnapshot,
    pub construction_report: ConstructionReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParseMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<CompressionAlgorithm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decryption_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSnapshot {
    pub snapshot: CommunicationSnapshot,
    pub verification_result: VerificationResult,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub verification_id: String,
    pub snapshot_id: String,
    pub result: VerificationOutcome,
    pub checks: VerificationChecks,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationOutcome {
    Passed,
    Failed,
    PartialPass,
    Expired,
}

impl VerificationOutcome {
    pub fn is_passed(&self) -> bool {
        matches!(self, VerificationOutcome::Passed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerificationChecks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_validation: Option<FormatValidation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_check: Option<VersionCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_validation: Option<TimestampValidation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_verification: Option<SignatureVerification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity_check: Option<IntegrityCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatValidation {
    pub passed: bool,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCheck {
    pub passed: bool,
    pub snapshot_version: String,
    pub supported_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampValidation {
    pub passed: bool,
    pub age_seconds: u64,
    pub max_age_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureVerification {
    pub passed: bool,
    pub signer_agent: String,
    pub algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheck {
    pub passed: bool,
    pub expected_checksum: String,
    pub actual_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_resolution_strategy: Option<ConflictResolutionStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_merge_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_conflicts: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserve_local_history: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_metacognition: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConflictResolutionStrategy {
    AutoMerge,
    Negotiate,
    AuthorityRuling,
    LastWriteWins,
    ConfidenceBased,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionResult {
    pub fusion_id: String,
    pub snapshot_id: String,
    #[serde(default)]
    pub node_mappings: Vec<NodeMapping>,
    #[serde(default)]
    pub entity_fusion_results: Vec<EntityFusionResult>,
    #[serde(default)]
    pub relation_fusion_results: Vec<RelationFusionResult>,
    #[serde(default)]
    pub triggered_residuals: Vec<String>,
    pub metrics: FusionMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMapping {
    pub external_node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_node_id: Option<String>,
    pub mapping_type: MappingType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MappingType {
    IdMatch,
    NameMatch,
    SimilarityMatch,
    Created,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityFusionResult {
    pub node_id: String,
    pub action: FusionAction,
    pub changes: FusionChanges,
    pub confidence_after_fusion: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FusionAction {
    Add,
    Update,
    Skip,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionChanges {
    #[serde(default)]
    pub added_attributes: Vec<String>,
    #[serde(default)]
    pub updated_attributes: Vec<String>,
    #[serde(default)]
    pub unchanged_attributes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationFusionResult {
    pub edge_id: String,
    pub action: RelationFusionAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationFusionAction {
    Add,
    Update,
    Skip,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionMetrics {
    pub nodes_added: usize,
    pub nodes_updated: usize,
    pub nodes_skipped: usize,
    pub conflicts_detected: usize,
    pub conflicts_resolved: usize,
    pub processing_time_ms: u64,
}

pub fn comm_node_type_from_belief(belief_type: BeliefNodeType) -> Option<CommNodeType> {
    match belief_type {
        BeliefNodeType::User => Some(CommNodeType::User),
        BeliefNodeType::File => Some(CommNodeType::File),
        BeliefNodeType::Tool => Some(CommNodeType::Tool),
        BeliefNodeType::Variable => Some(CommNodeType::Variable),
        BeliefNodeType::Concept => Some(CommNodeType::Concept),
        BeliefNodeType::Event => Some(CommNodeType::Event),
        BeliefNodeType::Agent => Some(CommNodeType::Agent),
        BeliefNodeType::Resource | BeliefNodeType::Process => None,
    }
}

pub fn comm_edge_type_from_relation(relation_type: RelationEdgeType) -> Option<CommEdgeType> {
    match relation_type {
        RelationEdgeType::Owns => Some(CommEdgeType::Owns),
        RelationEdgeType::DependsOn => Some(CommEdgeType::DependsOn),
        RelationEdgeType::Authorizes => Some(CommEdgeType::Authorizes),
        RelationEdgeType::Calls => Some(CommEdgeType::Calls),
        RelationEdgeType::Contains => Some(CommEdgeType::Contains),
        RelationEdgeType::RelatedTo => Some(CommEdgeType::RelatedTo),
        RelationEdgeType::SynchronizesWith => Some(CommEdgeType::SyncsWith),
        RelationEdgeType::Enables
        | RelationEdgeType::Blocks
        | RelationEdgeType::Modifies
        | RelationEdgeType::References
        | RelationEdgeType::Precedes
        | RelationEdgeType::Follows => None,
    }
}

pub fn belief_node_type_from_comm(comm_type: CommNodeType) -> BeliefNodeType {
    match comm_type {
        CommNodeType::User => BeliefNodeType::User,
        CommNodeType::File => BeliefNodeType::File,
        CommNodeType::Tool => BeliefNodeType::Tool,
        CommNodeType::Variable => BeliefNodeType::Variable,
        CommNodeType::Concept => BeliefNodeType::Concept,
        CommNodeType::Event => BeliefNodeType::Event,
        CommNodeType::Agent => BeliefNodeType::Agent,
    }
}

pub fn relation_edge_type_from_comm(comm_type: CommEdgeType) -> Option<RelationEdgeType> {
    match comm_type {
        CommEdgeType::Owns => Some(RelationEdgeType::Owns),
        CommEdgeType::DependsOn => Some(RelationEdgeType::DependsOn),
        CommEdgeType::Authorizes => Some(RelationEdgeType::Authorizes),
        CommEdgeType::Calls => Some(RelationEdgeType::Calls),
        CommEdgeType::Contains => Some(RelationEdgeType::Contains),
        CommEdgeType::RelatedTo => Some(RelationEdgeType::RelatedTo),
        CommEdgeType::DelegatesTo => None,
        CommEdgeType::SyncsWith => Some(RelationEdgeType::SynchronizesWith),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::belief_graph::types::{BeliefNodeType, RelationEdgeType};

    #[test]
    fn test_communication_snapshot_serialization_roundtrip() {
        let snapshot = CommunicationSnapshot {
            snapshot_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            snapshot_metadata: SnapshotMetadata {
                version: "1.0.0".to_string(),
                timestamp: Utc::now(),
                source_agent: SourceAgent {
                    agent_id: "agent-001".to_string(),
                    agent_type: Some("coordinator".to_string()),
                    capabilities: vec!["plan".to_string(), "delegate".to_string()],
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
                            source: Some("direct".to_string()),
                            last_updated: Some(Utc::now()),
                        },
                    );
                    map
                }),
                tags: vec!["important".to_string()],
            }],
            relation_beliefs: vec![RelationBelief {
                edge_id: "edge-1".to_string(),
                edge_type: CommEdgeType::Owns,
                source_entity: EntityReference {
                    node_id: "node-1".to_string(),
                    name: Some("Alice".to_string()),
                },
                target_entity: EntityReference {
                    node_id: "node-2".to_string(),
                    name: None,
                },
                attributes: None,
                confidence: 0.9,
                directional: true,
            }],
            intention_summary: Some(IntentionSummary {
                top_goal: TopGoal {
                    description: "Complete task".to_string(),
                    target_state: Some(serde_json::json!({"status": "done"})),
                },
                key_subtasks: vec![KeySubtask {
                    subtask_id: "sub-1".to_string(),
                    description: "Step 1".to_string(),
                    status: SubtaskStatus::InProgress,
                    progress: 0.5,
                }],
                blockers: vec![],
                estimated_completion_steps: Some(10),
            }),
            prediction_residual_summary: vec![],
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: CommunicationSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.snapshot_id, snapshot.snapshot_id);
        assert_eq!(deserialized.entity_beliefs.len(), 1);
        assert_eq!(deserialized.entity_beliefs[0].node_type, CommNodeType::User);
        assert_eq!(deserialized.relation_beliefs.len(), 1);
        assert_eq!(
            deserialized.relation_beliefs[0].edge_type,
            CommEdgeType::Owns
        );
        assert!(deserialized.intention_summary.is_some());
        assert!(deserialized.delegation_context.is_none());
        assert!(deserialized.security_metadata.is_none());
        assert!(deserialized.compression_info.is_none());
    }

    #[test]
    fn test_comm_node_type_from_belief() {
        assert_eq!(
            comm_node_type_from_belief(BeliefNodeType::User),
            Some(CommNodeType::User)
        );
        assert_eq!(
            comm_node_type_from_belief(BeliefNodeType::File),
            Some(CommNodeType::File)
        );
        assert_eq!(
            comm_node_type_from_belief(BeliefNodeType::Tool),
            Some(CommNodeType::Tool)
        );
        assert_eq!(
            comm_node_type_from_belief(BeliefNodeType::Variable),
            Some(CommNodeType::Variable)
        );
        assert_eq!(
            comm_node_type_from_belief(BeliefNodeType::Concept),
            Some(CommNodeType::Concept)
        );
        assert_eq!(
            comm_node_type_from_belief(BeliefNodeType::Event),
            Some(CommNodeType::Event)
        );
        assert_eq!(
            comm_node_type_from_belief(BeliefNodeType::Agent),
            Some(CommNodeType::Agent)
        );
        assert_eq!(comm_node_type_from_belief(BeliefNodeType::Resource), None);
        assert_eq!(comm_node_type_from_belief(BeliefNodeType::Process), None);
    }

    #[test]
    fn test_comm_edge_type_from_relation() {
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::Owns),
            Some(CommEdgeType::Owns)
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::DependsOn),
            Some(CommEdgeType::DependsOn)
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::Authorizes),
            Some(CommEdgeType::Authorizes)
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::Calls),
            Some(CommEdgeType::Calls)
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::Contains),
            Some(CommEdgeType::Contains)
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::RelatedTo),
            Some(CommEdgeType::RelatedTo)
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::SynchronizesWith),
            Some(CommEdgeType::SyncsWith)
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::Enables),
            None
        );
        assert_eq!(comm_edge_type_from_relation(RelationEdgeType::Blocks), None);
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::Modifies),
            None
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::References),
            None
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::Precedes),
            None
        );
        assert_eq!(
            comm_edge_type_from_relation(RelationEdgeType::Follows),
            None
        );
    }

    #[test]
    fn test_belief_node_type_from_comm() {
        assert_eq!(
            belief_node_type_from_comm(CommNodeType::User),
            BeliefNodeType::User
        );
        assert_eq!(
            belief_node_type_from_comm(CommNodeType::File),
            BeliefNodeType::File
        );
        assert_eq!(
            belief_node_type_from_comm(CommNodeType::Tool),
            BeliefNodeType::Tool
        );
        assert_eq!(
            belief_node_type_from_comm(CommNodeType::Variable),
            BeliefNodeType::Variable
        );
        assert_eq!(
            belief_node_type_from_comm(CommNodeType::Concept),
            BeliefNodeType::Concept
        );
        assert_eq!(
            belief_node_type_from_comm(CommNodeType::Event),
            BeliefNodeType::Event
        );
        assert_eq!(
            belief_node_type_from_comm(CommNodeType::Agent),
            BeliefNodeType::Agent
        );
    }

    #[test]
    fn test_relation_edge_type_from_comm() {
        assert_eq!(
            relation_edge_type_from_comm(CommEdgeType::Owns),
            Some(RelationEdgeType::Owns)
        );
        assert_eq!(
            relation_edge_type_from_comm(CommEdgeType::DependsOn),
            Some(RelationEdgeType::DependsOn)
        );
        assert_eq!(
            relation_edge_type_from_comm(CommEdgeType::Authorizes),
            Some(RelationEdgeType::Authorizes)
        );
        assert_eq!(
            relation_edge_type_from_comm(CommEdgeType::Calls),
            Some(RelationEdgeType::Calls)
        );
        assert_eq!(
            relation_edge_type_from_comm(CommEdgeType::Contains),
            Some(RelationEdgeType::Contains)
        );
        assert_eq!(
            relation_edge_type_from_comm(CommEdgeType::RelatedTo),
            Some(RelationEdgeType::RelatedTo)
        );
        assert_eq!(
            relation_edge_type_from_comm(CommEdgeType::DelegatesTo),
            None
        );
        assert_eq!(
            relation_edge_type_from_comm(CommEdgeType::SyncsWith),
            Some(RelationEdgeType::SynchronizesWith)
        );
    }

    #[test]
    fn test_comm_snapshot_scope_default() {
        let scope = CommSnapshotScope::default();
        assert!(scope.topics.is_empty());
        assert!(scope.entity_types.is_empty());
        assert!(scope.relationship_types.is_empty());
        assert!(scope.max_depth.is_none());
        assert!(scope.max_nodes.is_none());
        assert!(scope.include_intentions.is_none());
        assert!(scope.include_residuals.is_none());
    }

    #[test]
    fn test_construction_options_default() {
        let opts = ConstructionOptions::default();
        assert!(opts.compression.is_none());
        assert!(!opts.encryption);
        assert!(opts.max_size.is_none());
        assert!(opts.priority.is_none());
        assert!(opts.ttl.is_none());
    }

    #[test]
    fn test_parse_metadata_default() {
        let meta = ParseMetadata::default();
        assert!(meta.expected_version.is_none());
        assert!(meta.compression.is_none());
        assert!(meta.decryption_key.is_none());
        assert!(meta.correlation_id.is_none());
    }

    #[test]
    fn test_verification_checks_default() {
        let checks = VerificationChecks::default();
        assert!(checks.format_validation.is_none());
        assert!(checks.version_check.is_none());
        assert!(checks.timestamp_validation.is_none());
        assert!(checks.signature_verification.is_none());
        assert!(checks.integrity_check.is_none());
    }

    #[test]
    fn test_verification_outcome_is_passed() {
        assert!(VerificationOutcome::Passed.is_passed());
        assert!(!VerificationOutcome::Failed.is_passed());
        assert!(!VerificationOutcome::PartialPass.is_passed());
        assert!(!VerificationOutcome::Expired.is_passed());
    }

    #[test]
    fn test_enum_serde_screaming_snake_case() {
        assert_eq!(
            serde_json::to_string(&CommNodeType::User).unwrap(),
            "\"USER\""
        );
        assert_eq!(
            serde_json::to_string(&CommEdgeType::DependsOn).unwrap(),
            "\"DEPENDS_ON\""
        );
        assert_eq!(
            serde_json::to_string(&SnapshotPurpose::Sync).unwrap(),
            "\"SYNC\""
        );
        assert_eq!(serde_json::to_string(&Priority::High).unwrap(), "\"HIGH\"");
        assert_eq!(
            serde_json::to_string(&SubtaskStatus::InProgress).unwrap(),
            "\"IN_PROGRESS\""
        );
        assert_eq!(
            serde_json::to_string(&ResidualSeverity::Critical).unwrap(),
            "\"CRITICAL\""
        );
        assert_eq!(
            serde_json::to_string(&CompressionAlgorithm::Lz4).unwrap(),
            "\"LZ4\""
        );
        assert_eq!(
            serde_json::to_string(&FallbackStrategy::ReturnError).unwrap(),
            "\"RETURN_ERROR\""
        );
        assert_eq!(
            serde_json::to_string(&VerificationOutcome::PartialPass).unwrap(),
            "\"PARTIAL_PASS\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictResolutionStrategy::LastWriteWins).unwrap(),
            "\"LAST_WRITE_WINS\""
        );
        assert_eq!(
            serde_json::to_string(&FusionAction::Skip).unwrap(),
            "\"SKIP\""
        );
        assert_eq!(
            serde_json::to_string(&MappingType::IdMatch).unwrap(),
            "\"ID_MATCH\""
        );
    }

    #[test]
    fn test_skip_serializing_if_none() {
        let snapshot = CommunicationSnapshot {
            snapshot_id: "test".to_string(),
            snapshot_metadata: SnapshotMetadata {
                version: "1.0".to_string(),
                timestamp: Utc::now(),
                source_agent: SourceAgent {
                    agent_id: "a1".to_string(),
                    agent_type: None,
                    capabilities: vec![],
                },
                scope: CommSnapshotScope::default(),
                purpose: SnapshotPurpose::Notify,
                priority: Priority::Low,
                expires_at: None,
            },
            entity_beliefs: vec![],
            relation_beliefs: vec![],
            intention_summary: None,
            prediction_residual_summary: vec![],
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(value.get("intention_summary").is_none());
        assert!(value.get("delegation_context").is_none());
        assert!(value.get("security_metadata").is_none());
        assert!(value.get("compression_info").is_none());
    }
}
