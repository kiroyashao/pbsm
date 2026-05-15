use std::collections::HashMap;
use std::sync::Arc;

use pbsm_core::modules::belief_graph::graph::BeliefGraph;
use pbsm_core::modules::belief_graph::operations::BeliefGraphOperations;
use pbsm_core::modules::belief_graph::types::{BeliefNodeType, SourceType};
use pbsm_core::modules::common::{EventPublisher, NullBeliefGraphReader, NullEventPublisher};
use pbsm_core::modules::intention_stack::manager::{
    IntentionStackManager, IntentionStackManagerImpl,
};
use pbsm_core::modules::intention_stack::state::{ExecutionState, GoalPriority};
use pbsm_core::modules::intention_stack::types::{
    GoalDefinition, PlanStep, PopIntentRequest, PopReason, PushIntentRequest,
    UpdateIntentStateRequest,
};
use pbsm_core::modules::memory::config::MemoryConfig;
use pbsm_core::modules::memory::store::ExternalMemoryStore;
use pbsm_core::modules::memory::types::{
    AttentionMode as MemoryAttentionMode, AttentionState, BeliefState, Intention, IntentionState,
    IntentionStatus, SnapshotType,
};
use pbsm_core::modules::metacognition::controller::MetacognitiveController;
use pbsm_core::modules::metacognition::types::{
    AdjustAttentionRequest, AdjustmentTrigger, EvaluateMemoryValueRequest, ForceForgetRequest,
    ForgetReason,
};
use pbsm_core::modules::prediction_engine::PredictionEngine;
use pbsm_core::types::prediction::{ActionRequest, ActionType, Observation, PredictionState};

#[tokio::test]
/// BeliefGraph 初始化为空、IntentionStackManager 推入根意图、MetacognitiveController 注意力参数初始值 0.5、PredictionEngine 组件初始化
async fn test_task_startup_workflow() {
    let graph = BeliefGraph::with_default_config();
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.version(), 0);

    let manager = IntentionStackManagerImpl::new("startup_stack".to_string());
    let goal = GoalDefinition::simple("Root goal".to_string(), GoalPriority::High);
    let push_req = PushIntentRequest {
        goal,
        plan: None,
        parent_level: None,
        micro_prediction: None,
        attach_to_current: false,
    };
    let push_resp = manager.push_intention(push_req).await.unwrap();
    assert!(push_resp.success);
    assert_eq!(push_resp.layer_index, 0);

    let controller = MetacognitiveController::new();
    let attention_status = controller.get_attention_status().await;
    assert!((attention_status.attention_parameter - 0.5).abs() < f64::EPSILON);

    let belief_graph: Arc<dyn pbsm_core::modules::common::BeliefGraphReader> =
        Arc::new(NullBeliefGraphReader);
    let event_publisher: Arc<dyn EventPublisher> = Arc::new(NullEventPublisher);
    let engine = PredictionEngine::with_components(belief_graph, event_publisher);

    let stats = graph.get_statistics();
    assert_eq!(stats.total_nodes, 0);
    assert_eq!(stats.total_edges, 0);

    let stack_state = manager.get_stack_state().await.unwrap();
    assert_eq!(stack_state.layers.len(), 1);
    assert_eq!(stack_state.layers[0].goal.description, "Root goal");

    assert!((attention_status.attention_parameter - 0.5).abs() < f64::EPSILON);

    let pending = engine.get_pending_count();
    assert_eq!(pending, 0);
}

#[tokio::test]
/// 创建信念节点 → 推入带计划的意图 → 状态转换 Ready→InProgress → 创建预测 Pending → 观测验证 → 推进步骤 → 完成弹出
async fn test_task_execution_cycle() {
    let graph = BeliefGraph::with_default_config();
    let node_id = BeliefGraphOperations::create_belief(
        &graph,
        BeliefNodeType::File,
        "config.yaml".to_string(),
        HashMap::new(),
        "test".to_string(),
        SourceType::UserInput,
        None,
        None,
    )
    .unwrap();
    assert!(graph.get_node(node_id).is_some());
    assert_eq!(graph.node_count(), 1);

    let manager = IntentionStackManagerImpl::new("execution_stack".to_string());
    let goal = GoalDefinition::simple("Process config file".to_string(), GoalPriority::High);
    let step1 = PlanStep::simple("read_config".to_string(), 0);
    let step2 = PlanStep::simple("validate_config".to_string(), 1);
    let push_req = PushIntentRequest {
        goal,
        plan: Some(vec![step1, step2]),
        parent_level: None,
        micro_prediction: None,
        attach_to_current: false,
    };
    let push_resp = manager.push_intention(push_req).await.unwrap();
    assert!(push_resp.success);

    let update_ready = manager
        .update_intention_state(UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::Ready,
            transition_context: None,
            force: false,
        })
        .await
        .unwrap();
    assert!(update_ready.success);
    assert_eq!(update_ready.current_state, ExecutionState::Ready);

    let update_progress = manager
        .update_intention_state(UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::InProgress,
            transition_context: None,
            force: false,
        })
        .await
        .unwrap();
    assert!(update_progress.success);
    assert_eq!(update_progress.current_state, ExecutionState::InProgress);

    let engine = PredictionEngine::new();
    let action = ActionRequest {
        action_type: ActionType::ToolCall,
        action_name: "read_config".to_string(),
        parameters: serde_json::json!({"file": "config.yaml"}),
        target_id: Some(node_id.to_string()),
    };
    let prediction = engine.create_prediction(action, None).await.unwrap();
    assert_eq!(prediction.status, PredictionState::Pending);

    let observation = Observation {
        format: "json".to_string(),
        data: serde_json::json!({"status": "ok", "content": "valid"}),
        timestamp: chrono::Utc::now(),
        source: "tool_response".to_string(),
    };
    let _verify_result = engine
        .verify_prediction(&prediction.prediction_id.to_string(), observation)
        .await
        .unwrap();

    let advance_result = manager.advance_step(0).await.unwrap();
    assert!(advance_result.success);

    let layer = manager.get_layer(0).await.unwrap().unwrap();
    assert_eq!(layer.current_step_index, 1);

    let pop_req = PopIntentRequest {
        layer_index: 0,
        reason: PopReason::Completed,
        final_state: None,
        completion_report: None,
        cascade: false,
    };
    let pop_resp = manager.pop_intention(pop_req).await.unwrap();
    assert!(pop_resp.success);
}

#[tokio::test]
/// 预测被不匹配观测 Falsified → 注意力上调（PredictionDeviation 触发）→ 创建检查点用于回滚 → 验证错误状态被捕获
async fn test_error_handling_workflow() {
    let manager = IntentionStackManagerImpl::new("error_stack".to_string());
    let goal = GoalDefinition::simple("Error-prone task".to_string(), GoalPriority::High);
    let push_req = PushIntentRequest {
        goal,
        plan: None,
        parent_level: None,
        micro_prediction: None,
        attach_to_current: false,
    };
    manager.push_intention(push_req).await.unwrap();

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

    let engine = PredictionEngine::new();
    let action = ActionRequest {
        action_type: ActionType::ToolCall,
        action_name: "risky_operation".to_string(),
        parameters: serde_json::json!({"target": "critical_resource", "expected_value": "success"}),
        target_id: Some("critical_resource".to_string()),
    };
    let prediction = engine.create_prediction(action, None).await.unwrap();
    assert_eq!(prediction.status, PredictionState::Pending);

    let observation = Observation {
        format: "json".to_string(),
        data: serde_json::json!({"error": "permission_denied", "code": 403}),
        timestamp: chrono::Utc::now(),
        source: "tool_response".to_string(),
    };
    let _verify_result = engine
        .verify_prediction(&prediction.prediction_id.to_string(), observation)
        .await
        .unwrap();

    let updated_prediction = engine
        .get_prediction_by_id(&prediction.prediction_id.to_string(), None)
        .unwrap();
    assert_eq!(updated_prediction.status, PredictionState::Falsified);

    let controller = MetacognitiveController::new();
    let adjust_result = controller
        .adjust_attention(AdjustAttentionRequest {
            delta: None,
            target_value: None,
            trigger: AdjustmentTrigger::PredictionDeviation,
            override_mode: None,
        })
        .await
        .unwrap();
    assert!(adjust_result.new_value > adjust_result.previous_value);

    let checkpoint = manager
        .create_checkpoint(0, Some("error_recovery".to_string()))
        .await
        .unwrap();
    assert_eq!(checkpoint.label, Some("error_recovery".to_string()));

    let layer = manager.get_layer(0).await.unwrap().unwrap();
    assert_eq!(layer.execution_state, ExecutionState::InProgress);
}

#[tokio::test]
/// 写入快照到 ExternalMemoryStore → 关闭 → 重新打开 → 恢复快照 → 验证信念/意图/注意力数据一致 → 从恢复状态初始化新的 IntentionStackManager
async fn test_cross_session_recovery() {
    fn temp_config() -> MemoryConfig {
        let uid = uuid::Uuid::new_v4().to_string();
        let path = std::env::temp_dir().join(format!("pbsm_e2e_test_{uid}"));
        MemoryConfig {
            storage_path: path,
            ..MemoryConfig::default()
        }
    }

    fn make_belief_state() -> BeliefState {
        BeliefState {
            nodes: vec![],
            edges: vec![],
            active_predictions: vec![],
            unresolved_residuals: vec![],
        }
    }

    fn make_intention_state() -> IntentionState {
        IntentionState {
            stack: vec![Intention {
                intention_id: "int-e2e-001".to_string(),
                goal: "E2E test goal".to_string(),
                status: IntentionStatus::Active,
                confidence: 0.85,
                created_at: chrono::Utc::now().timestamp_millis(),
            }],
            active_goal_pointer: 0,
            execution_depth: 1,
        }
    }

    fn make_attention_state() -> AttentionState {
        AttentionState {
            parameter: 0.7,
            mode: MemoryAttentionMode::HighResolution,
            focus_areas: vec!["error_handling".to_string()],
        }
    }

    let config = temp_config();
    let storage_path = config.storage_path.clone();
    let snapshot_id;

    {
        let store = ExternalMemoryStore::open(config.clone()).await.unwrap();

        let write_result = store
            .write_snapshot(
                "session-e2e-001",
                SnapshotType::Manual,
                make_belief_state(),
                make_intention_state(),
                make_attention_state(),
                "e2e cross-session test",
            )
            .await
            .unwrap();

        snapshot_id = write_result.snapshot_id.clone();
        assert!(!snapshot_id.is_empty());

        let _ = store.close().await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let config2 = MemoryConfig {
        storage_path: storage_path.clone(),
        ..MemoryConfig::default()
    };
    let store2 = ExternalMemoryStore::open(config2).await.unwrap();

    let restore_result = store2.restore_snapshot(&snapshot_id, true).await.unwrap();
    assert!(restore_result.restored);

    let restored_snapshot = restore_result.snapshot;
    assert_eq!(restored_snapshot.intention_state.stack.len(), 1);
    assert_eq!(
        restored_snapshot.intention_state.stack[0].goal,
        "E2E test goal"
    );
    assert_eq!(
        restored_snapshot.intention_state.stack[0].status,
        IntentionStatus::Active
    );
    assert!((restored_snapshot.attention_state.parameter - 0.7).abs() < f64::EPSILON);
    assert_eq!(
        restored_snapshot.attention_state.mode,
        MemoryAttentionMode::HighResolution
    );

    let manager = IntentionStackManagerImpl::new("recovered_stack".to_string());
    let goal = GoalDefinition::simple(
        restored_snapshot.intention_state.stack[0].goal.clone(),
        GoalPriority::High,
    );
    let push_req = PushIntentRequest {
        goal,
        plan: None,
        parent_level: None,
        micro_prediction: None,
        attach_to_current: false,
    };
    let push_resp = manager.push_intention(push_req).await.unwrap();
    assert!(push_resp.success);

    let current = manager.get_current_intention(None).await.unwrap();
    assert_eq!(current.current_layer.goal_description, "E2E test goal");

    store2.close().await.unwrap();

    let _ = std::fs::remove_dir_all(&storage_path);
}

#[tokio::test]
/// 注册高低价值信念 → 评估记忆价值 → 高价值分数严格大于低价值 → 强制遗忘低价值信念 → 验证遗忘统计
async fn test_active_forgetting_convergence() {
    let controller = MetacognitiveController::new();

    controller
        .value_evaluator()
        .register_belief("high_value_1", 0.95);
    controller
        .value_evaluator()
        .set_belief_access_count("high_value_1", 10);
    controller
        .value_evaluator()
        .set_belief_last_accessed("high_value_1", 0);
    controller
        .value_evaluator()
        .set_belief_residual_association("high_value_1", 0.5);

    controller
        .value_evaluator()
        .register_belief("high_value_2", 0.85);
    controller
        .value_evaluator()
        .set_belief_access_count("high_value_2", 8);
    controller
        .value_evaluator()
        .set_belief_last_accessed("high_value_2", 1);
    controller
        .value_evaluator()
        .set_belief_residual_association("high_value_2", 0.3);

    controller
        .value_evaluator()
        .register_belief("low_value_1", 0.1);
    controller
        .value_evaluator()
        .set_belief_access_count("low_value_1", 0);
    controller
        .value_evaluator()
        .set_belief_last_accessed("low_value_1", 100);
    controller
        .value_evaluator()
        .set_belief_residual_association("low_value_1", 0.0);

    controller
        .value_evaluator()
        .register_belief("low_value_2", 0.05);
    controller
        .value_evaluator()
        .set_belief_access_count("low_value_2", 0);
    controller
        .value_evaluator()
        .set_belief_last_accessed("low_value_2", 200);
    controller
        .value_evaluator()
        .set_belief_residual_association("low_value_2", 0.0);

    controller
        .forgetting_executor()
        .set_belief_age("low_value_1", 20);
    controller
        .forgetting_executor()
        .set_belief_age("low_value_2", 20);

    let eval_result = controller
        .evaluate_memory_value(EvaluateMemoryValueRequest {
            node_ids: Some(vec![
                "high_value_1".to_string(),
                "high_value_2".to_string(),
                "low_value_1".to_string(),
                "low_value_2".to_string(),
            ]),
            all_active: None,
            include_factors: Some(true),
        })
        .await
        .unwrap();

    assert_eq!(eval_result.value_scores.len(), 4);

    let high_scores: Vec<f64> = eval_result
        .value_scores
        .iter()
        .filter(|s| s.node_id.starts_with("high_value"))
        .map(|s| s.total_score)
        .collect();
    let low_scores: Vec<f64> = eval_result
        .value_scores
        .iter()
        .filter(|s| s.node_id.starts_with("low_value"))
        .map(|s| s.total_score)
        .collect();

    for hs in &high_scores {
        for ls in &low_scores {
            assert!(
                hs > ls,
                "high value score {} should exceed low value score {}",
                hs,
                ls
            );
        }
    }

    let forget_result = controller
        .force_forget(ForceForgetRequest {
            node_ids: vec!["low_value_1".to_string(), "low_value_2".to_string()],
            force_flag: None,
            reason: ForgetReason::LowValue,
        })
        .unwrap();

    assert!(
        forget_result
            .forgotten_ids
            .contains(&"low_value_1".to_string())
            || forget_result
                .forgotten_ids
                .contains(&"low_value_2".to_string())
    );

    let forget_status = controller.get_forget_status();
    assert!(forget_status.statistics.total_forgotten_this_session > 0);
}
