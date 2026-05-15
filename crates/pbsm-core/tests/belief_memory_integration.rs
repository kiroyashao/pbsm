mod integration_common;

use std::collections::HashMap;

use integration_common::temp_memory_config;
use pbsm_core::modules::belief_graph::graph::BeliefGraph;
use pbsm_core::modules::belief_graph::operations::BeliefGraphOperations;
use pbsm_core::modules::belief_graph::types::{BeliefNodeType, SourceType};
use pbsm_core::modules::intention_stack::manager::IntentionStackManager;
use pbsm_core::modules::intention_stack::manager::IntentionStackManagerImpl;
use pbsm_core::modules::intention_stack::state::{ExecutionState, GoalPriority};
use pbsm_core::modules::intention_stack::types::{
    GoalDefinition, PushIntentRequest, UpdateIntentStateRequest,
};
use pbsm_core::modules::memory::store::ExternalMemoryStore;
use pbsm_core::modules::memory::types::{
    AttentionMode, AttentionState, BeliefState, CleanupPolicy, CleanupScope, CleanupType,
    Experience, ExperienceContent, ExperienceMetadata, ExperienceRelationships,
    ExperienceUsageStats, Intention, IntentionState, LogType, MemoryQuery, PatternType,
    ProblemType, SnapshotType, StateTarget,
};

#[tokio::test]
async fn test_belief_state_snapshot_write_and_restore() {
    let config = temp_memory_config();
    let store = ExternalMemoryStore::open(config).await.unwrap();

    let graph = BeliefGraph::with_default_config();
    let node_id_1 = BeliefGraphOperations::create_belief(
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
    let node_id_2 = BeliefGraphOperations::create_belief(
        &graph,
        BeliefNodeType::File,
        "readme.md".to_string(),
        HashMap::new(),
        "test".to_string(),
        SourceType::DirectObservation,
        None,
        None,
    )
    .unwrap();

    assert!(graph.get_node(node_id_1).is_some());
    assert!(graph.get_node(node_id_2).is_some());

    let nodes: Vec<_> = graph.nodes().read().values().cloned().collect();
    let edges: Vec<_> = graph.edges().read().values().cloned().collect();

    let belief_state = BeliefState {
        nodes,
        edges,
        active_predictions: vec![],
        unresolved_residuals: vec![],
    };

    let intention_state = IntentionState {
        stack: vec![],
        active_goal_pointer: 0,
        execution_depth: 0,
    };

    let attention_state = AttentionState {
        parameter: 0.6,
        mode: AttentionMode::Moderate,
        focus_areas: vec![],
    };

    let write_result = store
        .write_snapshot(
            "sess-belief-snap",
            SnapshotType::Manual,
            belief_state,
            intention_state,
            attention_state,
            "manual",
            "belief graph snapshot test",
        )
        .await
        .unwrap();

    assert!(!write_result.snapshot_id.is_empty());
    assert!(write_result.node_count >= 2);

    let restored = store
        .restore_snapshot(&write_result.snapshot_id, StateTarget::Full, true)
        .await
        .unwrap();

    assert!(restored.restored);
    assert_eq!(
        restored.snapshot.metadata.snapshot_id,
        write_result.snapshot_id
    );
    assert_eq!(restored.snapshot.belief_state.nodes.len(), 2);

    let _ = store.close().await;
}

#[tokio::test]
async fn test_intention_state_in_snapshot() {
    let config = temp_memory_config();
    let store = ExternalMemoryStore::open(config).await.unwrap();

    let manager = IntentionStackManagerImpl::new("test_stack".to_string());

    let goal = GoalDefinition::simple("Analyze codebase".to_string(), GoalPriority::High);
    let push_req = PushIntentRequest {
        goal,
        plan: None,
        parent_level: None,
        micro_prediction: None,
        attach_to_current: false,
    };
    let push_resp = manager.push_intention(push_req).await.unwrap();
    assert!(push_resp.success);

    manager
        .update_intention_state(UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::Ready,
            transition_context: None,
            force: false,
        })
        .await
        .unwrap();
    manager
        .update_intention_state(UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::InProgress,
            transition_context: None,
            force: false,
        })
        .await
        .unwrap();

    let exported = manager.export_state().await.unwrap();

    let intentions: Vec<Intention> = exported
        .stack
        .layers
        .iter()
        .map(|layer| Intention {
            intention_id: layer.layer_id.clone(),
            goal: layer.goal.description.clone(),
            status: pbsm_core::modules::memory::types::IntentionStatus::Active,
            confidence: 0.85,
            created_at: layer.metadata.created_at.timestamp_millis(),
        })
        .collect();

    let intention_state = IntentionState {
        stack: intentions,
        active_goal_pointer: exported.stack.active_goal_pointer,
        execution_depth: exported.stack.depth(),
    };

    assert_eq!(intention_state.stack.len(), 1);
    assert_eq!(intention_state.stack[0].goal, "Analyze codebase");

    let belief_state = BeliefState {
        nodes: vec![],
        edges: vec![],
        active_predictions: vec![],
        unresolved_residuals: vec![],
    };

    let attention_state = AttentionState {
        parameter: 0.5,
        mode: AttentionMode::Moderate,
        focus_areas: vec![],
    };

    let write_result = store
        .write_snapshot(
            "sess-intention-snap",
            SnapshotType::Automatic,
            belief_state,
            intention_state,
            attention_state,
            "automatic",
            "intention state snapshot test",
        )
        .await
        .unwrap();

    let restored = store
        .restore_snapshot(&write_result.snapshot_id, StateTarget::Full, false)
        .await
        .unwrap();

    assert!(restored.restored);
    assert_eq!(restored.snapshot.intention_state.stack.len(), 1);
    assert_eq!(
        restored.snapshot.intention_state.stack[0].goal,
        "Analyze codebase"
    );
    assert_eq!(restored.snapshot.intention_state.active_goal_pointer, 0);
    assert_eq!(restored.snapshot.intention_state.execution_depth, 1);

    let _ = store.close().await;
}

#[tokio::test]
async fn test_memory_retrieval_by_topic() {
    let config = temp_memory_config();
    let store = ExternalMemoryStore::open(config).await.unwrap();

    store
        .write_raw_log(
            "sess-001",
            LogType::Dialogue,
            serde_json::json!({"message": "hello"}),
            "greeting",
            Some(0.9),
            None,
        )
        .await
        .unwrap();

    store
        .write_raw_log(
            "sess-001",
            LogType::ToolCall,
            serde_json::json!({"tool": "search", "query": "rust"}),
            "tool_usage",
            Some(0.8),
            None,
        )
        .await
        .unwrap();

    store
        .write_raw_log(
            "sess-001",
            LogType::Dialogue,
            serde_json::json!({"message": "goodbye"}),
            "greeting",
            Some(0.7),
            None,
        )
        .await
        .unwrap();

    let query = MemoryQuery {
        topic: "greeting".to_string(),
        confidence_threshold: None,
        layer_filter: None,
        time_range_start: None,
        time_range_end: None,
        include_raw_logs: true,
    };

    let result = store.retrieve_by_topic(query).await.unwrap();

    assert_eq!(result.query_topic, "greeting");
    assert!(
        result.total_matches >= 2,
        "expected at least 2 matches for 'greeting', got {}",
        result.total_matches
    );

    let query_tool = MemoryQuery {
        topic: "tool_usage".to_string(),
        confidence_threshold: None,
        layer_filter: None,
        time_range_start: None,
        time_range_end: None,
        include_raw_logs: true,
    };

    let result_tool = store.retrieve_by_topic(query_tool).await.unwrap();
    assert!(
        result_tool.total_matches >= 1,
        "expected at least 1 match for 'tool_usage', got {}",
        result_tool.total_matches
    );

    let _ = store.close().await;
}

#[tokio::test]
async fn test_experience_write_and_problem_retrieval() {
    let config = temp_memory_config();
    let store = ExternalMemoryStore::open(config).await.unwrap();

    let experience = Experience {
        experience_id: "exp-error-001".to_string(),
        metadata: ExperienceMetadata {
            source_type: "integration_test".to_string(),
            source_snapshot_ids: None,
            source_log_ids: None,
            verification_count: 0,
            last_used_at: None,
            tags: None,
        },
        content: ExperienceContent {
            title: "Tool Timeout Recovery".to_string(),
            summary: "How to recover from tool execution timeouts".to_string(),
            domain: "error_handling".to_string(),
            pattern: PatternType::ErrorHandling,
            confidence: 0.9,
            context: serde_json::json!({"timeout_ms": 30000}),
            knowledge: serde_json::json!({"strategy": "retry_with_backoff"}),
            outcomes: serde_json::json!({"success_rate": 0.92}),
        },
        usage_stats: ExperienceUsageStats {
            access_count: 0,
            last_accessed_at: None,
            verification_count: 0,
        },
        relationships: ExperienceRelationships {
            related_experience_ids: None,
            contradicts_experience_ids: None,
            refines_experience_ids: None,
        },
    };

    let write_result = store.write_experience(experience, false).await.unwrap();

    assert_eq!(write_result.experience_id, "exp-error-001");

    let problem_result = store
        .retrieve_for_problem(
            "Tool execution timeout during search operation",
            Some(ProblemType::ToolExecutionFailure),
            None,
        )
        .await
        .unwrap();

    assert!(!problem_result.request_id.is_empty());
    assert_eq!(
        problem_result.inferred_problem_type,
        ProblemType::ToolExecutionFailure
    );

    let _ = store.close().await;
}

#[tokio::test]
async fn test_cleanup_preserves_active_data() {
    let config = temp_memory_config();
    let store = ExternalMemoryStore::open(config).await.unwrap();

    let belief_state = BeliefState {
        nodes: vec![],
        edges: vec![],
        active_predictions: vec![],
        unresolved_residuals: vec![],
    };

    let intention_state = IntentionState {
        stack: vec![],
        active_goal_pointer: 0,
        execution_depth: 0,
    };

    let attention_state = AttentionState {
        parameter: 0.5,
        mode: AttentionMode::Moderate,
        focus_areas: vec![],
    };

    store
        .write_snapshot(
            "sess-cleanup",
            SnapshotType::Manual,
            belief_state,
            intention_state,
            attention_state,
            "manual",
            "cleanup test snapshot",
        )
        .await
        .unwrap();

    store
        .write_raw_log(
            "sess-cleanup",
            LogType::Dialogue,
            serde_json::json!({"msg": "cleanup test log"}),
            "cleanup_topic",
            Some(0.6),
            None,
        )
        .await
        .unwrap();

    let stats_before = store.get_storage_stats().await.unwrap();
    assert_eq!(stats_before.snapshot_count, 1);
    assert_eq!(stats_before.raw_log_count, 1);

    let policy = CleanupPolicy {
        cleanup_type: CleanupType::Standard,
        scope: CleanupScope::AllLayers,
        max_age_days: Some(7),
        min_importance: None,
        dry_run: true,
    };

    let cleanup_result = store.cleanup_expired(policy).await.unwrap();
    assert_eq!(
        cleanup_result.status,
        pbsm_core::modules::memory::types::CleanupStatus::Completed
    );

    let stats_after = store.get_storage_stats().await.unwrap();
    assert_eq!(stats_after.snapshot_count, 1);
    assert_eq!(stats_after.raw_log_count, 1);
    assert_eq!(stats_after.total_entries, stats_before.total_entries);

    let _ = store.close().await;
}
