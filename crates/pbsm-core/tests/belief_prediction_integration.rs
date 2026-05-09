mod integration_common;

use std::sync::Arc;

use integration_common::{
    make_test_belief_node, make_test_observation, CollectingEventPublisher, MockBeliefGraphReader,
};
use pbsm_core::modules::common::{
    PredictionCreatedPayload, PredictionFalsifiedPayload, PredictionVerifiedPayload,
};
use pbsm_core::modules::prediction_engine::PredictionEngine;
use pbsm_core::types::filter::CancellationReason;
use pbsm_core::types::prediction::{ActionRequest, ActionType, PredictionState};
use pbsm_core::types::residual::MatchLevel;

#[tokio::test]
async fn test_prediction_generation_reads_belief_context() {
    let mock_reader = MockBeliefGraphReader::new();
    mock_reader.add_node(make_test_belief_node("node-1", "Entity", 0.9));
    mock_reader.add_node(make_test_belief_node("node-2", "Event", 0.7));

    let event_publisher = Arc::new(CollectingEventPublisher::new());
    let engine = PredictionEngine::with_components(Arc::new(mock_reader), event_publisher.clone());

    let action = ActionRequest {
        action_type: ActionType::ToolCall,
        action_name: "update_entity".to_string(),
        parameters: serde_json::json!({"expected_value": 100}),
        target_id: Some("node-1".to_string()),
    };

    let result = engine.create_prediction(action, None).await;
    assert!(result.is_ok(), "create_prediction should succeed");

    let prediction = result.unwrap();
    assert_eq!(prediction.status, PredictionState::Pending);
    assert_eq!(
        prediction.associated_action.target_node,
        Some("node-1".to_string())
    );

    let created_events: Vec<PredictionCreatedPayload> = event_publisher.events_of_type();
    assert_eq!(
        created_events.len(),
        1,
        "should publish exactly one PredictionCreated event"
    );
    assert_eq!(created_events[0].prediction_id, prediction.prediction_id);
    assert_eq!(created_events[0].target_node, Some("node-1".to_string()));
}

#[tokio::test]
async fn test_prediction_verified_publishes_event() {
    let mock_reader = MockBeliefGraphReader::new();
    mock_reader.add_node(make_test_belief_node("node-1", "Entity", 0.9));

    let event_publisher = Arc::new(CollectingEventPublisher::new());
    let engine = PredictionEngine::with_components(Arc::new(mock_reader), event_publisher.clone());

    let action = ActionRequest {
        action_type: ActionType::ToolCall,
        action_name: "update_entity".to_string(),
        parameters: serde_json::json!({"expected_value": 100}),
        target_id: Some("node-1".to_string()),
    };

    let prediction = engine.create_prediction(action, None).await.unwrap();
    let prediction_id = prediction.prediction_id.to_string();

    let observation =
        make_test_observation("json", serde_json::json!({"state": 100}), "tool_response");

    let result = engine.verify_prediction(&prediction_id, observation).await;
    assert!(result.is_ok(), "verify_prediction should succeed");

    let verification = result.unwrap();
    assert!(
        matches!(
            verification.match_level,
            MatchLevel::Exact | MatchLevel::Partial
        ),
        "match_level should be Exact or Partial, got {:?}",
        verification.match_level
    );

    let verified_events: Vec<PredictionVerifiedPayload> = event_publisher.events_of_type();
    assert_eq!(
        verified_events.len(),
        1,
        "should publish exactly one PredictionVerified event"
    );
    assert_eq!(verified_events[0].prediction_id, prediction.prediction_id);
    assert!(matches!(
        verified_events[0].match_level,
        MatchLevel::Exact | MatchLevel::Partial
    ));
}

#[tokio::test]
async fn test_prediction_falsified_publishes_event_and_residual() {
    let mock_reader = MockBeliefGraphReader::new();
    mock_reader.add_node(make_test_belief_node("node-1", "Entity", 0.9));

    let event_publisher = Arc::new(CollectingEventPublisher::new());
    let engine = PredictionEngine::with_components(Arc::new(mock_reader), event_publisher.clone());

    let action = ActionRequest {
        action_type: ActionType::ToolCall,
        action_name: "update_entity".to_string(),
        parameters: serde_json::json!({"expected_value": 100}),
        target_id: Some("node-1".to_string()),
    };

    let prediction = engine.create_prediction(action, None).await.unwrap();
    let prediction_id = prediction.prediction_id.to_string();

    let observation =
        make_test_observation("json", serde_json::json!({"state": 0}), "tool_response");

    let result = engine.verify_prediction(&prediction_id, observation).await;
    assert!(result.is_ok(), "verify_prediction should succeed");

    let verification = result.unwrap();
    assert!(
        verification.residual.overall_degree > 0.3,
        "residual overall_degree should be significant, got {}",
        verification.residual.overall_degree,
    );

    let falsified_events: Vec<PredictionFalsifiedPayload> = event_publisher.events_of_type();
    assert_eq!(
        falsified_events.len(),
        1,
        "should publish exactly one PredictionFalsified event"
    );
    assert_eq!(falsified_events[0].prediction_id, prediction.prediction_id);
    assert!(
        falsified_events[0].overall_degree > 0.3,
        "falsified event overall_degree should be significant",
    );
}

#[tokio::test]
async fn test_prediction_lifecycle_with_belief_graph() {
    let mock_reader = MockBeliefGraphReader::new();
    mock_reader.add_node(make_test_belief_node("node-1", "Entity", 0.9));

    let event_publisher = Arc::new(CollectingEventPublisher::new());
    let engine = PredictionEngine::with_components(Arc::new(mock_reader), event_publisher.clone());

    let action = ActionRequest {
        action_type: ActionType::ToolCall,
        action_name: "update_entity".to_string(),
        parameters: serde_json::json!({"expected_value": 100}),
        target_id: Some("node-1".to_string()),
    };

    let prediction = engine.create_prediction(action, None).await.unwrap();
    let prediction_id = prediction.prediction_id.to_string();
    assert_eq!(prediction.status, PredictionState::Pending);

    let observation =
        make_test_observation("json", serde_json::json!({"state": 100}), "tool_response");

    let result = engine
        .verify_prediction(&prediction_id, observation)
        .await
        .unwrap();
    assert!(
        matches!(result.match_level, MatchLevel::Exact | MatchLevel::Partial),
        "match_level should be Exact or Partial, got {:?}",
        result.match_level
    );

    let retrieved = engine.get_prediction_by_id(&prediction_id, None).unwrap();
    assert_eq!(retrieved.status, PredictionState::Verified);

    let stats = engine.get_prediction_statistics(None);
    assert_eq!(stats.total, 1);
    assert_eq!(stats.by_status.get("Verified"), Some(&1u64));
    assert!(
        stats.verification_rate > 0.0,
        "verification_rate should be positive after a verified prediction",
    );
    assert_eq!(stats.falsification_rate, 0.0);
}

#[tokio::test]
async fn test_multiple_predictions_isolated() {
    let mock_reader = MockBeliefGraphReader::new();
    mock_reader.add_node(make_test_belief_node("node-1", "Entity", 0.9));
    mock_reader.add_node(make_test_belief_node("node-2", "Event", 0.7));

    let event_publisher = Arc::new(CollectingEventPublisher::new());
    let engine = PredictionEngine::with_components(Arc::new(mock_reader), event_publisher.clone());

    let action1 = ActionRequest {
        action_type: ActionType::ToolCall,
        action_name: "update_entity".to_string(),
        parameters: serde_json::json!({"expected_value": 100}),
        target_id: Some("node-1".to_string()),
    };

    let action2 = ActionRequest {
        action_type: ActionType::InternalInference,
        action_name: "analyze_event".to_string(),
        parameters: serde_json::json!({"expected_value": 50}),
        target_id: Some("node-2".to_string()),
    };

    let prediction1 = engine.create_prediction(action1, None).await.unwrap();
    let prediction2 = engine.create_prediction(action2, None).await.unwrap();
    let id1 = prediction1.prediction_id.to_string();
    let id2 = prediction2.prediction_id.to_string();

    assert_ne!(id1, id2, "predictions should have unique IDs");

    let observation1 =
        make_test_observation("json", serde_json::json!({"state": 100}), "tool_response");

    let result1 = engine.verify_prediction(&id1, observation1).await.unwrap();
    assert!(
        matches!(result1.match_level, MatchLevel::Exact | MatchLevel::Partial),
        "match_level should be Exact or Partial, got {:?}",
        result1.match_level
    );

    let cancel_result = engine.cancel_prediction(&id2, CancellationReason::UserRequest);
    assert!(cancel_result.is_ok());

    let retrieved1 = engine.get_prediction_by_id(&id1, None).unwrap();
    assert_eq!(retrieved1.status, PredictionState::Verified);

    let retrieved2 = engine.get_prediction_by_id(&id2, None).unwrap();
    assert_eq!(retrieved2.status, PredictionState::Cancelled);

    assert!(
        retrieved1.residuals.is_some(),
        "verified prediction should have residuals"
    );
    assert!(
        retrieved2.residuals.is_none(),
        "cancelled prediction should not have residuals"
    );

    let stats = engine.get_prediction_statistics(None);
    assert_eq!(stats.total, 2);
    assert_eq!(stats.by_status.get("Verified"), Some(&1u64));
    assert_eq!(stats.by_status.get("Cancelled"), Some(&1u64));
}
