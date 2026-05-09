use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::error::MetacognitiveError;
use super::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttentionMode {
    LowVigilance,
    ModerateFocus,
    HighReconnaissance,
}

impl AttentionMode {
    pub fn from_alpha(alpha: f64) -> Self {
        if alpha <= 0.3 {
            AttentionMode::LowVigilance
        } else if alpha <= 0.7 {
            AttentionMode::ModerateFocus
        } else {
            AttentionMode::HighReconnaissance
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AttentionMode::LowVigilance => "LOW_VIGILANCE",
            AttentionMode::ModerateFocus => "MODERATE_FOCUS",
            AttentionMode::HighReconnaissance => "HIGH_RECONNAISSANCE",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AttentionMode::LowVigilance => "系统处于放松状态，对后续观测进行低分辨率处理",
            AttentionMode::ModerateFocus => "系统保持适度的警觉，对新信息进行常规评估",
            AttentionMode::HighReconnaissance => {
                "系统进入高度聚焦状态，对预测残差区域进行结构化扫描"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AttentionBounds {
    pub min: f64,
    pub max: f64,
}

impl Default for AttentionBounds {
    fn default() -> Self {
        Self { min: 0.1, max: 1.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdjustmentTrigger {
    PredictionVerified,
    PredictionDeviation,
    TimeDecay,
    UserOverride,
    AnomalyCorrection,
    IntentionChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustmentRecord {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub previous_value: f64,
    pub new_value: f64,
    pub trigger: AdjustmentTrigger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionState {
    pub parameter: f64,
    pub mode: AttentionMode,
    pub bounds: AttentionBounds,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_adjustment_timestamp: DateTime<Utc>,
}

impl Default for AttentionState {
    fn default() -> Self {
        Self {
            parameter: 0.5,
            mode: AttentionMode::ModerateFocus,
            bounds: AttentionBounds::default(),
            last_adjustment_timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightConfiguration {
    pub goal_relevance_weight: f64,
    pub access_frequency_weight: f64,
    pub recency_weight: f64,
    pub residual_weight: f64,
}

impl Default for WeightConfiguration {
    fn default() -> Self {
        Self {
            goal_relevance_weight: 0.35,
            access_frequency_weight: 0.25,
            recency_weight: 0.20,
            residual_weight: 0.20,
        }
    }
}

impl WeightConfiguration {
    pub fn validate(&self) -> Result<()> {
        let sum = self.goal_relevance_weight
            + self.access_frequency_weight
            + self.recency_weight
            + self.residual_weight;

        if (sum - 1.0).abs() > f64::EPSILON {
            return Err(MetacognitiveError::WeightValidationFailed {
                reason: format!("Weight sum must be 1.0, got {}", sum),
            });
        }

        for (name, weight) in [
            ("goal_relevance_weight", self.goal_relevance_weight),
            ("access_frequency_weight", self.access_frequency_weight),
            ("recency_weight", self.recency_weight),
            ("residual_weight", self.residual_weight),
        ] {
            if !(0.0..=1.0).contains(&weight) {
                return Err(MetacognitiveError::WeightValidationFailed {
                    reason: format!("{} must be in range [0, 1], got {}", name, weight),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueFactors {
    pub goal_relevance: f64,
    pub access_frequency: f64,
    pub recency: f64,
    pub residual_association: f64,
}

impl Default for ValueFactors {
    fn default() -> Self {
        Self {
            goal_relevance: 0.5,
            access_frequency: 0.3,
            recency: 0.7,
            residual_association: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueEvaluation {
    pub node_id: String,
    pub score: f64,
    pub factors: ValueFactors,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_calculated: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValueTrend {
    Rising,
    Falling,
    Stable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusItem {
    pub residual_id: String,
    pub priority: f64,
    pub relevance_to_goal: f64,
    pub required_inference_depth: u32,
    pub affected_belief_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusSummary {
    pub focus_count: usize,
    pub top_foci: Vec<FocusItem>,
    pub overall_focus_level: f64,
}

impl Default for FocusSummary {
    fn default() -> Self {
        Self {
            focus_count: 0,
            top_foci: Vec::new(),
            overall_focus_level: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgetCandidate {
    pub node_id: String,
    pub value_score: f64,
    pub reason: String,
    pub is_protected: bool,
    pub is_deferred: bool,
    pub defer_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgetResult {
    pub archived_ids: Vec<String>,
    pub protected_ids: Vec<String>,
    pub deferred_ids: Vec<String>,
    pub archived_count: usize,
    pub protected_count: usize,
    pub deferred_count: usize,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ForgetReason {
    LowValue,
    ContextOverflow,
    UserRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnomalyType {
    ExcessiveFocus,
    Oscillation,
    Drift,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnomalySeverity {
    None,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub evidence: serde_json::Value,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub detected_at: DateTime<Utc>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyReport {
    pub has_anomalies: bool,
    pub severity: AnomalySeverity,
    pub anomalies: Vec<Anomaly>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_check_timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAttentionStatusResponse {
    pub attention_parameter: f64,
    pub current_mode: String,
    pub mode_description: String,
    pub focus_summary: FocusSummary,
    pub adjustment_history: Vec<AdjustmentRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAttentionBoundsRequest {
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAttentionBoundsResponse {
    pub previous_bounds: AttentionBounds,
    pub new_bounds: AttentionBounds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustAttentionRequest {
    pub delta: Option<f64>,
    pub target_value: Option<f64>,
    pub trigger: AdjustmentTrigger,
    pub override_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustAttentionResponse {
    pub previous_value: f64,
    pub new_value: f64,
    pub adjustment_applied: f64,
    pub trigger: AdjustmentTrigger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateMemoryValueRequest {
    pub node_ids: Option<Vec<String>>,
    pub all_active: Option<bool>,
    pub include_factors: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryValueResult {
    pub node_id: String,
    pub total_score: f64,
    pub factors: Option<ValueFactors>,
    pub forget_recommendation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueStatistics {
    pub mean_score: f64,
    pub median_score: f64,
    pub below_threshold_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateMemoryValueResponse {
    pub value_scores: Vec<MemoryValueResult>,
    pub statistics: ValueStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateValueWeightsRequest {
    pub weights: WeightConfiguration,
    pub persist: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateValueWeightsResponse {
    pub previous_weights: WeightConfiguration,
    pub new_weights: WeightConfiguration,
    pub validation_result: ValidationResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceForgetRequest {
    pub node_ids: Vec<String>,
    pub force_flag: Option<bool>,
    pub reason: ForgetReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveResult {
    pub node_id: String,
    pub success: bool,
    pub archive_location: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceForgetResponse {
    pub forgotten_ids: Vec<String>,
    pub protected_ids: Vec<String>,
    pub deferred_ids: Vec<String>,
    pub archive_results: Vec<ArchiveResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingForget {
    pub node_id: String,
    pub reason: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub queued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeferredForget {
    pub node_id: String,
    pub defer_reason: String,
    pub defer_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentForget {
    pub node_id: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub archived_at: DateTime<Utc>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgetStatistics {
    pub total_forgotten_this_session: usize,
    pub total_forgotten_all_time: usize,
    pub average_value_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetForgetStatusResponse {
    pub pending_forgets: Vec<PendingForget>,
    pub deferred_forgets: Vec<DeferredForget>,
    pub recent_forgets: Vec<RecentForget>,
    pub statistics: ForgetStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAnomalyReportRequest {
    pub include_details: Option<bool>,
    pub window_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAnomalyReportResponse {
    pub has_anomalies: bool,
    pub severity: AnomalySeverity,
    pub anomalies: Vec<Anomaly>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_check_timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInterventionRequest {
    pub anomaly_type: Option<AnomalyType>,
    pub force_level: Option<AnomalySeverity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intervention {
    pub anomaly_type: AnomalyType,
    pub action: String,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInterventionResponse {
    pub interventions: Vec<Intervention>,
    pub parameter_reset: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    pub archive_id: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub archived_at: DateTime<Utc>,
    pub forget_reason: ForgetReason,
    pub original_node_id: String,
    pub version_at_archive: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefSummaryArchive {
    pub node_type: String,
    pub name: String,
    pub key_attributes: serde_json::Value,
    pub final_confidence: f64,
    pub summary_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub original_source: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_modified: DateTime<Utc>,
    pub update_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextLinks {
    pub related_beliefs: Vec<String>,
    pub related_residuals: Vec<String>,
    pub related_intentions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutcomeType {
    Successful,
    Failed,
    Mixed,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeTag {
    pub outcome_type: OutcomeType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivePackage {
    pub metadata: ArchiveMetadata,
    pub summary: BeliefSummaryArchive,
    pub provenance: Provenance,
    pub context_links: ContextLinks,
    pub outcome_tag: OutcomeTag,
}
