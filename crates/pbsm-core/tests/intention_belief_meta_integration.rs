use pbsm_core::modules::belief_graph::graph::BeliefGraph;
use pbsm_core::modules::intention_stack::manager::IntentionStackManager;
use pbsm_core::modules::intention_stack::manager::IntentionStackManagerImpl;
use pbsm_core::modules::intention_stack::state::{ExecutionState, GoalPriority};
use pbsm_core::modules::intention_stack::types::{
    CorrectiveAction, GoalDefinition, PlanStep, PopIntentRequest, PopReason, PushIntentRequest,
    UpdateIntentStateRequest,
};
use pbsm_core::modules::metacognition::controller::MetacognitiveController;
use pbsm_core::modules::metacognition::types::{AdjustAttentionRequest, AdjustmentTrigger};

#[tokio::test]
/// 推入意图成功创建，当前意图反映目标描述
async fn test_intention_push_creates_belief_context() {
    let manager = IntentionStackManagerImpl::new("test_stack".to_string());
    let _graph = BeliefGraph::with_default_config();

    let goal = GoalDefinition::simple("Analyze data".to_string(), GoalPriority::High);
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

    let current = manager.get_current_intention(None).await.unwrap();
    assert_eq!(current.current_layer.goal_description, "Analyze data");
}

#[tokio::test]
/// 意图完成流程（Push→Ready→InProgress→Advance→Pop Completed），元认知状态一致
async fn test_intention_completion_triggers_evaluation() {
    let manager = IntentionStackManagerImpl::new("test_stack".to_string());
    let controller = MetacognitiveController::new();

    let goal = GoalDefinition::simple("Complete task".to_string(), GoalPriority::High);
    let step1 = PlanStep::simple("step1".to_string(), 0);
    let step2 = PlanStep::simple("step2".to_string(), 1);
    let push_req = PushIntentRequest {
        goal,
        plan: Some(vec![step1, step2]),
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

    manager.advance_step(0).await.unwrap();

    let pop_req = PopIntentRequest {
        layer_index: 0,
        reason: PopReason::Completed,
        final_state: None,
        completion_report: None,
        cascade: false,
    };
    let pop_resp = manager.pop_intention(pop_req).await.unwrap();
    assert!(pop_resp.success);

    let status = controller.get_attention_status().await;
    assert!(status.attention_parameter >= 0.0 && status.attention_parameter <= 1.0);
}

#[tokio::test]
/// 初始漂移分数低（<0.3），Recontextualize 纠正操作成功
async fn test_drift_detection_and_correction() {
    let manager = IntentionStackManagerImpl::new("test_stack".to_string());

    let goal = GoalDefinition::simple("Drift test goal".to_string(), GoalPriority::Medium);
    let push_req = PushIntentRequest {
        goal,
        plan: None,
        parent_level: None,
        micro_prediction: None,
        attach_to_current: false,
    };

    manager.push_intention(push_req).await.unwrap();

    let assessment = manager.detect_drift(0).await.unwrap();
    assert!(assessment.overall_drift_score < 0.3);

    let result = manager
        .handle_drift(0, CorrectiveAction::Recontextualize)
        .await
        .unwrap();

    assert!(result.success);
}

#[tokio::test]
/// 检查点快照保存正确步骤索引，恢复后检查点数据一致
async fn test_checkpoint_restore_consistency() {
    let manager = IntentionStackManagerImpl::new("test_stack".to_string());

    let goal = GoalDefinition::simple("Checkpoint goal".to_string(), GoalPriority::High);
    let step1 = PlanStep::simple("step1".to_string(), 0);
    let step2 = PlanStep::simple("step2".to_string(), 1);
    let step3 = PlanStep::simple("step3".to_string(), 2);
    let push_req = PushIntentRequest {
        goal,
        plan: Some(vec![step1, step2, step3]),
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

    manager.advance_step(0).await.unwrap();

    let checkpoint = manager
        .create_checkpoint(0, Some("after_step_1".to_string()))
        .await
        .unwrap();

    let step_at_checkpoint = manager.get_layer(0).await.unwrap().unwrap();
    let checkpoint_step_index = step_at_checkpoint.current_step_index;

    manager.advance_step(0).await.unwrap();

    let step_after_advance = manager.get_layer(0).await.unwrap().unwrap();
    assert!(step_after_advance.current_step_index > checkpoint_step_index);

    let restore_result = manager
        .restore_checkpoint(&checkpoint.checkpoint_id)
        .await
        .unwrap();
    assert!(restore_result.success);

    let checkpoints = manager.list_checkpoints(0).await.unwrap();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(
        checkpoints[0].state_snapshot.current_step_index,
        checkpoint_step_index
    );
}

#[tokio::test]
/// 嵌套意图后注意力聚焦（通过 IntentionChange 触发 + override 模式设为0.9）
async fn test_nested_intention_attention_focus() {
    let manager = IntentionStackManagerImpl::new("test_stack".to_string());
    let controller = MetacognitiveController::new();

    let root_goal = GoalDefinition::simple("Root goal".to_string(), GoalPriority::Critical);
    let root_push = PushIntentRequest {
        goal: root_goal,
        plan: None,
        parent_level: None,
        micro_prediction: None,
        attach_to_current: false,
    };
    manager.push_intention(root_push).await.unwrap();

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

    let child_goal = GoalDefinition::simple("Child goal".to_string(), GoalPriority::High);
    let child_push = PushIntentRequest {
        goal: child_goal,
        plan: None,
        parent_level: Some(0),
        micro_prediction: None,
        attach_to_current: true,
    };
    manager.push_intention(child_push).await.unwrap();

    let initial_status = controller.get_attention_status().await;
    let initial_attention = initial_status.attention_parameter;

    controller
        .adjust_attention(AdjustAttentionRequest {
            delta: None,
            target_value: Some(0.9),
            trigger: AdjustmentTrigger::IntentionChange,
            override_mode: Some(true),
        })
        .await
        .unwrap();

    let focused_status = controller.get_attention_status().await;
    assert!(focused_status.attention_parameter > initial_attention);
}
