use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use super::config::IntentionStackConfig;
use super::error::Result;
use super::events::{IntentionStackEventPublisher, NullIntentionStackEventPublisher};
use super::operations::IntentionStackOperations;
use super::types::{
    AddStepResponse, CorrectiveAction, DriftAssessment, DriftHandlingResult, DriftThreshold,
    ExportedIntentionStack, GetCurrentIntentionRequest, GetCurrentIntentionResponse, ImportResult,
    IntentionLayer, IntentionStack, PlanStep, PopIntentRequest, PopIntentResponse,
    PushIntentRequest, PushIntentResponse, RemoveStepResponse, ReorderResponse, RevertResult,
    RevertToIntentionRequest, StepAdvanceResult, UpdateIntentStateRequest,
    UpdateIntentStateResponse,
};

#[async_trait]
pub trait IntentionStackManager: Send + Sync {
    async fn push_intention(&self, request: PushIntentRequest) -> Result<PushIntentResponse>;
    async fn pop_intention(&self, request: PopIntentRequest) -> Result<PopIntentResponse>;
    async fn update_intention_state(
        &self,
        request: UpdateIntentStateRequest,
    ) -> Result<UpdateIntentStateResponse>;
    async fn get_current_intention(
        &self,
        request: Option<GetCurrentIntentionRequest>,
    ) -> Result<GetCurrentIntentionResponse>;
    async fn revert_to_intention(&self, request: RevertToIntentionRequest) -> Result<RevertResult>;

    async fn get_layer(&self, layer_index: usize) -> Result<Option<IntentionLayer>>;
    async fn get_ancestors(&self, layer_index: usize) -> Result<Vec<IntentionLayer>>;
    async fn get_descendants(
        &self,
        layer_index: usize,
        depth_limit: Option<usize>,
    ) -> Result<Vec<IntentionLayer>>;
    async fn get_layer_by_id(&self, layer_id: &str) -> Result<Option<IntentionLayer>>;

    async fn advance_step(&self, layer_index: usize) -> Result<StepAdvanceResult>;
    async fn add_step(&self, layer_index: usize, step: PlanStep) -> Result<AddStepResponse>;
    async fn remove_step(
        &self,
        layer_index: usize,
        step_index: usize,
    ) -> Result<RemoveStepResponse>;
    async fn reorder_steps(
        &self,
        layer_index: usize,
        new_order: Vec<usize>,
    ) -> Result<ReorderResponse>;

    async fn detect_drift(&self, layer_index: usize) -> Result<DriftAssessment>;
    async fn handle_drift(
        &self,
        layer_index: usize,
        action: CorrectiveAction,
    ) -> Result<DriftHandlingResult>;
    async fn set_drift_threshold(&self, threshold: DriftThreshold);

    async fn get_stack_state(&self) -> Result<IntentionStack>;
    async fn get_stack_depth(&self) -> Result<usize>;
    async fn get_active_layers(&self) -> Result<Vec<IntentionLayer>>;
    async fn get_pending_layers(&self) -> Result<Vec<IntentionLayer>>;
    async fn get_completed_layers(&self) -> Result<Vec<IntentionLayer>>;

    async fn create_checkpoint(
        &self,
        layer_index: usize,
        label: Option<String>,
    ) -> Result<super::types::Checkpoint>;
    async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<super::types::RestoreResult>;
    async fn list_checkpoints(&self, layer_index: usize) -> Result<Vec<super::types::Checkpoint>>;
    async fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<bool>;

    async fn export_state(&self) -> Result<ExportedIntentionStack>;
    async fn import_state(&self, exported: ExportedIntentionStack) -> Result<ImportResult>;
}

pub struct IntentionStackManagerImpl {
    operations: IntentionStackOperations,
}

impl IntentionStackManagerImpl {
    pub fn new(stack_name: String) -> Self {
        let stack = Arc::new(RwLock::new(IntentionStack::new(stack_name)));
        let config = IntentionStackConfig::default();
        let event_publisher: Arc<dyn IntentionStackEventPublisher> =
            Arc::new(NullIntentionStackEventPublisher);
        let operations = IntentionStackOperations::new(stack, config, event_publisher);
        Self { operations }
    }

    pub fn with_config(stack_name: String, config: IntentionStackConfig) -> Self {
        let stack = Arc::new(RwLock::new(IntentionStack::new(stack_name)));
        let event_publisher: Arc<dyn IntentionStackEventPublisher> =
            Arc::new(NullIntentionStackEventPublisher);
        let operations = IntentionStackOperations::new(stack, config, event_publisher);
        Self { operations }
    }

    pub fn with_event_publisher(
        stack_name: String,
        config: IntentionStackConfig,
        event_publisher: Arc<dyn IntentionStackEventPublisher>,
    ) -> Self {
        let stack = Arc::new(RwLock::new(IntentionStack::new(stack_name)));
        let operations = IntentionStackOperations::new(stack, config, event_publisher);
        Self { operations }
    }
}

#[async_trait]
impl IntentionStackManager for IntentionStackManagerImpl {
    async fn push_intention(&self, request: PushIntentRequest) -> Result<PushIntentResponse> {
        self.operations.push_intent(request)
    }

    async fn pop_intention(&self, request: PopIntentRequest) -> Result<PopIntentResponse> {
        self.operations.pop_intent(request)
    }

    async fn update_intention_state(
        &self,
        request: UpdateIntentStateRequest,
    ) -> Result<UpdateIntentStateResponse> {
        self.operations.update_intent_state(request)
    }

    async fn get_current_intention(
        &self,
        request: Option<GetCurrentIntentionRequest>,
    ) -> Result<GetCurrentIntentionResponse> {
        self.operations.get_current_intention(request)
    }

    async fn revert_to_intention(&self, request: RevertToIntentionRequest) -> Result<RevertResult> {
        self.operations.revert_to_intention(request)
    }

    async fn get_layer(&self, layer_index: usize) -> Result<Option<IntentionLayer>> {
        self.operations.get_layer(layer_index)
    }

    async fn get_ancestors(&self, layer_index: usize) -> Result<Vec<IntentionLayer>> {
        self.operations.get_ancestors(layer_index)
    }

    async fn get_descendants(
        &self,
        layer_index: usize,
        depth_limit: Option<usize>,
    ) -> Result<Vec<IntentionLayer>> {
        self.operations.get_descendants(layer_index, depth_limit)
    }

    async fn get_layer_by_id(&self, layer_id: &str) -> Result<Option<IntentionLayer>> {
        self.operations.get_layer_by_id(layer_id)
    }

    async fn advance_step(&self, layer_index: usize) -> Result<StepAdvanceResult> {
        self.operations.advance_step(layer_index)
    }

    async fn add_step(&self, layer_index: usize, step: PlanStep) -> Result<AddStepResponse> {
        self.operations.add_step(layer_index, step)
    }

    async fn remove_step(
        &self,
        layer_index: usize,
        step_index: usize,
    ) -> Result<RemoveStepResponse> {
        self.operations.remove_step(layer_index, step_index)
    }

    async fn reorder_steps(
        &self,
        layer_index: usize,
        new_order: Vec<usize>,
    ) -> Result<ReorderResponse> {
        self.operations.reorder_steps(layer_index, new_order)
    }

    async fn detect_drift(&self, layer_index: usize) -> Result<DriftAssessment> {
        self.operations.detect_drift(layer_index)
    }

    async fn handle_drift(
        &self,
        layer_index: usize,
        action: CorrectiveAction,
    ) -> Result<DriftHandlingResult> {
        self.operations.handle_drift(layer_index, action)
    }

    async fn set_drift_threshold(&self, threshold: DriftThreshold) {
        self.operations.set_drift_threshold(threshold)
    }

    async fn get_stack_state(&self) -> Result<IntentionStack> {
        self.operations.get_stack_state()
    }

    async fn get_stack_depth(&self) -> Result<usize> {
        self.operations.get_stack_depth()
    }

    async fn get_active_layers(&self) -> Result<Vec<IntentionLayer>> {
        self.operations.get_active_layers()
    }

    async fn get_pending_layers(&self) -> Result<Vec<IntentionLayer>> {
        self.operations.get_pending_layers()
    }

    async fn get_completed_layers(&self) -> Result<Vec<IntentionLayer>> {
        self.operations.get_completed_layers()
    }

    async fn create_checkpoint(
        &self,
        layer_index: usize,
        label: Option<String>,
    ) -> Result<super::types::Checkpoint> {
        self.operations.create_checkpoint(layer_index, label)
    }

    async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<super::types::RestoreResult> {
        self.operations.restore_checkpoint(checkpoint_id)
    }

    async fn list_checkpoints(&self, layer_index: usize) -> Result<Vec<super::types::Checkpoint>> {
        self.operations.list_checkpoints(layer_index)
    }

    async fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<bool> {
        self.operations.delete_checkpoint(checkpoint_id)
    }

    async fn export_state(&self) -> Result<ExportedIntentionStack> {
        self.operations.export_state()
    }

    async fn import_state(&self, exported: ExportedIntentionStack) -> Result<ImportResult> {
        self.operations.import_state(exported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::intention_stack::state::{ExecutionState, GoalPriority};
    use crate::modules::intention_stack::types::{
        GoalDefinition, PopReason, PushIntentRequest, UpdateIntentStateRequest,
    };

    fn create_test_manager() -> IntentionStackManagerImpl {
        IntentionStackManagerImpl::new("test_stack".to_string())
    }

    #[tokio::test]
    async fn test_push_pop_workflow() {
        let manager = create_test_manager();

        let goal = GoalDefinition::simple("Test goal".to_string(), GoalPriority::High);
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
    async fn test_push_update_advance_pop() {
        let manager = create_test_manager();

        let goal = GoalDefinition::simple("Multi-step goal".to_string(), GoalPriority::High);
        let step1 = PlanStep::simple("step1".to_string(), 0);
        let step2 = PlanStep::simple("step2".to_string(), 1);
        let push_req = PushIntentRequest {
            goal,
            plan: Some(vec![step1, step2]),
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };

        let push_resp = manager.push_intention(push_req).await.unwrap();
        assert!(push_resp.success);

        let update_req = UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::Ready,
            transition_context: None,
            force: false,
        };
        let update_resp = manager.update_intention_state(update_req).await.unwrap();
        assert!(update_resp.success);
        assert_eq!(update_resp.current_state, ExecutionState::Ready);

        let update_req2 = UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::InProgress,
            transition_context: None,
            force: false,
        };
        let update_resp2 = manager.update_intention_state(update_req2).await.unwrap();
        assert!(update_resp2.success);

        let advance_resp = manager.advance_step(0).await.unwrap();
        assert!(advance_resp.success);

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
    async fn test_nested_intentions() {
        let manager = create_test_manager();

        let root_goal = GoalDefinition::simple("Root goal".to_string(), GoalPriority::Critical);
        let push_req = PushIntentRequest {
            goal: root_goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        let push_resp = manager.push_intention(push_req).await.unwrap();
        assert!(push_resp.success);

        let update_req = UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::Ready,
            transition_context: None,
            force: false,
        };
        manager.update_intention_state(update_req).await.unwrap();

        let update_req2 = UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::InProgress,
            transition_context: None,
            force: false,
        };
        manager.update_intention_state(update_req2).await.unwrap();

        let child_goal = GoalDefinition::simple("Child goal".to_string(), GoalPriority::High);
        let push_req2 = PushIntentRequest {
            goal: child_goal,
            plan: None,
            parent_level: Some(0),
            micro_prediction: None,
            attach_to_current: true,
        };
        let push_resp2 = manager.push_intention(push_req2).await.unwrap();
        assert!(push_resp2.success);
        assert_eq!(push_resp2.parent_layer_id, Some(push_resp.layer_id.clone()));
    }

    #[tokio::test]
    async fn test_invalid_state_transition() {
        let manager = create_test_manager();

        let goal = GoalDefinition::simple("Test".to_string(), GoalPriority::Medium);
        let push_req = PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        manager.push_intention(push_req).await.unwrap();

        let update_req = UpdateIntentStateRequest {
            layer_index: 0,
            new_state: ExecutionState::Completed,
            transition_context: None,
            force: false,
        };
        let resp = manager.update_intention_state(update_req).await.unwrap();
        assert!(!resp.success);
        assert!(!resp.state_change_allowed);
    }

    #[tokio::test]
    async fn test_get_current_intention() {
        let manager = create_test_manager();

        let goal = GoalDefinition::simple("Current".to_string(), GoalPriority::High);
        let push_req = PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        manager.push_intention(push_req).await.unwrap();

        let resp = manager.get_current_intention(None).await.unwrap();
        assert_eq!(resp.current_layer.goal_description, "Current");
    }

    #[tokio::test]
    async fn test_get_current_intention_empty() {
        let manager = create_test_manager();
        let resp = manager.get_current_intention(None).await;
        assert!(resp.is_err());
    }

    #[tokio::test]
    async fn test_add_and_remove_step() {
        let manager = create_test_manager();

        let goal = GoalDefinition::simple("Test".to_string(), GoalPriority::Medium);
        let push_req = PushIntentRequest {
            goal,
            plan: Some(vec![]),
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        manager.push_intention(push_req).await.unwrap();

        let step = PlanStep::simple("new_step".to_string(), 0);
        let add_resp = manager.add_step(0, step).await.unwrap();
        assert!(add_resp.success);

        let state = manager.get_stack_state().await.unwrap();
        assert_eq!(state.layers[0].plan.len(), 1);

        let remove_resp = manager.remove_step(0, 0).await.unwrap();
        assert!(remove_resp.success);

        let state = manager.get_stack_state().await.unwrap();
        assert!(state.layers[0].plan.is_empty());
    }

    #[tokio::test]
    async fn test_export_import() {
        let manager = create_test_manager();

        let goal = GoalDefinition::simple("Export test".to_string(), GoalPriority::High);
        let push_req = PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        manager.push_intention(push_req).await.unwrap();

        let exported = manager.export_state().await.unwrap();
        assert_eq!(exported.stack.layers.len(), 1);

        let manager2 = create_test_manager();
        let import_result = manager2.import_state(exported).await.unwrap();
        assert!(import_result.success);
        assert_eq!(import_result.imported_layers, 1);
    }

    #[tokio::test]
    async fn test_drift_detection() {
        let manager = create_test_manager();

        let goal = GoalDefinition::simple("Drift test".to_string(), GoalPriority::Medium);
        let push_req = PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        manager.push_intention(push_req).await.unwrap();

        let assessment = manager.detect_drift(0).await.unwrap();
        assert_eq!(
            assessment.layer_id,
            manager.get_layer(0).await.unwrap().unwrap().layer_id
        );
    }

    #[tokio::test]
    async fn test_checkpoint_lifecycle() {
        let manager = create_test_manager();

        let goal = GoalDefinition::simple("Checkpoint test".to_string(), GoalPriority::Medium);
        let push_req = PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        manager.push_intention(push_req).await.unwrap();

        let checkpoint = manager
            .create_checkpoint(0, Some("test_cp".to_string()))
            .await
            .unwrap();
        assert_eq!(checkpoint.label, Some("test_cp".to_string()));

        let checkpoints = manager.list_checkpoints(0).await.unwrap();
        assert_eq!(checkpoints.len(), 1);

        let restore = manager
            .restore_checkpoint(&checkpoint.checkpoint_id)
            .await
            .unwrap();
        assert!(restore.success);

        let delete = manager
            .delete_checkpoint(&checkpoint.checkpoint_id)
            .await
            .unwrap();
        assert!(delete);
    }

    #[tokio::test]
    async fn test_push_empty_goal_description() {
        let manager = create_test_manager();
        let goal = GoalDefinition::simple("".to_string(), GoalPriority::Medium);
        let push_req = PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        let result = manager.push_intention(push_req).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_stack_depth() {
        let manager = create_test_manager();
        assert_eq!(manager.get_stack_depth().await.unwrap(), 0);

        let goal = GoalDefinition::simple("Test".to_string(), GoalPriority::Medium);
        let push_req = PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        manager.push_intention(push_req).await.unwrap();
        assert_eq!(manager.get_stack_depth().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_get_active_pending_completed_layers() {
        let manager = create_test_manager();

        let goal1 = GoalDefinition::simple("Active".to_string(), GoalPriority::High);
        let push_req1 = PushIntentRequest {
            goal: goal1,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };
        manager.push_intention(push_req1).await.unwrap();

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

        let active = manager.get_active_layers().await.unwrap();
        assert_eq!(active.len(), 1);

        let pending = manager.get_pending_layers().await.unwrap();
        assert_eq!(pending.len(), 0);

        let completed = manager.get_completed_layers().await.unwrap();
        assert_eq!(completed.len(), 0);
    }

    #[tokio::test]
    async fn test_set_drift_threshold() {
        let manager = create_test_manager();
        let new_threshold = DriftThreshold {
            warning: 0.2,
            moderate: 0.4,
            severe: 0.6,
            critical: 0.8,
        };
        manager.set_drift_threshold(new_threshold).await;
    }
}
