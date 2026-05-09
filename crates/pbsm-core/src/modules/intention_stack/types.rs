use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::error::{IntentionStackError, Result};
use super::state::{DriftSeverity, ExecutionState, GoalPriority};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectiveAction {
    None,
    Replan,
    Reorder,
    Recontextualize,
    Escalate,
    Abandon,
    Rollback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateType {
    Belief,
    External,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Eq,
    Ne,
    Gt,
    Lt,
    Contains,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    Observation,
    Prediction,
    Inference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionType {
    Belief,
    Action,
    Time,
    Composite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureType {
    BeliefInvalid,
    ActionFailed,
    Timeout,
    Drift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    BeliefQuery,
    BeliefUpdate,
    BeliefCreate,
    BeliefDelete,
    ToolInvocation,
    MemoryOperation,
    ExternalAction,
    ReasoningStep,
    SubgoalCreation,
    CompositeAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrerequisiteType {
    Belief,
    Action,
    State,
    Time,
    Composite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsatisfiedAction {
    Block,
    Skip,
    Retry,
    Compensate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeType {
    Success,
    PartialSuccess,
    Failure,
    Unexpected,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Add,
    Remove,
    Modify,
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateChangeType {
    Transition,
    Increment,
    Decrement,
    Assignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternType {
    Exact,
    Regex,
    Structured,
    Fuzzy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Attempted,
    Succeeded,
    Failed,
    Skipped,
    Compensated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviationType {
    Numeric,
    Semantic,
    Structural,
    Temporal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicroPredictionType {
    StateChange,
    ObservationReturn,
    BeliefUpdate,
    ConditionMet,
    Timeout,
    FeedbackReceived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectType {
    Belief,
    Action,
    Environment,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidityType {
    Time,
    Steps,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionStatus {
    Pending,
    Verified,
    Falsified,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchResult {
    Exact,
    Partial,
    Mismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationSource {
    Direct,
    Inferred,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftTrend {
    Increasing,
    Stable,
    Decreasing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateTransitionTrigger {
    StepComplete,
    StepFailed,
    UserInput,
    DriftDetected,
    Timeout,
    ExternalEvent,
    SystemRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackPushState {
    Pushed,
    Replaced,
    Nested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopReason {
    Completed,
    Abandoned,
    Failed,
    UserRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevertMode {
    Checkpoint,
    StateOnly,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefCondition {
    pub node_id: String,
    pub attribute: String,
    pub operator: Operator,
    pub expected_value: serde_json::Value,
    pub confidence_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCondition {
    pub condition_type: String,
    pub target_resource: String,
    pub expected_status: serde_json::Value,
    pub verification_method: VerificationMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetState {
    pub state_type: StateType,
    pub belief_conditions: Vec<BeliefCondition>,
    pub external_conditions: Vec<ExternalCondition>,
    pub expected_attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessCriterion {
    pub criterion_id: String,
    pub criterion_type: CriterionType,
    pub description: String,
    pub satisfied: bool,
    pub satisfaction_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureCondition {
    pub condition_id: String,
    pub condition_type: FailureType,
    pub description: String,
    pub triggered: bool,
    pub trigger_time: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalDefinition {
    pub goal_id: String,
    pub description: String,
    pub target_state: TargetState,
    pub success_criteria: Vec<SuccessCriterion>,
    pub failure_conditions: Vec<FailureCondition>,
    pub priority: GoalPriority,
    pub deadline: Option<i64>,
    pub origin_belief_ids: Vec<String>,
}

impl GoalDefinition {
    pub fn simple(description: String, priority: GoalPriority) -> Self {
        Self {
            goal_id: Uuid::new_v4().to_string(),
            description,
            target_state: TargetState {
                state_type: StateType::Mixed,
                belief_conditions: Vec::new(),
                external_conditions: Vec::new(),
                expected_attributes: HashMap::new(),
            },
            success_criteria: Vec::new(),
            failure_conditions: Vec::new(),
            priority,
            deadline: None,
            origin_belief_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub action_type: ActionType,
    pub action_name: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub target_node_id: Option<String>,
    pub tool_id: Option<String>,
    pub estimated_duration: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrerequisiteCondition {
    pub condition_id: String,
    pub condition_type: PrerequisiteType,
    pub description: String,
    pub satisfied: bool,
    pub unsatisfied_action: UnsatisfiedAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefChangeSpec {
    pub node_id: String,
    pub attribute: String,
    pub change_type: ChangeType,
    pub expected_value: serde_json::Value,
    pub confidence_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeSpec {
    pub state_variable: String,
    pub previous_value: serde_json::Value,
    pub expected_value: serde_json::Value,
    pub change_type: StateChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationPattern {
    pub pattern_type: PatternType,
    pub pattern: serde_json::Value,
    pub tolerance: f64,
    pub key_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedOutcome {
    pub outcome_type: OutcomeType,
    pub description: String,
    pub belief_changes: Vec<BeliefChangeSpec>,
    pub state_changes: Vec<StateChangeSpec>,
    pub observation_pattern: ObservationPattern,
    pub confidence_expectation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefUpdateResult {
    pub node_id: String,
    pub attribute: String,
    pub previous_value: serde_json::Value,
    pub new_value: serde_json::Value,
    pub update_success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResidual {
    pub residual_id: String,
    pub prediction_id: String,
    pub expected_value: serde_json::Value,
    pub actual_value: serde_json::Value,
    pub deviation_degree: f64,
    pub deviation_type: DeviationType,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualOutcome {
    pub outcome_type: OutcomeType,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub result_data: serde_json::Value,
    pub belief_updates: Vec<BeliefUpdateResult>,
    pub residual: Option<PredictionResidual>,
    pub execution_duration: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub execution_id: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub status: ExecutionStatus,
    pub duration: i64,
    pub error_details: Option<String>,
    pub belief_snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepMetadata {
    pub version: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: DateTime<Utc>,
    pub last_execution_attempt: Option<i64>,
    pub total_execution_time: i64,
    pub average_execution_time: f64,
}

impl Default for StepMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            updated_at: now,
            last_execution_attempt: None,
            total_execution_time: 0,
            average_execution_time: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_id: String,
    pub step_index: usize,
    pub action: ActionDefinition,
    pub prerequisite: PrerequisiteCondition,
    pub expected_outcome: ExpectedOutcome,
    pub actual_outcome: Option<ActualOutcome>,
    pub retry_count: usize,
    pub max_retries: usize,
    pub timeout: i64,
    pub execution_history: Vec<ExecutionRecord>,
    pub associated_belief_ids: Vec<String>,
    pub metadata: StepMetadata,
}

impl PlanStep {
    pub fn simple(action_name: String, step_index: usize) -> Self {
        Self {
            step_id: Uuid::new_v4().to_string(),
            step_index,
            action: ActionDefinition {
                action_type: ActionType::ExternalAction,
                action_name,
                parameters: HashMap::new(),
                target_node_id: None,
                tool_id: None,
                estimated_duration: 0,
            },
            prerequisite: PrerequisiteCondition {
                condition_id: Uuid::new_v4().to_string(),
                condition_type: PrerequisiteType::State,
                description: String::new(),
                satisfied: true,
                unsatisfied_action: UnsatisfiedAction::Block,
            },
            expected_outcome: ExpectedOutcome {
                outcome_type: OutcomeType::Success,
                description: String::new(),
                belief_changes: Vec::new(),
                state_changes: Vec::new(),
                observation_pattern: ObservationPattern {
                    pattern_type: PatternType::Exact,
                    pattern: serde_json::Value::Null,
                    tolerance: 0.0,
                    key_fields: Vec::new(),
                },
                confidence_expectation: 0.8,
            },
            actual_outcome: None,
            retry_count: 0,
            max_retries: 3,
            timeout: 30000,
            execution_history: Vec::new(),
            associated_belief_ids: Vec::new(),
            metadata: StepMetadata::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub step_number: usize,
    pub premise: String,
    pub inference: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueRange {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionContent {
    pub subject_type: SubjectType,
    pub subject_id: String,
    pub property: String,
    pub expected_value: serde_json::Value,
    pub value_range: Option<ValueRange>,
    pub confidence_level: f64,
    pub reasoning_chain: Vec<ReasoningStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidityWindow {
    pub validity_type: ValidityType,
    pub duration: i64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub start_timestamp: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub end_timestamp: DateTime<Utc>,
    pub extension_allowed: bool,
    pub max_extensions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRecord {
    pub verification_id: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub observation: serde_json::Value,
    pub match_result: MatchResult,
    pub residual_degree: f64,
    pub verification_method: VerificationSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionMetadata {
    pub version: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: DateTime<Utc>,
}

impl Default for PredictionMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroPrediction {
    pub prediction_id: String,
    pub parent_intention_id: String,
    pub associated_step_id: String,
    pub prediction_type: MicroPredictionType,
    pub prediction_content: PredictionContent,
    pub validity_window: ValidityWindow,
    pub status: PredictionStatus,
    pub verification_history: Vec<VerificationRecord>,
    pub metadata: PredictionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMetrics {
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub progress_percentage: f64,
    pub estimated_remaining_time: i64,
    pub actual_elapsed_time: i64,
    pub efficiency_score: f64,
}

impl Default for ProgressMetrics {
    fn default() -> Self {
        Self {
            total_steps: 0,
            completed_steps: 0,
            failed_steps: 0,
            progress_percentage: 0.0,
            estimated_remaining_time: 0,
            actual_elapsed_time: 0,
            efficiency_score: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerMetadata {
    pub version: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: DateTime<Utc>,
    pub created_from_belief_id: Option<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_state_change: DateTime<Utc>,
    pub state_change_count: u64,
    pub checkpoint_id: Option<String>,
}

impl Default for LayerMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            updated_at: now,
            created_from_belief_id: None,
            last_state_change: now,
            state_change_count: 0,
            checkpoint_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDriftStatus {
    pub is_drifting: bool,
    pub drift_angle: f64,
    pub drift_direction: Option<String>,
    pub drift_start_time: Option<i64>,
    pub root_cause_hypothesis: Option<String>,
}

impl Default for LayerDriftStatus {
    fn default() -> Self {
        Self {
            is_drifting: false,
            drift_angle: 0.0,
            drift_direction: None,
            drift_start_time: None,
            root_cause_hypothesis: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionLayer {
    pub layer_id: String,
    pub level: usize,
    pub goal: GoalDefinition,
    pub plan: Vec<PlanStep>,
    pub execution_state: ExecutionState,
    pub current_step_index: usize,
    pub micro_prediction: Option<MicroPrediction>,
    pub parent_level: Option<usize>,
    pub child_levels: Vec<usize>,
    pub progress_metrics: ProgressMetrics,
    pub metadata: LayerMetadata,
    pub drift_status: LayerDriftStatus,
}

impl IntentionLayer {
    pub fn new(goal: GoalDefinition, level: usize, parent_level: Option<usize>) -> Self {
        Self {
            layer_id: Uuid::new_v4().to_string(),
            level,
            goal,
            plan: Vec::new(),
            execution_state: ExecutionState::Pending,
            current_step_index: 0,
            micro_prediction: None,
            parent_level,
            child_levels: Vec::new(),
            progress_metrics: ProgressMetrics::default(),
            metadata: LayerMetadata::default(),
            drift_status: LayerDriftStatus::default(),
        }
    }

    pub fn effective_depth(&self) -> usize {
        self.level
    }

    pub fn can_add_child(&self) -> bool {
        let max_children = match self.level {
            0 => 10,
            1..=3 => 5,
            4..=7 => 3,
            8..=15 => 2,
            _ => 1,
        };
        self.child_levels.len() < max_children
    }

    pub fn update_progress(&mut self) {
        if self.plan.is_empty() {
            self.progress_metrics.total_steps = 0;
            self.progress_metrics.completed_steps = 0;
            self.progress_metrics.failed_steps = 0;
            self.progress_metrics.progress_percentage = 0.0;
            return;
        }
        self.progress_metrics.total_steps = self.plan.len();
        let completed = self
            .plan
            .iter()
            .filter(|s| {
                s.actual_outcome
                    .as_ref()
                    .is_some_and(|o| o.outcome_type == OutcomeType::Success)
            })
            .count();
        let failed = self
            .plan
            .iter()
            .filter(|s| s.retry_count >= s.max_retries)
            .count();
        self.progress_metrics.completed_steps = completed;
        self.progress_metrics.failed_steps = failed;
        self.progress_metrics.progress_percentage =
            (completed as f64 / self.plan.len() as f64) * 100.0;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftRecord {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub drift_angle: f64,
    pub trigger_event: String,
    pub corrective_action: CorrectiveAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftComponents {
    pub directional_drift: f64,
    pub scope_drift: f64,
    pub priority_drift: f64,
    pub method_drift: f64,
}

impl Default for DriftComponents {
    fn default() -> Self {
        Self {
            directional_drift: 0.0,
            scope_drift: 0.0,
            priority_drift: 0.0,
            method_drift: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftThreshold {
    pub warning: f64,
    pub moderate: f64,
    pub severe: f64,
    pub critical: f64,
}

impl Default for DriftThreshold {
    fn default() -> Self {
        Self {
            warning: 0.3,
            moderate: 0.5,
            severe: 0.7,
            critical: 0.9,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftMetrics {
    pub current_drift_angle: f64,
    pub drift_components: DriftComponents,
    pub overall_drift_score: f64,
    pub drift_threshold: DriftThreshold,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_assessment_time: DateTime<Utc>,
    pub trend: DriftTrend,
    pub drift_history: Vec<DriftRecord>,
}

impl Default for DriftMetrics {
    fn default() -> Self {
        Self {
            current_drift_angle: 0.0,
            drift_components: DriftComponents::default(),
            overall_drift_score: 0.0,
            drift_threshold: DriftThreshold::default(),
            last_assessment_time: Utc::now(),
            trend: DriftTrend::Stable,
            drift_history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackMetadata {
    pub version: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: DateTime<Utc>,
    pub root_intention_id: Option<String>,
    pub completed_count: usize,
    pub abandoned_count: usize,
    pub total_execution_time: i64,
}

impl Default for StackMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            updated_at: now,
            root_intention_id: None,
            completed_count: 0,
            abandoned_count: 0,
            total_execution_time: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionStack {
    pub stack_id: String,
    pub stack_name: String,
    pub layers: Vec<IntentionLayer>,
    pub active_goal_pointer: usize,
    pub max_depth: usize,
    pub metadata: StackMetadata,
    pub drift_metrics: DriftMetrics,
}

impl IntentionStack {
    pub const MAX_STACK_DEPTH: usize = 20;
    pub const MAX_STACK_CAPACITY: usize = 500;

    pub fn new(stack_name: String) -> Self {
        Self {
            stack_id: Uuid::new_v4().to_string(),
            stack_name,
            layers: Vec::new(),
            active_goal_pointer: 0,
            max_depth: Self::MAX_STACK_DEPTH,
            metadata: StackMetadata::default(),
            drift_metrics: DriftMetrics::default(),
        }
    }

    pub fn depth(&self) -> usize {
        self.layers.len()
    }

    pub fn current_layer(&self) -> Option<&IntentionLayer> {
        self.layers.get(self.active_goal_pointer)
    }

    pub fn current_layer_mut(&mut self) -> Option<&mut IntentionLayer> {
        self.layers.get_mut(self.active_goal_pointer)
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn validate_depth_limit(&self, parent_level: Option<usize>) -> DepthValidationResult {
        let requested_depth = match parent_level {
            Some(level) => {
                if level >= self.layers.len() {
                    return DepthValidationResult {
                        allowed: false,
                        actual_depth: None,
                        reason: Some("Invalid parent level".to_string()),
                    };
                }
                self.layers[level].level + 1
            }
            None => 0,
        };

        if requested_depth > Self::MAX_STACK_DEPTH {
            return DepthValidationResult {
                allowed: false,
                actual_depth: None,
                reason: Some(format!(
                    "Exceeds hard limit: {} > {}",
                    requested_depth,
                    Self::MAX_STACK_DEPTH
                )),
            };
        }

        let max_children = match requested_depth {
            0 => 10,
            1..=3 => 5,
            4..=7 => 3,
            8..=15 => 2,
            _ => 1,
        };

        if let Some(parent_level) = parent_level {
            if parent_level < self.layers.len() {
                let parent = &self.layers[parent_level];
                if parent.child_levels.len() >= max_children {
                    return DepthValidationResult {
                        allowed: false,
                        actual_depth: Some(requested_depth),
                        reason: Some(format!(
                            "Child limit exceeded: {} >= {} at level {}",
                            parent.child_levels.len(),
                            max_children,
                            requested_depth
                        )),
                    };
                }
            }
        }

        DepthValidationResult {
            allowed: true,
            actual_depth: Some(requested_depth),
            reason: None,
        }
    }

    pub fn find_root_layer(&self) -> Option<&IntentionLayer> {
        self.layers.iter().find(|l| l.parent_level.is_none())
    }
}

#[derive(Debug, Clone)]
pub struct DepthValidationResult {
    pub allowed: bool,
    pub actual_depth: Option<usize>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warning {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushIntentRequest {
    pub goal: GoalDefinition,
    pub plan: Option<Vec<PlanStep>>,
    pub parent_level: Option<usize>,
    pub micro_prediction: Option<MicroPrediction>,
    pub attach_to_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushIntentResponse {
    pub success: bool,
    pub layer_index: usize,
    pub layer_id: String,
    pub stack_state: StackPushState,
    pub parent_layer_id: Option<String>,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionReport {
    pub outcome: OutcomeType,
    pub achieved_goals: Vec<String>,
    pub unmet_goals: Vec<String>,
    pub belief_snapshots: Vec<String>,
    pub lessons_learned: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedLayerInfo {
    pub layer_id: String,
    pub level: usize,
    pub final_state: ExecutionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildPromotionResult {
    pub promoted_layer_id: String,
    pub original_parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefUpdate {
    pub belief_id: String,
    pub update_type: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopIntentRequest {
    pub layer_index: usize,
    pub reason: PopReason,
    pub final_state: Option<ExecutionState>,
    pub completion_report: Option<CompletionReport>,
    pub cascade: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopIntentResponse {
    pub success: bool,
    pub removed_layers: Vec<RemovedLayerInfo>,
    pub promoted_child: Option<ChildPromotionResult>,
    pub belief_updates: Vec<BeliefUpdate>,
    pub parent_updated: bool,
    pub next_intention_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransitionContext {
    pub trigger: StateTransitionTrigger,
    pub details: HashMap<String, serde_json::Value>,
    pub evidence: Vec<String>,
    pub next_step_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffect {
    pub effect_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildStateUpdate {
    pub child_level: usize,
    pub previous_state: ExecutionState,
    pub new_state: ExecutionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIntentStateRequest {
    pub layer_index: usize,
    pub new_state: ExecutionState,
    pub transition_context: Option<StateTransitionContext>,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIntentStateResponse {
    pub success: bool,
    pub previous_state: ExecutionState,
    pub current_state: ExecutionState,
    pub state_change_allowed: bool,
    pub blocked_reasons: Vec<String>,
    pub side_effects: Vec<SideEffect>,
    pub child_state_updates: Vec<ChildStateUpdate>,
    pub event_emitted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionLayerSummary {
    pub layer_id: String,
    pub level: usize,
    pub goal_description: String,
    pub execution_state: ExecutionState,
    pub progress_percentage: f64,
    pub current_step_description: Option<String>,
    pub estimated_completion: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStepSummary {
    pub step_id: String,
    pub step_index: usize,
    pub description: String,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroPredictionSummary {
    pub prediction_id: String,
    pub prediction_type: MicroPredictionType,
    pub status: PredictionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSuggestion {
    pub action_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub active_step: Option<PlanStepSummary>,
    pub pending_steps: Vec<PlanStepSummary>,
    pub blocked_steps: Vec<PlanStepSummary>,
    pub completed_steps: Vec<PlanStepSummary>,
    pub failed_steps: Vec<PlanStepSummary>,
    pub micro_prediction_status: Option<MicroPredictionSummary>,
    pub blocked_reasons: Vec<String>,
    pub available_actions: Vec<ActionSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breadcrumb {
    pub level: usize,
    pub layer_id: String,
    pub goal_description: String,
    pub execution_state: ExecutionState,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalSummary {
    pub goal_id: String,
    pub description: String,
    pub priority: GoalPriority,
    pub overall_progress: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub recommendation_type: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetCurrentIntentionRequest {
    pub include_subtree: bool,
    pub include_history: bool,
    pub include_micro_predictions: bool,
    pub depth_limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCurrentIntentionResponse {
    pub current_layer: IntentionLayerSummary,
    pub ancestors: Vec<IntentionLayerSummary>,
    pub children: Vec<IntentionLayerSummary>,
    pub execution_context: ExecutionContext,
    pub drift_status: LayerDriftStatus,
    pub top_level_goal: Option<GoalSummary>,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub recommendations: Vec<Recommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolledBackLayerInfo {
    pub layer_id: String,
    pub previous_state: ExecutionState,
    pub new_state: ExecutionState,
    pub steps_reverted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefRestoration {
    pub belief_id: String,
    pub restored_snapshot_id: String,
    pub restoration_success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertToIntentionRequest {
    pub target_layer_index: usize,
    pub revert_mode: RevertMode,
    pub checkpoint_id: Option<String>,
    pub reason: Option<String>,
    pub preserve_completed_steps: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertResult {
    pub success: bool,
    pub revert_depth: usize,
    pub rolled_back_layers: Vec<RolledBackLayerInfo>,
    pub restored_checkpoint: Option<String>,
    pub belief_restorations: Vec<BeliefRestoration>,
    pub invalidated_predictions: Vec<String>,
    pub new_current_layer_index: usize,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftAssessment {
    pub layer_id: String,
    pub severity: DriftSeverity,
    pub overall_drift_score: f64,
    pub components: DriftComponents,
    pub trend: DriftTrend,
    pub root_cause_hypothesis: Option<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub recommended_action: CorrectiveAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftHandlingResult {
    pub success: bool,
    pub action_taken: CorrectiveAction,
    pub new_drift_status: LayerDriftStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub checkpoint_id: String,
    pub layer_id: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    pub label: Option<String>,
    pub state_snapshot: IntentionLayer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub success: bool,
    pub restored_checkpoint_id: String,
    pub layers_restored: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepAdvanceResult {
    pub success: bool,
    pub new_step_index: usize,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddStepResponse {
    pub success: bool,
    pub step_id: String,
    pub step_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveStepResponse {
    pub success: bool,
    pub removed_step_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderResponse {
    pub success: bool,
    pub new_order: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedIntentionStack {
    pub version: u32,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub exported_at: DateTime<Utc>,
    pub stack: IntentionStack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub success: bool,
    pub imported_layers: usize,
    pub warnings: Vec<String>,
}

impl IntentionStack {
    pub fn export(&self) -> ExportedIntentionStack {
        ExportedIntentionStack {
            version: 1,
            exported_at: Utc::now(),
            stack: self.clone(),
        }
    }

    pub fn import(exported: ExportedIntentionStack) -> Result<Self> {
        if exported.version != 1 {
            return Err(IntentionStackError::Internal(format!(
                "Unsupported export version: {}",
                exported.version
            )));
        }
        Ok(exported.stack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intention_stack_new() {
        let stack = IntentionStack::new("test_stack".to_string());
        assert_eq!(stack.stack_name, "test_stack");
        assert!(stack.is_empty());
        assert_eq!(stack.depth(), 0);
        assert_eq!(stack.max_depth, IntentionStack::MAX_STACK_DEPTH);
    }

    #[test]
    fn test_intention_layer_new() {
        let goal = GoalDefinition::simple("Test goal".to_string(), GoalPriority::High);
        let layer = IntentionLayer::new(goal, 0, None);
        assert_eq!(layer.level, 0);
        assert_eq!(layer.execution_state, ExecutionState::Pending);
        assert!(layer.parent_level.is_none());
        assert!(layer.child_levels.is_empty());
    }

    #[test]
    fn test_can_add_child() {
        let goal = GoalDefinition::simple("Root".to_string(), GoalPriority::Critical);
        let layer = IntentionLayer::new(goal, 0, None);
        assert!(layer.can_add_child());

        let goal2 = GoalDefinition::simple("Deep".to_string(), GoalPriority::Low);
        let mut deep_layer = IntentionLayer::new(goal2, 18, Some(0));
        deep_layer.child_levels = vec![1, 2];
        assert!(!deep_layer.can_add_child());
    }

    #[test]
    fn test_validate_depth_limit_root() {
        let stack = IntentionStack::new("test".to_string());
        let result = stack.validate_depth_limit(None);
        assert!(result.allowed);
        assert_eq!(result.actual_depth, Some(0));
    }

    #[test]
    fn test_validate_depth_limit_exceeds_max() {
        let mut stack = IntentionStack::new("test".to_string());
        let goal = GoalDefinition::simple("Root".to_string(), GoalPriority::Critical);
        let mut layer = IntentionLayer::new(goal, 20, None);
        layer.level = 20;
        stack.layers.push(layer);
        let result = stack.validate_depth_limit(Some(0));
        assert!(!result.allowed);
    }

    #[test]
    fn test_goal_definition_simple() {
        let goal = GoalDefinition::simple("Test".to_string(), GoalPriority::Medium);
        assert_eq!(goal.description, "Test");
        assert_eq!(goal.priority, GoalPriority::Medium);
        assert!(goal.success_criteria.is_empty());
        assert!(goal.failure_conditions.is_empty());
    }

    #[test]
    fn test_plan_step_simple() {
        let step = PlanStep::simple("test_action".to_string(), 0);
        assert_eq!(step.step_index, 0);
        assert_eq!(step.action.action_name, "test_action");
        assert_eq!(step.retry_count, 0);
    }

    #[test]
    fn test_drift_threshold_default() {
        let threshold = DriftThreshold::default();
        assert!((threshold.warning - 0.3).abs() < f64::EPSILON);
        assert!((threshold.moderate - 0.5).abs() < f64::EPSILON);
        assert!((threshold.severe - 0.7).abs() < f64::EPSILON);
        assert!((threshold.critical - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_export_import() {
        let stack = IntentionStack::new("test".to_string());
        let exported = stack.export();
        assert_eq!(exported.version, 1);

        let imported = IntentionStack::import(exported).unwrap();
        assert_eq!(imported.stack_name, "test");
    }

    #[test]
    fn test_update_progress() {
        let goal = GoalDefinition::simple("Test".to_string(), GoalPriority::Medium);
        let mut layer = IntentionLayer::new(goal, 0, None);
        layer.plan = vec![
            PlanStep::simple("step1".to_string(), 0),
            PlanStep::simple("step2".to_string(), 1),
        ];
        layer.update_progress();
        assert_eq!(layer.progress_metrics.total_steps, 2);
        assert_eq!(layer.progress_metrics.completed_steps, 0);
        assert!((layer.progress_metrics.progress_percentage - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let goal = GoalDefinition::simple("Test".to_string(), GoalPriority::High);
        let layer = IntentionLayer::new(goal, 0, None);
        let json = serde_json::to_string(&layer).unwrap();
        let deserialized: IntentionLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.layer_id, layer.layer_id);
        assert_eq!(deserialized.level, layer.level);
    }
}
