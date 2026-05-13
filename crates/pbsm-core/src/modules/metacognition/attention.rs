use chrono::Utc;
use parking_lot::RwLock;
use std::sync::Arc;

use super::config::AttentionConfig;
use super::error::{MetacognitiveError, Result};
use super::events::{MetacognitiveEvent, MetacognitiveEventPublisher};
use super::types::{
    AdjustAttentionRequest, AdjustAttentionResponse, AdjustmentRecord, AdjustmentTrigger,
    AttentionBounds, AttentionMode, AttentionState, FocusItem, FocusSummary,
    GetAttentionStatusResponse, SetAttentionBoundsRequest, SetAttentionBoundsResponse,
};

pub struct AttentionController {
    state: RwLock<AttentionState>,
    config: AttentionConfig,
    adjustment_history: RwLock<Vec<AdjustmentRecord>>,
    event_publisher: Arc<dyn MetacognitiveEventPublisher>,
}

impl AttentionController {
    pub fn new(
        config: AttentionConfig,
        event_publisher: Arc<dyn MetacognitiveEventPublisher>,
    ) -> Self {
        let mode = AttentionMode::from_alpha(config.default_attention);
        let state = AttentionState {
            parameter: config.default_attention,
            mode,
            bounds: AttentionBounds {
                min: config.min_attention,
                max: config.max_attention,
            },
            last_adjustment_timestamp: Utc::now(),
        };
        Self {
            state: RwLock::new(state),
            config,
            adjustment_history: RwLock::new(Vec::new()),
            event_publisher,
        }
    }

    pub fn get_attention_status(&self) -> GetAttentionStatusResponse {
        let state = self.state.read();
        let history = self.adjustment_history.read();
        GetAttentionStatusResponse {
            attention_parameter: state.parameter,
            current_mode: state.mode.as_str().to_string(),
            mode_description: state.mode.description().to_string(),
            focus_summary: FocusSummary::default(),
            adjustment_history: history.clone(),
        }
    }

    pub async fn adjust_attention(
        &self,
        request: AdjustAttentionRequest,
    ) -> Result<AdjustAttentionResponse> {
        let mut state = self.state.write();
        let previous_value = state.parameter;

        let new_value = if let Some(target) = request.target_value {
            if request.trigger == AdjustmentTrigger::UserOverride
                || request.override_mode.unwrap_or(false)
            {
                self.validate_and_clamp(target, &state.bounds)?
            } else {
                let delta = target - previous_value;
                let clamped_delta =
                    delta.clamp(-self.config.max_adjustment, self.config.max_adjustment);
                self.validate_and_clamp(previous_value + clamped_delta, &state.bounds)?
            }
        } else if let Some(delta) = request.delta {
            let clamped_delta =
                delta.clamp(-self.config.max_adjustment, self.config.max_adjustment);
            self.validate_and_clamp(previous_value + clamped_delta, &state.bounds)?
        } else {
            let delta = match request.trigger {
                AdjustmentTrigger::PredictionVerified => -self.config.decay_rate,
                AdjustmentTrigger::PredictionDeviation => self.config.boost_step,
                AdjustmentTrigger::TimeDecay => -self.config.time_decay_rate,
                AdjustmentTrigger::UserOverride => {
                    return Err(MetacognitiveError::InvalidParameter {
                        field: "Either delta or target_value must be provided for UserOverride"
                            .to_string(),
                    });
                }
                AdjustmentTrigger::AnomalyCorrection => -0.2,
                AdjustmentTrigger::IntentionChange => 0.1,
            };
            let clamped_delta =
                delta.clamp(-self.config.max_adjustment, self.config.max_adjustment);
            self.validate_and_clamp(previous_value + clamped_delta, &state.bounds)?
        };

        let adjustment_applied = new_value - previous_value;
        let new_mode = AttentionMode::from_alpha(new_value);

        let record = AdjustmentRecord {
            timestamp: Utc::now(),
            previous_value,
            new_value,
            trigger: request.trigger,
        };

        let old_mode = state.mode;
        state.parameter = new_value;
        state.mode = new_mode;
        state.last_adjustment_timestamp = Utc::now();

        drop(state);

        self.adjustment_history.write().push(record);

        if old_mode != new_mode {
            let _ = self
                .event_publisher
                .publish(MetacognitiveEvent::AttentionModeChanged {
                    previous_mode: old_mode,
                    new_mode,
                    reason: format!("{:?}", request.trigger),
                });
        }

        let _ = self
            .event_publisher
            .publish(MetacognitiveEvent::AttentionAdjusted {
                previous_value,
                new_value,
                trigger: request.trigger,
                mode: new_mode,
                adjustment_magnitude: adjustment_applied.abs(),
            });

        Ok(AdjustAttentionResponse {
            previous_value,
            new_value,
            adjustment_applied,
            trigger: request.trigger,
        })
    }

    pub fn set_attention_bounds(
        &self,
        request: SetAttentionBoundsRequest,
    ) -> Result<SetAttentionBoundsResponse> {
        let mut state = self.state.write();
        let previous_bounds = state.bounds;

        let new_min = request.min_value.unwrap_or(state.bounds.min);
        let new_max = request.max_value.unwrap_or(state.bounds.max);

        if new_min >= new_max {
            return Err(MetacognitiveError::InvalidParameter {
                field: "min_value must be less than max_value".to_string(),
            });
        }
        if !(0.0..=0.5).contains(&new_min) {
            return Err(MetacognitiveError::AttentionOutOfBounds {
                value: new_min,
                min: 0.0,
                max: 0.5,
            });
        }
        if !(0.5..=1.0).contains(&new_max) {
            return Err(MetacognitiveError::AttentionOutOfBounds {
                value: new_max,
                min: 0.5,
                max: 1.0,
            });
        }

        let new_bounds = AttentionBounds {
            min: new_min,
            max: new_max,
        };

        state.bounds = new_bounds;
        if state.parameter < new_bounds.min {
            state.parameter = new_bounds.min;
            state.mode = AttentionMode::from_alpha(new_bounds.min);
        } else if state.parameter > new_bounds.max {
            state.parameter = new_bounds.max;
            state.mode = AttentionMode::from_alpha(new_bounds.max);
        }

        Ok(SetAttentionBoundsResponse {
            previous_bounds,
            new_bounds,
        })
    }

    pub fn identify_focus(
        &self,
        unresolved_residuals: &[ResidualInfo],
        top_goal_id: Option<&str>,
    ) -> FocusSummary {
        if unresolved_residuals.is_empty() {
            return FocusSummary::default();
        }

        let mut prioritized: Vec<&ResidualInfo> = unresolved_residuals.iter().collect();
        prioritized.sort_by(|a, b| {
            let pa = a.severity * a.affected_node_count as f64;
            let pb = b.severity * b.affected_node_count as f64;
            pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
        });

        let n = prioritized.len().min(5);
        let top_residuals = &prioritized[..n];

        let foci: Vec<FocusItem> = top_residuals
            .iter()
            .map(|r| {
                let relevance = if let Some(goal_id) = top_goal_id {
                    let goal_factor = (goal_id.len() as f64 % 5.0 + 1.0) / 5.0;
                    goal_factor / (1.0 + r.graph_distance as f64)
                } else {
                    0.5
                };
                FocusItem {
                    residual_id: r.id.clone(),
                    priority: r.severity * r.affected_node_count as f64,
                    relevance_to_goal: relevance,
                    required_inference_depth: r.avg_root_hops,
                    affected_belief_ids: r.affected_belief_ids.clone(),
                }
            })
            .collect();

        let overall_level = prioritized
            .iter()
            .map(|r| r.severity)
            .fold(0.0_f64, |a, b| a.max(b));

        FocusSummary {
            focus_count: foci.len(),
            top_foci: foci,
            overall_focus_level: overall_level,
        }
    }

    pub fn get_state(&self) -> AttentionState {
        self.state.read().clone()
    }

    pub fn get_adjustment_history(&self) -> Vec<AdjustmentRecord> {
        self.adjustment_history.read().clone()
    }

    pub fn reset_to_default(&self) -> f64 {
        let mut state = self.state.write();
        state.parameter = self.config.default_attention;
        state.mode = AttentionMode::from_alpha(self.config.default_attention);
        state.last_adjustment_timestamp = Utc::now();
        state.parameter
    }

    fn validate_and_clamp(&self, value: f64, bounds: &AttentionBounds) -> Result<f64> {
        if !value.is_finite() {
            return Err(MetacognitiveError::AttentionOutOfBounds {
                value,
                min: bounds.min,
                max: bounds.max,
            });
        }
        Ok(value.clamp(bounds.min, bounds.max))
    }
}

#[derive(Debug, Clone)]
pub struct ResidualInfo {
    pub id: String,
    pub severity: f64,
    pub affected_node_count: usize,
    pub graph_distance: usize,
    pub avg_root_hops: u32,
    pub affected_belief_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_controller() -> AttentionController {
        AttentionController::new(
            AttentionConfig::default(),
            Arc::new(super::super::events::NullMetacognitiveEventPublisher),
        )
    }

    #[test]
    fn test_default_attention_state() {
        let ctrl = create_controller();
        let state = ctrl.get_state();
        assert!((state.parameter - 0.5).abs() < f64::EPSILON);
        assert_eq!(state.mode, AttentionMode::ModerateFocus);
    }

    #[tokio::test]
    async fn test_prediction_verified_decreases_attention() {
        let ctrl = create_controller();
        let result = ctrl
            .adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: None,
                trigger: AdjustmentTrigger::PredictionVerified,
                override_mode: None,
            })
            .await
            .unwrap();

        assert!(result.new_value < result.previous_value);
        assert!((result.new_value - 0.45).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_prediction_deviation_increases_attention() {
        let ctrl = create_controller();
        let result = ctrl
            .adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: None,
                trigger: AdjustmentTrigger::PredictionDeviation,
                override_mode: None,
            })
            .await
            .unwrap();

        assert!(result.new_value > result.previous_value);
        assert!((result.new_value - 0.8).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_time_decay_decreases_attention() {
        let ctrl = create_controller();
        let result = ctrl
            .adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: None,
                trigger: AdjustmentTrigger::TimeDecay,
                override_mode: None,
            })
            .await
            .unwrap();

        assert!(result.new_value < result.previous_value);
    }

    #[tokio::test]
    async fn test_user_override_sets_target() {
        let ctrl = create_controller();
        let result = ctrl
            .adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: Some(0.9),
                trigger: AdjustmentTrigger::UserOverride,
                override_mode: Some(true),
            })
            .await
            .unwrap();

        assert!((result.new_value - 0.9).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_delta_adjustment() {
        let ctrl = create_controller();
        let result = ctrl
            .adjust_attention(AdjustAttentionRequest {
                delta: Some(0.2),
                target_value: None,
                trigger: AdjustmentTrigger::AnomalyCorrection,
                override_mode: None,
            })
            .await
            .unwrap();

        assert!((result.new_value - 0.7).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_bounds_limit() {
        let ctrl = create_controller();
        let result = ctrl
            .adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: Some(2.0),
                trigger: AdjustmentTrigger::UserOverride,
                override_mode: Some(true),
            })
            .await
            .unwrap();

        assert!((result.new_value - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_mode_transition_to_high_reconnaissance() {
        let ctrl = create_controller();
        ctrl.adjust_attention(AdjustAttentionRequest {
            delta: None,
            target_value: Some(0.8),
            trigger: AdjustmentTrigger::UserOverride,
            override_mode: Some(true),
        })
        .await
        .unwrap();

        let state = ctrl.get_state();
        assert_eq!(state.mode, AttentionMode::HighReconnaissance);
    }

    #[tokio::test]
    async fn test_mode_transition_to_low_vigilance() {
        let ctrl = create_controller();
        ctrl.adjust_attention(AdjustAttentionRequest {
            delta: None,
            target_value: Some(0.2),
            trigger: AdjustmentTrigger::UserOverride,
            override_mode: Some(true),
        })
        .await
        .unwrap();

        let state = ctrl.get_state();
        assert_eq!(state.mode, AttentionMode::LowVigilance);
    }

    #[test]
    fn test_set_attention_bounds() {
        let ctrl = create_controller();
        let result = ctrl
            .set_attention_bounds(SetAttentionBoundsRequest {
                min_value: Some(0.2),
                max_value: Some(0.8),
                reason: None,
            })
            .unwrap();

        assert!((result.new_bounds.min - 0.2).abs() < f64::EPSILON);
        assert!((result.new_bounds.max - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_set_attention_bounds_invalid() {
        let ctrl = create_controller();
        let result = ctrl.set_attention_bounds(SetAttentionBoundsRequest {
            min_value: Some(0.6),
            max_value: Some(0.4),
            reason: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_identify_focus_empty() {
        let ctrl = create_controller();
        let summary = ctrl.identify_focus(&[], None);
        assert_eq!(summary.focus_count, 0);
    }

    #[test]
    fn test_identify_focus_with_residuals() {
        let ctrl = create_controller();
        let residuals = vec![
            ResidualInfo {
                id: "r1".to_string(),
                severity: 0.8,
                affected_node_count: 3,
                graph_distance: 1,
                avg_root_hops: 2,
                affected_belief_ids: vec!["b1".to_string()],
            },
            ResidualInfo {
                id: "r2".to_string(),
                severity: 0.3,
                affected_node_count: 1,
                graph_distance: 3,
                avg_root_hops: 4,
                affected_belief_ids: vec!["b2".to_string()],
            },
        ];

        let summary = ctrl.identify_focus(&residuals, Some("goal-1"));
        assert_eq!(summary.focus_count, 2);
        assert!(summary.overall_focus_level > 0.0);
        assert!(summary.top_foci[0].priority > summary.top_foci[1].priority);
    }

    #[test]
    fn test_adjustment_history_recorded() {
        let ctrl = create_controller();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            ctrl.adjust_attention(AdjustAttentionRequest {
                delta: Some(0.1),
                target_value: None,
                trigger: AdjustmentTrigger::AnomalyCorrection,
                override_mode: None,
            })
            .await
            .unwrap();
        });

        let history = ctrl.get_adjustment_history();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_reset_to_default() {
        let ctrl = create_controller();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            ctrl.adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: Some(0.9),
                trigger: AdjustmentTrigger::UserOverride,
                override_mode: Some(true),
            })
            .await
            .unwrap();
        });

        let value = ctrl.reset_to_default();
        assert!((value - 0.5).abs() < f64::EPSILON);
    }
}
