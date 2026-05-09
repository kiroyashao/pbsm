use serde::{Deserialize, Serialize};

use super::state::{DriftSeverity, ExecutionState, GoalPriority};
use super::types::{CorrectiveAction, DriftComponents, DriftTrend};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntentionStackEvent {
    IntentionPushed(IntentionPushedPayload),
    IntentionPopped(IntentionPoppedPayload),
    StateChanged(StateChangedPayload),
    DriftDetected(DriftDetectedPayload),
    StepCompleted(StepCompletedPayload),
    CheckpointCreated(CheckpointCreatedPayload),
    CheckpointRestored(CheckpointRestoredPayload),
    IntentionReverted(IntentionRevertedPayload),
}

impl IntentionStackEvent {
    pub fn event_type_name(&self) -> &'static str {
        match self {
            Self::IntentionPushed(_) => "intention.pushed",
            Self::IntentionPopped(_) => "intention.popped",
            Self::StateChanged(_) => "intention.stateChanged",
            Self::DriftDetected(_) => "intention.driftDetected",
            Self::StepCompleted(_) => "intention.stepCompleted",
            Self::CheckpointCreated(_) => "intention.checkpointCreated",
            Self::CheckpointRestored(_) => "intention.checkpointRestored",
            Self::IntentionReverted(_) => "intention.reverted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionPushedPayload {
    pub stack_id: String,
    pub layer_id: String,
    pub layer_index: usize,
    pub level: usize,
    pub parent_layer_id: Option<String>,
    pub parent_level: Option<usize>,
    pub goal_description: String,
    pub priority: GoalPriority,
    pub plan_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionPoppedPayload {
    pub layer_id: String,
    pub layer_index: usize,
    pub reason: super::types::PopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangedPayload {
    pub layer_id: String,
    pub layer_index: usize,
    pub previous_state: ExecutionState,
    pub current_state: ExecutionState,
    pub trigger: String,
    pub child_updates: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectedPayload {
    pub layer_id: String,
    pub layer_index: usize,
    pub severity: DriftSeverity,
    pub overall_score: f64,
    pub components: DriftComponents,
    pub trend: DriftTrend,
    pub root_cause_hypothesis: String,
    pub recommended_action: CorrectiveAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepCompletedPayload {
    pub layer_id: String,
    pub layer_index: usize,
    pub step_id: String,
    pub step_index: usize,
    pub outcome: super::types::OutcomeType,
    pub duration: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointCreatedPayload {
    pub checkpoint_id: String,
    pub layer_id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRestoredPayload {
    pub layer_id: String,
    pub layer_index: usize,
    pub checkpoint_id: String,
    pub reverted_layers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionRevertedPayload {
    pub target_layer_index: usize,
    pub rolled_back_count: usize,
    pub new_current_index: usize,
}

pub trait IntentionStackEventPublisher: Send + Sync {
    fn publish(&self, event: IntentionStackEvent) -> Result<(), String>;
}

pub struct NullIntentionStackEventPublisher;

impl IntentionStackEventPublisher for NullIntentionStackEventPublisher {
    fn publish(&self, _event: IntentionStackEvent) -> Result<(), String> {
        Ok(())
    }
}
