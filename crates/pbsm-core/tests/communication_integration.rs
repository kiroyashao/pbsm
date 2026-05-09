use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use pbsm_core::modules::belief_graph::graph::BeliefGraph;
use pbsm_core::modules::belief_graph::operations::BeliefGraphOperations;
use pbsm_core::modules::belief_graph::types::{AttributeValue, BeliefNodeType, SourceType};

use pbsm_core::modules::communication::conflict::detector::ConflictType;
use pbsm_core::modules::communication::conflict::negotiation::{
    NegotiationHandler, NegotiationOutcome, NegotiationResponse, NegotiationState, Proposal,
    ProposalJustification, ResponseData, ResponseType,
};
use pbsm_core::modules::communication::conflict::ConflictDetector;
use pbsm_core::modules::communication::security::SensitiveDataFilter;
use pbsm_core::modules::communication::snapshot::SnapshotConstructor;
use pbsm_core::modules::communication::snapshot::SnapshotFusion;
use pbsm_core::modules::communication::snapshot::SnapshotParser;
use pbsm_core::modules::communication::sync::{SyncStateMachine, SyncStateTransition, SyncStatus};
use pbsm_core::modules::communication::types::*;

#[test]
fn test_snapshot_construction_and_parsing() {
    let graph = Arc::new(BeliefGraph::with_default_config());

    BeliefGraphOperations::create_belief(
        &graph,
        BeliefNodeType::User,
        "Alice".to_string(),
        HashMap::new(),
        "test".to_string(),
        SourceType::UserInput,
        None,
        None,
    )
    .unwrap();

    let constructor = SnapshotConstructor::new(
        graph,
        "agent-001".to_string(),
        "coordinator".to_string(),
        vec!["query".to_string()],
    );

    let constructed = constructor
        .construct_snapshot(CommSnapshotScope::default(), SnapshotPurpose::Sync, None)
        .unwrap();

    let json_bytes = serde_json::to_vec(&constructed.snapshot).unwrap();

    let parser = SnapshotParser::new();
    let parsed = parser.parse_snapshot(&json_bytes, None).unwrap();

    assert_eq!(
        parsed.snapshot.snapshot_id,
        constructed.snapshot.snapshot_id
    );
    assert_eq!(
        parsed.verification_result.result,
        VerificationOutcome::Passed
    );
}

#[test]
fn test_snapshot_fusion_with_belief_graph() {
    let graph = Arc::new(BeliefGraph::with_default_config());

    let node_id = BeliefGraphOperations::create_belief(
        &graph,
        BeliefNodeType::User,
        "Alice".to_string(),
        {
            let mut attrs = HashMap::new();
            attrs.insert(
                "role".to_string(),
                AttributeValue::new(
                    json!("admin"),
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

    let fusion = SnapshotFusion::new(graph.clone());

    let snapshot = CommunicationSnapshot {
        snapshot_id: "fusion-test-001".to_string(),
        snapshot_metadata: SnapshotMetadata {
            version: "1.0".to_string(),
            timestamp: Utc::now(),
            source_agent: SourceAgent {
                agent_id: "agent-external".to_string(),
                agent_type: None,
                capabilities: vec![],
            },
            scope: CommSnapshotScope::default(),
            purpose: SnapshotPurpose::Sync,
            priority: Priority::Normal,
            expires_at: None,
        },
        entity_beliefs: vec![EntityBelief {
            node_id: node_id.to_string(),
            node_type: CommNodeType::User,
            name: Some("Alice".to_string()),
            key_attributes: Some({
                let mut map = HashMap::new();
                map.insert(
                    "role".to_string(),
                    CommAttributeValue {
                        value: json!("user"),
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
    };

    let result = fusion
        .fuse_snapshot(
            &snapshot,
            Some(FusionOptions {
                conflict_resolution_strategy: Some(ConflictResolutionStrategy::AutoMerge),
                auto_merge_threshold: None,
                max_conflicts: None,
                preserve_local_history: None,
                trigger_metacognition: None,
            }),
        )
        .unwrap();

    assert!(result.metrics.nodes_updated >= 1);
    assert!(result.metrics.conflicts_detected >= 1);
    assert!(result.metrics.conflicts_resolved >= 1);
}

#[test]
fn test_sync_state_machine_flow() {
    let mut sm = SyncStateMachine::new();
    let sync_id = "test-sync-1";

    sm.transition(sync_id, SyncStateTransition::RequestInitiated)
        .unwrap();
    let info = sm.get_status(sync_id).unwrap();
    assert_eq!(info.status, SyncStatus::Initiated);

    sm.transition(sync_id, SyncStateTransition::ResponseReceived)
        .unwrap();

    sm.transition(sync_id, SyncStateTransition::SnapshotConstructed)
        .unwrap();
    let info = sm.get_status(sync_id).unwrap();
    assert_eq!(info.status, SyncStatus::InProgress);

    sm.transition(sync_id, SyncStateTransition::SnapshotTransmitted)
        .unwrap();

    sm.transition(sync_id, SyncStateTransition::VerificationCompleted)
        .unwrap();

    sm.transition(sync_id, SyncStateTransition::Completed)
        .unwrap();
    let info = sm.get_status(sync_id).unwrap();
    assert_eq!(info.status, SyncStatus::Completed);
}

#[test]
fn test_security_filter_removes_sensitive_fields() {
    let filter = SensitiveDataFilter::new();

    let mut snapshot = CommunicationSnapshot {
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
            purpose: SnapshotPurpose::Sync,
            priority: Priority::Normal,
            expires_at: None,
        },
        entity_beliefs: vec![EntityBelief {
            node_id: "node-1".to_string(),
            node_type: CommNodeType::User,
            name: None,
            key_attributes: Some({
                let mut map = HashMap::new();
                map.insert(
                    "password".to_string(),
                    CommAttributeValue {
                        value: json!("secret123"),
                        confidence: 0.9,
                        source: None,
                        last_updated: None,
                    },
                );
                map.insert(
                    "email".to_string(),
                    CommAttributeValue {
                        value: json!("user@example.com"),
                        confidence: 0.9,
                        source: None,
                        last_updated: None,
                    },
                );
                map.insert(
                    "name".to_string(),
                    CommAttributeValue {
                        value: json!("Alice"),
                        confidence: 0.9,
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
    };

    let result = filter
        .filter_sensitive_data(&mut snapshot, "agent-2", SnapshotPurpose::Sync)
        .unwrap();

    assert!(result
        .filter_report
        .filtered_fields
        .contains(&"password".to_string()));
    assert!(result
        .filter_report
        .filtered_fields
        .contains(&"email".to_string()));

    let attrs = result.snapshot.entity_beliefs[0]
        .key_attributes
        .as_ref()
        .unwrap();
    assert_eq!(attrs.get("password").unwrap().value, json!("[REDACTED]"));
    assert_eq!(attrs.get("email").unwrap().value, json!("***"));
    assert_eq!(attrs.get("name").unwrap().value, json!("Alice"));
}

#[test]
fn test_conflict_detection_and_negotiation() {
    let mut local_attrs = HashMap::new();
    local_attrs.insert(
        "role".to_string(),
        CommAttributeValue {
            value: json!("admin"),
            confidence: 0.9,
            source: None,
            last_updated: None,
        },
    );

    let mut remote_attrs = HashMap::new();
    remote_attrs.insert(
        "role".to_string(),
        CommAttributeValue {
            value: json!("viewer"),
            confidence: 0.7,
            source: None,
            last_updated: None,
        },
    );

    let local = EntityBelief {
        node_id: "node-1".to_string(),
        node_type: CommNodeType::User,
        name: None,
        tags: vec![],
        key_attributes: Some(local_attrs),
    };

    let remote = EntityBelief {
        node_id: "node-1".to_string(),
        node_type: CommNodeType::User,
        name: None,
        tags: vec![],
        key_attributes: Some(remote_attrs),
    };

    let result = ConflictDetector::detect_conflicts(&local, &remote);
    assert!(result.is_ok());
    let conflict_opt = result.unwrap();
    assert!(conflict_opt.is_some());
    let conflict = conflict_opt.unwrap();
    assert_eq!(conflict.conflict_type, ConflictType::AttributeMismatch);

    let handler = NegotiationHandler::new();

    let proposal = Proposal {
        proposal_id: Uuid::new_v4().to_string(),
        proposed_value: json!("editor"),
        justification: ProposalJustification {
            reasoning: "compromise".to_string(),
            evidence: vec![],
            authority: None,
        },
        confidence: 0.8,
    };

    let session = handler
        .initiate_negotiation(conflict, vec![proposal], None)
        .unwrap();

    assert_eq!(session.state, NegotiationState::Initiated);

    let neg_id = session.negotiation_id.clone();

    let result = handler
        .respond_to_negotiation(
            &neg_id,
            NegotiationResponse {
                response_type: ResponseType::Accept,
                data: ResponseData {
                    justification: None,
                    counter_proposal: None,
                },
            },
        )
        .unwrap();

    assert_eq!(result.outcome, NegotiationOutcome::Accepted);
    assert!(result.resolution.is_some());
}
