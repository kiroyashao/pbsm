use chrono::Utc;
use std::sync::Arc;

use super::checkpoint::CheckpointManager;
use super::config::IntentionStackConfig;
use super::drift::DriftDetector;
use super::error::{IntentionStackError, Result};
use super::events::{
    IntentionPushedPayload, IntentionStackEvent, IntentionStackEventPublisher, StateChangedPayload,
};
use super::state::ExecutionState;
use super::types::{
    AddStepResponse, ChildStateUpdate, CorrectiveAction, DriftThreshold, ExecutionContext,
    ExportedIntentionStack, GetCurrentIntentionRequest, GetCurrentIntentionResponse, ImportResult,
    IntentionLayer, IntentionLayerSummary, IntentionStack, PlanStep, PlanStepSummary,
    PopIntentRequest, PopIntentResponse, PopReason, PushIntentRequest, PushIntentResponse,
    RemoveStepResponse, ReorderResponse, RevertMode, RevertResult, RolledBackLayerInfo,
    StackPushState, StepAdvanceResult, UpdateIntentStateRequest, UpdateIntentStateResponse,
};
use parking_lot::RwLock;

pub struct IntentionStackOperations {
    stack: Arc<RwLock<IntentionStack>>,
    checkpoint_manager: Arc<RwLock<CheckpointManager>>,
    drift_detector: Arc<RwLock<DriftDetector>>,
    event_publisher: Arc<dyn IntentionStackEventPublisher>,
    config: IntentionStackConfig,
}

impl IntentionStackOperations {
    pub fn new(
        stack: Arc<RwLock<IntentionStack>>,
        config: IntentionStackConfig,
        event_publisher: Arc<dyn IntentionStackEventPublisher>,
    ) -> Self {
        let cp_manager = CheckpointManager::with_event_publisher(
            config.max_checkpoints_per_layer,
            event_publisher.clone(),
        );
        let drift = DriftDetector::new(config.drift_threshold.clone());
        Self {
            stack,
            checkpoint_manager: Arc::new(RwLock::new(cp_manager)),
            drift_detector: Arc::new(RwLock::new(drift)),
            event_publisher,
            config,
        }
    }

    pub fn push_intent(&self, request: PushIntentRequest) -> Result<PushIntentResponse> {
        let mut stack = self.stack.write();

        if stack.layers.len() >= IntentionStack::MAX_STACK_CAPACITY {
            return Err(IntentionStackError::StackCapacityExceeded {
                max: IntentionStack::MAX_STACK_CAPACITY,
                current: stack.layers.len(),
            });
        }

        if request.goal.description.is_empty() {
            return Err(IntentionStackError::InvalidGoal {
                reason: "Goal description cannot be empty".to_string(),
            });
        }

        let parent_level = request.parent_level.unwrap_or(if stack.is_empty() {
            0
        } else {
            stack.active_goal_pointer
        });

        let effective_depth = if stack.is_empty() {
            0
        } else if parent_level < stack.layers.len() {
            stack.layers[parent_level].level + 1
        } else if request.parent_level.is_some() {
            return Err(IntentionStackError::InvalidParentLevel {
                level: parent_level,
            });
        } else {
            0
        };

        if effective_depth >= stack.max_depth {
            return Err(IntentionStackError::MaxDepthExceeded {
                max: stack.max_depth,
                attempted: effective_depth,
            });
        }

        if !stack.is_empty() && parent_level < stack.layers.len() {
            let max_children = match stack.layers[parent_level].level {
                0 => 10,
                1..=3 => 5,
                4..=7 => 3,
                8..=15 => 2,
                _ => 1,
            };
            if stack.layers[parent_level].child_levels.len() >= max_children {
                return Err(IntentionStackError::ChildLimitExceeded {
                    level: stack.layers[parent_level].level,
                    max: max_children,
                    current: stack.layers[parent_level].child_levels.len(),
                });
            }
        }

        let actual_parent = if stack.is_empty() {
            None
        } else {
            Some(parent_level)
        };

        let mut new_layer = IntentionLayer::new(request.goal, effective_depth, actual_parent);

        if let Some(plan) = request.plan {
            new_layer.plan = plan;
            new_layer.progress_metrics.total_steps = new_layer.plan.len();
        }

        if let Some(pred) = request.micro_prediction {
            new_layer.micro_prediction = Some(pred);
        }

        let layer_id = new_layer.layer_id.clone();
        let parent_layer_id =
            actual_parent.and_then(|p| stack.layers.get(p).map(|l| l.layer_id.clone()));

        let layer_index = if request.attach_to_current && !stack.is_empty() {
            let insert_pos = stack.active_goal_pointer + 1;
            if insert_pos < stack.layers.len() {
                stack.layers.insert(insert_pos, new_layer);
            } else {
                stack.layers.push(new_layer);
            }
            insert_pos
        } else {
            stack.layers.push(new_layer);
            stack.layers.len() - 1
        };

        if let Some(p_level) = actual_parent {
            if let Some(parent) = stack.layers.get_mut(p_level) {
                parent.child_levels.push(layer_index);
                if request.attach_to_current {
                    parent.metadata.updated_at = Utc::now();
                }
            }
        }

        if stack.is_empty() || stack.layers.len() == 1 {
            stack.metadata.root_intention_id = Some(layer_id.clone());
        }

        stack.active_goal_pointer = layer_index;
        stack.metadata.updated_at = Utc::now();
        stack.metadata.version += 1;

        let stack_state = if request.attach_to_current && actual_parent.is_some() {
            StackPushState::Nested
        } else {
            StackPushState::Pushed
        };

        let goal_desc = stack
            .layers
            .get(layer_index)
            .map(|l| l.goal.description.clone())
            .unwrap_or_default();
        let plan_len = stack
            .layers
            .get(layer_index)
            .map(|l| l.plan.len())
            .unwrap_or(0);

        let _ = self
            .event_publisher
            .publish(IntentionStackEvent::IntentionPushed(
                IntentionPushedPayload {
                    stack_id: stack.stack_id.clone(),
                    layer_id: layer_id.clone(),
                    layer_index,
                    level: effective_depth,
                    parent_layer_id: parent_layer_id.clone(),
                    parent_level: actual_parent,
                    goal_description: goal_desc,
                    priority: stack
                        .layers
                        .get(layer_index)
                        .map(|l| l.goal.priority)
                        .unwrap_or(super::state::GoalPriority::Medium),
                    plan_length: plan_len,
                },
            ));

        Ok(PushIntentResponse {
            success: true,
            layer_index,
            layer_id,
            stack_state,
            parent_layer_id,
            warnings: Vec::new(),
        })
    }

    pub fn pop_intent(&self, request: PopIntentRequest) -> Result<PopIntentResponse> {
        let mut stack = self.stack.write();

        if request.layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: request.layer_index,
                max: stack.layers.len(),
            });
        }

        let target_layer = &stack.layers[request.layer_index];

        if target_layer.execution_state.is_terminal() {
            return Err(IntentionStackError::PopFailed(format!(
                "Layer already in terminal state: {:?}",
                target_layer.execution_state
            )));
        }

        let mut removed_layers = Vec::new();
        let mut layers_to_remove = vec![request.layer_index];

        if request.cascade {
            let mut child_queue: Vec<usize> = vec![request.layer_index];
            while let Some(idx) = child_queue.pop() {
                if let Some(layer) = stack.layers.get(idx) {
                    for &child in &layer.child_levels {
                        if child < stack.layers.len() && !layers_to_remove.contains(&child) {
                            layers_to_remove.push(child);
                            child_queue.push(child);
                        }
                    }
                }
            }
        }

        layers_to_remove.sort_unstable();
        layers_to_remove.dedup();

        for &idx in &layers_to_remove {
            if idx < stack.layers.len() {
                let layer = &stack.layers[idx];
                let final_state = request.final_state.unwrap_or(match request.reason {
                    PopReason::Completed => ExecutionState::Completed,
                    PopReason::Abandoned => ExecutionState::Abandoned,
                    PopReason::Failed => ExecutionState::Failed,
                    PopReason::UserRequest => ExecutionState::Abandoned,
                });
                removed_layers.push(super::types::RemovedLayerInfo {
                    layer_id: layer.layer_id.clone(),
                    level: layer.level,
                    final_state,
                });
            }
        }

        for &idx in &layers_to_remove {
            if idx < stack.layers.len() {
                if let Some(parent_idx) = stack.layers[idx].parent_level {
                    if let Some(parent) = stack.layers.get_mut(parent_idx) {
                        parent.child_levels.retain(|&c| c != idx);
                    }
                }
            }
        }

        let old_to_new = Self::build_reindex_map(stack.layers.len(), &layers_to_remove);

        for &idx in layers_to_remove.iter().rev() {
            if idx < stack.layers.len() {
                stack.layers.remove(idx);
            }
        }

        Self::apply_reindex(&mut stack, &old_to_new);

        if stack.active_goal_pointer >= stack.layers.len() && !stack.layers.is_empty() {
            stack.active_goal_pointer = stack.layers.len() - 1;
        } else if stack.layers.is_empty() {
            stack.active_goal_pointer = 0;
        }

        match request.reason {
            PopReason::Completed => stack.metadata.completed_count += 1,
            PopReason::Abandoned | PopReason::UserRequest => stack.metadata.abandoned_count += 1,
            PopReason::Failed => {}
        }

        stack.metadata.updated_at = Utc::now();
        stack.metadata.version += 1;

        let next_intention_id = stack.current_layer().map(|l| l.layer_id.clone());

        for removed in &removed_layers {
            let _ = self
                .event_publisher
                .publish(IntentionStackEvent::IntentionPopped(
                    super::events::IntentionPoppedPayload {
                        layer_id: removed.layer_id.clone(),
                        layer_index: 0,
                        reason: request.reason,
                    },
                ));
        }

        Ok(PopIntentResponse {
            success: true,
            removed_layers,
            promoted_child: None,
            belief_updates: Vec::new(),
            parent_updated: true,
            next_intention_id,
        })
    }

    pub fn update_intent_state(
        &self,
        request: UpdateIntentStateRequest,
    ) -> Result<UpdateIntentStateResponse> {
        let mut stack = self.stack.write();

        if request.layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: request.layer_index,
                max: stack.layers.len(),
            });
        }

        let previous_state = stack.layers[request.layer_index].execution_state;

        if !previous_state.can_transition_to(&request.new_state) && !request.force {
            return Ok(UpdateIntentStateResponse {
                success: false,
                previous_state,
                current_state: previous_state,
                state_change_allowed: false,
                blocked_reasons: vec![format!(
                    "Transition from {} to {} is not allowed",
                    previous_state, request.new_state
                )],
                side_effects: Vec::new(),
                child_state_updates: Vec::new(),
                event_emitted: String::new(),
            });
        }

        if !previous_state.can_transition_to(&request.new_state) && request.force {
            return Err(IntentionStackError::InvalidStateTransition {
                from: previous_state,
                to: request.new_state,
            });
        }

        let layer = &mut stack.layers[request.layer_index];
        layer.execution_state = request.new_state;
        layer.metadata.last_state_change = Utc::now();
        layer.metadata.state_change_count += 1;
        layer.metadata.version += 1;
        layer.metadata.updated_at = Utc::now();

        let mut child_state_updates = Vec::new();
        if matches!(
            request.new_state,
            ExecutionState::Suspended | ExecutionState::Abandoned | ExecutionState::Failed
        ) {
            let child_levels: Vec<usize> = layer.child_levels.clone();
            for child_level in child_levels {
                if child_level < stack.layers.len() {
                    let child_prev = stack.layers[child_level].execution_state;
                    let inherited = match request.new_state {
                        ExecutionState::Suspended => ExecutionState::Suspended,
                        ExecutionState::Abandoned => ExecutionState::Abandoned,
                        ExecutionState::Failed => ExecutionState::Failed,
                        _ => child_prev,
                    };
                    if child_prev != inherited && child_prev.can_transition_to(&inherited) {
                        stack.layers[child_level].execution_state = inherited;
                        stack.layers[child_level].metadata.last_state_change = Utc::now();
                        stack.layers[child_level].metadata.state_change_count += 1;
                        child_state_updates.push(ChildStateUpdate {
                            child_level,
                            previous_state: child_prev,
                            new_state: inherited,
                        });
                    }
                }
            }
        }

        stack.metadata.updated_at = Utc::now();
        stack.metadata.version += 1;

        let layer_id = stack.layers[request.layer_index].layer_id.clone();
        let _ = self
            .event_publisher
            .publish(IntentionStackEvent::StateChanged(StateChangedPayload {
                layer_id,
                layer_index: request.layer_index,
                previous_state,
                current_state: request.new_state,
                trigger: request
                    .transition_context
                    .as_ref()
                    .map(|c| format!("{:?}", c.trigger))
                    .unwrap_or_default(),
                child_updates: child_state_updates.len(),
            }));

        Ok(UpdateIntentStateResponse {
            success: true,
            previous_state,
            current_state: request.new_state,
            state_change_allowed: true,
            blocked_reasons: Vec::new(),
            side_effects: Vec::new(),
            child_state_updates,
            event_emitted: "STATE_CHANGED".to_string(),
        })
    }

    pub fn get_current_intention(
        &self,
        request: Option<GetCurrentIntentionRequest>,
    ) -> Result<GetCurrentIntentionResponse> {
        let stack = self.stack.read();
        let req = request.unwrap_or_default();

        if stack.is_empty() {
            return Err(IntentionStackError::NoActiveIntention);
        }

        let current_layer = stack
            .current_layer()
            .ok_or(IntentionStackError::NoActiveIntention)?;

        let current_summary = Self::create_layer_summary(current_layer);

        let mut ancestors = Vec::new();
        let mut current_parent = current_layer.parent_level;
        while let Some(p_level) = current_parent {
            if p_level < stack.layers.len() {
                let ancestor = &stack.layers[p_level];
                ancestors.push(Self::create_layer_summary(ancestor));
                current_parent = ancestor.parent_level;
            } else {
                break;
            }
        }
        ancestors.reverse();

        let depth_limit = req.depth_limit.unwrap_or(1);
        let mut children = Vec::new();
        for &child_level in &current_layer.child_levels {
            if children.len() >= depth_limit {
                break;
            }
            if child_level < stack.layers.len() {
                children.push(Self::create_layer_summary(&stack.layers[child_level]));
            }
        }

        let execution_context = Self::build_execution_context(current_layer, &req);

        let top_level_goal = stack
            .find_root_layer()
            .map(|root| super::types::GoalSummary {
                goal_id: root.goal.goal_id.clone(),
                description: root.goal.description.clone(),
                priority: root.goal.priority,
                overall_progress: root.progress_metrics.progress_percentage,
            });

        let mut breadcrumbs = Vec::new();
        for ancestor in &ancestors {
            breadcrumbs.push(super::types::Breadcrumb {
                level: ancestor.level,
                layer_id: ancestor.layer_id.clone(),
                goal_description: ancestor.goal_description.clone(),
                execution_state: ancestor.execution_state,
                is_current: false,
            });
        }
        breadcrumbs.push(super::types::Breadcrumb {
            level: current_summary.level,
            layer_id: current_summary.layer_id.clone(),
            goal_description: current_summary.goal_description.clone(),
            execution_state: current_summary.execution_state,
            is_current: true,
        });

        let mut recommendations = Vec::new();
        match current_layer.execution_state {
            ExecutionState::Pending => {
                recommendations.push(super::types::Recommendation {
                    recommendation_type: "start".to_string(),
                    action: "Begin execution".to_string(),
                });
            }
            ExecutionState::WaitingFeedback => {
                recommendations.push(super::types::Recommendation {
                    recommendation_type: "monitor".to_string(),
                    action: "Await feedback".to_string(),
                });
            }
            _ => {}
        }
        if current_layer.drift_status.is_drifting {
            recommendations.push(super::types::Recommendation {
                recommendation_type: "correct".to_string(),
                action: "Address drift".to_string(),
            });
        }

        Ok(GetCurrentIntentionResponse {
            current_layer: current_summary,
            ancestors,
            children,
            execution_context,
            drift_status: current_layer.drift_status.clone(),
            top_level_goal,
            breadcrumbs,
            recommendations,
        })
    }

    pub fn revert_to_intention(
        &self,
        request: super::types::RevertToIntentionRequest,
    ) -> Result<RevertResult> {
        let mut stack = self.stack.write();

        if request.target_layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: request.target_layer_index,
                max: stack.layers.len(),
            });
        }

        if request.target_layer_index >= stack.active_goal_pointer && !stack.is_empty() {
            return Err(IntentionStackError::RevertFailed(
                "Cannot revert to current or future layer".to_string(),
            ));
        }

        let current_pointer = stack.active_goal_pointer;
        let revert_depth = current_pointer - request.target_layer_index;

        if revert_depth > self.config.max_revert_depth {
            return Err(IntentionStackError::RevertFailed(format!(
                "Revert depth {} exceeds maximum allowed ({})",
                revert_depth, self.config.max_revert_depth
            )));
        }

        let mut rolled_back_layers = Vec::new();
        let mut invalidated_predictions = Vec::new();

        for idx in (request.target_layer_index..=current_pointer).rev() {
            if idx >= stack.layers.len() {
                continue;
            }
            let layer = &mut stack.layers[idx];
            let previous_state = layer.execution_state;

            let new_state = match request.revert_mode {
                RevertMode::Checkpoint => ExecutionState::Ready,
                RevertMode::StateOnly | RevertMode::Full => ExecutionState::Ready,
            };

            layer.execution_state = new_state;

            if !request.preserve_completed_steps {
                layer.current_step_index = 0;
                for step in &mut layer.plan {
                    step.actual_outcome = None;
                    step.retry_count = 0;
                }
            }

            layer.progress_metrics.completed_steps = 0;
            layer.progress_metrics.progress_percentage = 0.0;

            layer.drift_status.is_drifting = false;
            layer.drift_status.drift_angle = 0.0;

            if let Some(pred) = &layer.micro_prediction {
                invalidated_predictions.push(pred.prediction_id.clone());
            }
            layer.micro_prediction = None;

            layer.metadata.version += 1;

            rolled_back_layers.push(RolledBackLayerInfo {
                layer_id: layer.layer_id.clone(),
                previous_state,
                new_state,
                steps_reverted: layer.current_step_index,
            });
        }

        let layers_after_target: Vec<usize> = (request.target_layer_index + 1..stack.layers.len())
            .filter(|&idx| {
                let layer = &stack.layers[idx];
                layer
                    .parent_level
                    .map_or(true, |p| p >= request.target_layer_index)
                    && !rolled_back_layers
                        .iter()
                        .any(|r| r.layer_id == layer.layer_id)
            })
            .collect();

        let old_to_new = Self::build_reindex_map(stack.layers.len(), &layers_after_target);

        for &idx in layers_after_target.iter().rev() {
            if idx < stack.layers.len() {
                stack.layers.remove(idx);
            }
        }

        Self::apply_reindex(&mut stack, &old_to_new);

        stack.active_goal_pointer = request
            .target_layer_index
            .min(stack.layers.len().saturating_sub(1));
        stack.metadata.updated_at = Utc::now();
        stack.metadata.version += 1;

        let _ = self
            .event_publisher
            .publish(IntentionStackEvent::IntentionReverted(
                super::events::IntentionRevertedPayload {
                    target_layer_index: request.target_layer_index,
                    rolled_back_count: rolled_back_layers.len(),
                    new_current_index: stack.active_goal_pointer,
                },
            ));

        Ok(RevertResult {
            success: true,
            revert_depth,
            rolled_back_layers,
            restored_checkpoint: request.checkpoint_id,
            belief_restorations: Vec::new(),
            invalidated_predictions,
            new_current_layer_index: stack.active_goal_pointer,
            warnings: Vec::new(),
        })
    }

    pub fn advance_step(&self, layer_index: usize) -> Result<StepAdvanceResult> {
        let mut stack = self.stack.write();

        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let layer = &mut stack.layers[layer_index];

        if layer.plan.is_empty() {
            return Ok(StepAdvanceResult {
                success: false,
                new_step_index: 0,
                completed: false,
            });
        }

        layer.current_step_index += 1;
        let completed = layer.current_step_index >= layer.plan.len();

        if completed {
            layer.current_step_index = layer.plan.len() - 1;
        }

        layer.update_progress();
        layer.metadata.updated_at = Utc::now();

        Ok(StepAdvanceResult {
            success: true,
            new_step_index: layer.current_step_index,
            completed,
        })
    }

    pub fn add_step(&self, layer_index: usize, mut step: PlanStep) -> Result<AddStepResponse> {
        let mut stack = self.stack.write();

        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let layer = &mut stack.layers[layer_index];
        step.step_index = layer.plan.len();
        let step_id = step.step_id.clone();
        let step_index = step.step_index;

        layer.plan.push(step);
        layer.progress_metrics.total_steps = layer.plan.len();
        layer.metadata.updated_at = Utc::now();

        Ok(AddStepResponse {
            success: true,
            step_id,
            step_index,
        })
    }

    pub fn remove_step(&self, layer_index: usize, step_index: usize) -> Result<RemoveStepResponse> {
        let mut stack = self.stack.write();

        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let layer = &mut stack.layers[layer_index];

        if step_index >= layer.plan.len() {
            return Err(IntentionStackError::Internal(format!(
                "Step index {} out of range [0, {})",
                step_index,
                layer.plan.len()
            )));
        }

        layer.plan.remove(step_index);

        for (i, step) in layer.plan.iter_mut().enumerate() {
            step.step_index = i;
        }

        layer.progress_metrics.total_steps = layer.plan.len();
        if layer.current_step_index >= layer.plan.len() && !layer.plan.is_empty() {
            layer.current_step_index = layer.plan.len() - 1;
        }
        layer.update_progress();
        layer.metadata.updated_at = Utc::now();

        Ok(RemoveStepResponse {
            success: true,
            removed_step_index: step_index,
        })
    }

    pub fn reorder_steps(
        &self,
        layer_index: usize,
        new_order: Vec<usize>,
    ) -> Result<ReorderResponse> {
        let mut stack = self.stack.write();

        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let layer = &mut stack.layers[layer_index];

        if new_order.len() != layer.plan.len() {
            return Err(IntentionStackError::Internal(format!(
                "New order length {} doesn't match plan length {}",
                new_order.len(),
                layer.plan.len()
            )));
        }

        let mut reordered = Vec::with_capacity(layer.plan.len());
        for &idx in &new_order {
            if idx >= layer.plan.len() {
                return Err(IntentionStackError::Internal(format!(
                    "Index {} out of range in new order",
                    idx
                )));
            }
            reordered.push(layer.plan[idx].clone());
        }

        for (i, step) in reordered.iter_mut().enumerate() {
            step.step_index = i;
        }

        layer.plan = reordered;
        layer.metadata.updated_at = Utc::now();

        Ok(ReorderResponse {
            success: true,
            new_order,
        })
    }

    pub fn get_layer(&self, layer_index: usize) -> Result<Option<IntentionLayer>> {
        let stack = self.stack.read();
        if layer_index >= stack.layers.len() {
            return Ok(None);
        }
        Ok(Some(stack.layers[layer_index].clone()))
    }

    pub fn get_ancestors(&self, layer_index: usize) -> Result<Vec<IntentionLayer>> {
        let stack = self.stack.read();
        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let mut ancestors = Vec::new();
        let mut current_parent = stack.layers[layer_index].parent_level;
        while let Some(p_level) = current_parent {
            if p_level < stack.layers.len() {
                ancestors.push(stack.layers[p_level].clone());
                current_parent = stack.layers[p_level].parent_level;
            } else {
                break;
            }
        }
        ancestors.reverse();
        Ok(ancestors)
    }

    pub fn get_descendants(
        &self,
        layer_index: usize,
        depth_limit: Option<usize>,
    ) -> Result<Vec<IntentionLayer>> {
        let stack = self.stack.read();
        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let limit = depth_limit.unwrap_or(usize::MAX);
        let mut descendants = Vec::new();
        let mut queue: Vec<(usize, usize)> = stack.layers[layer_index]
            .child_levels
            .iter()
            .map(|&c| (c, 1))
            .collect();

        while let Some((child_idx, depth)) = queue.pop() {
            if depth > limit || child_idx >= stack.layers.len() {
                continue;
            }
            descendants.push(stack.layers[child_idx].clone());
            for &grandchild in &stack.layers[child_idx].child_levels {
                queue.push((grandchild, depth + 1));
            }
        }

        Ok(descendants)
    }

    pub fn get_layer_by_id(&self, layer_id: &str) -> Result<Option<IntentionLayer>> {
        let stack = self.stack.read();
        Ok(stack
            .layers
            .iter()
            .find(|l| l.layer_id == layer_id)
            .cloned())
    }

    pub fn get_stack_state(&self) -> Result<IntentionStack> {
        let stack = self.stack.read();
        Ok(stack.clone())
    }

    pub fn get_stack_depth(&self) -> Result<usize> {
        let stack = self.stack.read();
        Ok(stack.depth())
    }

    pub fn get_active_layers(&self) -> Result<Vec<IntentionLayer>> {
        let stack = self.stack.read();
        Ok(stack
            .layers
            .iter()
            .filter(|l| {
                l.execution_state == ExecutionState::InProgress
                    || l.execution_state == ExecutionState::WaitingFeedback
                    || l.execution_state == ExecutionState::Ready
            })
            .cloned()
            .collect())
    }

    pub fn get_pending_layers(&self) -> Result<Vec<IntentionLayer>> {
        let stack = self.stack.read();
        Ok(stack
            .layers
            .iter()
            .filter(|l| l.execution_state == ExecutionState::Pending)
            .cloned()
            .collect())
    }

    pub fn get_completed_layers(&self) -> Result<Vec<IntentionLayer>> {
        let stack = self.stack.read();
        Ok(stack
            .layers
            .iter()
            .filter(|l| l.execution_state == ExecutionState::Completed)
            .cloned()
            .collect())
    }

    pub fn detect_drift(&self, layer_index: usize) -> Result<super::types::DriftAssessment> {
        let stack = self.stack.read();
        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let layer = &stack.layers[layer_index];
        let root = stack.find_root_layer();
        let detector = self.drift_detector.read();
        Ok(detector.detect_drift(layer, root))
    }

    pub fn handle_drift(
        &self,
        layer_index: usize,
        action: CorrectiveAction,
    ) -> Result<super::types::DriftHandlingResult> {
        let mut stack = self.stack.write();
        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let detector = self.drift_detector.read();
        let layer = &mut stack.layers[layer_index];
        detector.handle_drift(layer, action)
    }

    pub fn set_drift_threshold(&self, threshold: DriftThreshold) {
        let mut detector = self.drift_detector.write();
        detector.set_drift_threshold(threshold);
    }

    pub fn create_checkpoint(
        &self,
        layer_index: usize,
        label: Option<String>,
    ) -> Result<super::types::Checkpoint> {
        let stack = self.stack.read();
        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let layer = &stack.layers[layer_index];
        let mut cp_manager = self.checkpoint_manager.write();
        cp_manager.create_checkpoint(layer, label)
    }

    pub fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<super::types::RestoreResult> {
        let cp_manager = self.checkpoint_manager.read();
        cp_manager.restore_checkpoint(checkpoint_id)
    }

    pub fn list_checkpoints(&self, layer_index: usize) -> Result<Vec<super::types::Checkpoint>> {
        let stack = self.stack.read();
        if layer_index >= stack.layers.len() {
            return Err(IntentionStackError::InvalidLayerIndex {
                index: layer_index,
                max: stack.layers.len(),
            });
        }

        let layer_id = &stack.layers[layer_index].layer_id;
        let cp_manager = self.checkpoint_manager.read();
        Ok(cp_manager
            .list_checkpoints(layer_id)
            .into_iter()
            .cloned()
            .collect())
    }

    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<bool> {
        let mut cp_manager = self.checkpoint_manager.write();
        cp_manager.delete_checkpoint(checkpoint_id)
    }

    pub fn export_state(&self) -> Result<ExportedIntentionStack> {
        let stack = self.stack.read();
        Ok(stack.export())
    }

    pub fn import_state(&self, exported: ExportedIntentionStack) -> Result<ImportResult> {
        let imported = IntentionStack::import(exported)?;
        let layer_count = imported.layers.len();
        let mut stack = self.stack.write();
        *stack = imported;
        Ok(ImportResult {
            success: true,
            imported_layers: layer_count,
            warnings: Vec::new(),
        })
    }

    fn create_layer_summary(layer: &IntentionLayer) -> IntentionLayerSummary {
        let current_step_desc = layer
            .plan
            .get(layer.current_step_index)
            .map(|s| s.action.action_name.clone());
        IntentionLayerSummary {
            layer_id: layer.layer_id.clone(),
            level: layer.level,
            goal_description: layer.goal.description.clone(),
            execution_state: layer.execution_state,
            progress_percentage: layer.progress_metrics.progress_percentage,
            current_step_description: current_step_desc,
            estimated_completion: layer.goal.deadline,
        }
    }

    fn recompute_levels(stack: &mut IntentionStack) {
        if stack.layers.is_empty() {
            return;
        }

        let mut level_map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();

        for i in 0..stack.layers.len() {
            let new_level = match stack.layers[i].parent_level {
                None => 0,
                Some(p) => {
                    let parent_level_val = *level_map.get(&p).unwrap_or(&0);
                    parent_level_val + 1
                }
            };
            level_map.insert(i, new_level);
            stack.layers[i].level = new_level;
        }
    }

    fn build_reindex_map(
        old_len: usize,
        removed_indices: &[usize],
    ) -> std::collections::HashMap<usize, usize> {
        let mut old_to_new = std::collections::HashMap::new();
        let mut new_idx = 0;
        for old_idx in 0..old_len {
            if removed_indices.contains(&old_idx) {
                continue;
            }
            old_to_new.insert(old_idx, new_idx);
            new_idx += 1;
        }
        old_to_new
    }

    fn apply_reindex(
        stack: &mut IntentionStack,
        old_to_new: &std::collections::HashMap<usize, usize>,
    ) {
        if stack.layers.is_empty() {
            return;
        }

        for layer in &mut stack.layers {
            if let Some(ref p) = layer.parent_level {
                layer.parent_level = old_to_new.get(p).copied();
            }
            layer.child_levels = layer
                .child_levels
                .iter()
                .filter_map(|c| old_to_new.get(c).copied())
                .collect();
        }

        stack.active_goal_pointer = old_to_new
            .get(&stack.active_goal_pointer)
            .copied()
            .unwrap_or(0);

        Self::recompute_levels(stack);
    }

    fn build_execution_context(
        layer: &IntentionLayer,
        req: &GetCurrentIntentionRequest,
    ) -> ExecutionContext {
        let active_step = layer
            .plan
            .get(layer.current_step_index)
            .map(|s| PlanStepSummary {
                step_id: s.step_id.clone(),
                step_index: s.step_index,
                description: s.action.action_name.clone(),
                status: if s.actual_outcome.is_some() {
                    super::types::ExecutionStatus::Succeeded
                } else {
                    super::types::ExecutionStatus::Attempted
                },
            });

        let pending_steps: Vec<PlanStepSummary> = layer
            .plan
            .iter()
            .filter(|s| s.step_index > layer.current_step_index && s.actual_outcome.is_none())
            .map(|s| PlanStepSummary {
                step_id: s.step_id.clone(),
                step_index: s.step_index,
                description: s.action.action_name.clone(),
                status: super::types::ExecutionStatus::Attempted,
            })
            .collect();

        let completed_steps: Vec<PlanStepSummary> = layer
            .plan
            .iter()
            .filter(|s| {
                s.actual_outcome
                    .as_ref()
                    .is_some_and(|o| o.outcome_type == super::types::OutcomeType::Success)
            })
            .map(|s| PlanStepSummary {
                step_id: s.step_id.clone(),
                step_index: s.step_index,
                description: s.action.action_name.clone(),
                status: super::types::ExecutionStatus::Succeeded,
            })
            .collect();

        let failed_steps: Vec<PlanStepSummary> = layer
            .plan
            .iter()
            .filter(|s| s.retry_count >= s.max_retries)
            .map(|s| PlanStepSummary {
                step_id: s.step_id.clone(),
                step_index: s.step_index,
                description: s.action.action_name.clone(),
                status: super::types::ExecutionStatus::Failed,
            })
            .collect();

        let micro_prediction_status = if req.include_micro_predictions {
            layer
                .micro_prediction
                .as_ref()
                .map(|p| super::types::MicroPredictionSummary {
                    prediction_id: p.prediction_id.clone(),
                    prediction_type: p.prediction_type,
                    status: p.status,
                })
        } else {
            None
        };

        let mut blocked_reasons = Vec::new();
        for step in &layer.plan {
            if !step.prerequisite.satisfied {
                blocked_reasons.push(step.prerequisite.description.clone());
            }
        }

        let mut available_actions = Vec::new();
        if layer.execution_state == ExecutionState::Pending {
            available_actions.push(super::types::ActionSuggestion {
                action_type: "start".to_string(),
                description: "Start execution".to_string(),
            });
        }
        if layer.execution_state == ExecutionState::WaitingFeedback {
            available_actions.push(super::types::ActionSuggestion {
                action_type: "check_feedback".to_string(),
                description: "Check for feedback".to_string(),
            });
        }

        ExecutionContext {
            active_step,
            pending_steps,
            blocked_steps: Vec::new(),
            completed_steps,
            failed_steps,
            micro_prediction_status,
            blocked_reasons,
            available_actions,
        }
    }
}
