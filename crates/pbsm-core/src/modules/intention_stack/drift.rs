use chrono::Utc;

use super::error::Result;
use super::state::DriftSeverity;
use super::types::{
    CorrectiveAction, DriftAssessment, DriftComponents, DriftThreshold, DriftTrend, IntentionLayer,
};

pub struct DriftDetector {
    threshold: DriftThreshold,
}

impl DriftDetector {
    pub fn new(threshold: DriftThreshold) -> Self {
        Self { threshold }
    }

    pub fn detect_drift(
        &self,
        layer: &IntentionLayer,
        root_layer: Option<&IntentionLayer>,
    ) -> DriftAssessment {
        let directional_drift = self.calculate_directional_drift(layer, root_layer);
        let scope_drift = self.calculate_scope_drift(layer);
        let priority_drift = self.calculate_priority_drift(layer, root_layer);
        let method_drift = self.calculate_method_drift(layer);

        let overall_score =
            directional_drift * 0.4 + scope_drift * 0.2 + priority_drift * 0.2 + method_drift * 0.2;

        let severity = self.determine_severity(overall_score);
        let trend = self.determine_trend(layer);
        let root_cause =
            self.analyze_root_cause(directional_drift, scope_drift, priority_drift, method_drift);
        let recommended_action = self.get_recommended_action(&severity);

        DriftAssessment {
            layer_id: layer.layer_id.clone(),
            severity,
            overall_drift_score: overall_score,
            components: DriftComponents {
                directional_drift,
                scope_drift,
                priority_drift,
                method_drift,
            },
            trend,
            root_cause_hypothesis: if root_cause.is_empty() {
                None
            } else {
                Some(root_cause.join("; "))
            },
            timestamp: Utc::now(),
            recommended_action,
        }
    }

    pub fn handle_drift(
        &self,
        layer: &mut IntentionLayer,
        action: CorrectiveAction,
    ) -> Result<super::types::DriftHandlingResult> {
        match action {
            CorrectiveAction::None => {}
            CorrectiveAction::Replan => {
                layer.drift_status.root_cause_hypothesis =
                    Some("Replan triggered due to drift".to_string());
            }
            CorrectiveAction::Reorder => {
                layer.drift_status.root_cause_hypothesis =
                    Some("Step reorder triggered due to drift".to_string());
            }
            CorrectiveAction::Recontextualize => {
                layer.drift_status.root_cause_hypothesis =
                    Some("Recontextualization triggered due to drift".to_string());
            }
            CorrectiveAction::Escalate => {
                layer.drift_status.root_cause_hypothesis =
                    Some("Drift escalated to parent layer".to_string());
            }
            CorrectiveAction::Abandon => {
                layer.drift_status.is_drifting = false;
                layer.drift_status.drift_angle = 0.0;
            }
            CorrectiveAction::Rollback => {
                layer.drift_status.is_drifting = false;
                layer.drift_status.drift_angle = 0.0;
                layer.drift_status.drift_direction = None;
                layer.drift_status.drift_start_time = None;
                layer.drift_status.root_cause_hypothesis = None;
            }
        }

        Ok(super::types::DriftHandlingResult {
            success: true,
            action_taken: action,
            new_drift_status: layer.drift_status.clone(),
        })
    }

    pub fn set_drift_threshold(&mut self, threshold: DriftThreshold) {
        self.threshold = threshold;
    }

    pub fn threshold(&self) -> &DriftThreshold {
        &self.threshold
    }

    fn calculate_directional_drift(
        &self,
        layer: &IntentionLayer,
        root_layer: Option<&IntentionLayer>,
    ) -> f64 {
        if root_layer.is_none() {
            return 0.0;
        }
        let progress_factor = 1.0 - (layer.progress_metrics.progress_percentage / 100.0).min(1.0);
        let depth_factor = (layer.level as f64 * 0.05).min(0.5);
        (progress_factor * 0.6 + depth_factor * 0.4).min(1.0)
    }

    fn calculate_scope_drift(&self, layer: &IntentionLayer) -> f64 {
        let child_ratio = if layer.goal.success_criteria.is_empty() {
            0.0
        } else {
            layer.child_levels.len() as f64 / (layer.goal.success_criteria.len() as f64 + 1.0)
        };
        child_ratio.min(1.0) * 0.5
    }

    fn calculate_priority_drift(
        &self,
        layer: &IntentionLayer,
        root_layer: Option<&IntentionLayer>,
    ) -> f64 {
        match root_layer {
            Some(root) => {
                let root_weight = root.goal.priority.weight();
                let layer_weight = layer.goal.priority.weight();
                if layer_weight > root_weight {
                    0.0
                } else {
                    (root_weight - layer_weight) as f64 * 0.5
                }
            }
            None => 0.0,
        }
    }

    fn calculate_method_drift(&self, layer: &IntentionLayer) -> f64 {
        let failed_ratio = if layer.progress_metrics.total_steps == 0 {
            0.0
        } else {
            layer.progress_metrics.failed_steps as f64 / layer.progress_metrics.total_steps as f64
        };
        failed_ratio.min(1.0)
    }

    fn determine_severity(&self, score: f64) -> DriftSeverity {
        if score >= self.threshold.critical {
            DriftSeverity::Critical
        } else if score >= self.threshold.severe {
            DriftSeverity::Severe
        } else if score >= self.threshold.moderate {
            DriftSeverity::Moderate
        } else if score >= self.threshold.warning {
            DriftSeverity::Minor
        } else {
            DriftSeverity::None
        }
    }

    fn determine_trend(&self, layer: &IntentionLayer) -> DriftTrend {
        if layer.drift_status.is_drifting {
            DriftTrend::Increasing
        } else {
            DriftTrend::Stable
        }
    }

    fn analyze_root_cause(
        &self,
        directional: f64,
        scope: f64,
        priority: f64,
        method: f64,
    ) -> Vec<String> {
        let mut hypotheses = Vec::new();
        if directional > 0.5 {
            hypotheses.push("Execution path diverging from target state".to_string());
        }
        if scope > 0.5 {
            hypotheses.push("Scope expanding beyond original boundaries".to_string());
        }
        if priority > 0.5 {
            hypotheses.push("Lower priority tasks dominating execution".to_string());
        }
        if method > 0.5 {
            hypotheses.push("Problem-solving approach changed significantly".to_string());
        }
        hypotheses
    }

    fn get_recommended_action(&self, severity: &DriftSeverity) -> CorrectiveAction {
        match severity {
            DriftSeverity::None => CorrectiveAction::None,
            DriftSeverity::Minor => CorrectiveAction::None,
            DriftSeverity::Moderate => CorrectiveAction::Replan,
            DriftSeverity::Severe => CorrectiveAction::Escalate,
            DriftSeverity::Critical => CorrectiveAction::Rollback,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::intention_stack::state::GoalPriority;
    use crate::modules::intention_stack::types::GoalDefinition;

    fn create_test_layer() -> IntentionLayer {
        let goal = GoalDefinition::simple("Test goal".to_string(), GoalPriority::Medium);
        IntentionLayer::new(goal, 0, None)
    }

    #[test]
    fn test_detect_drift_no_root() {
        let detector = DriftDetector::new(DriftThreshold::default());
        let layer = create_test_layer();
        let assessment = detector.detect_drift(&layer, None);
        assert_eq!(assessment.severity, DriftSeverity::None);
    }

    #[test]
    fn test_detect_drift_with_root() {
        let detector = DriftDetector::new(DriftThreshold::default());
        let root_goal = GoalDefinition::simple("Root".to_string(), GoalPriority::Critical);
        let root = IntentionLayer::new(root_goal, 0, None);
        let child_goal = GoalDefinition::simple("Child".to_string(), GoalPriority::Low);
        let mut child = IntentionLayer::new(child_goal, 1, Some(0));
        child.progress_metrics.failed_steps = 5;
        child.progress_metrics.total_steps = 5;

        let assessment = detector.detect_drift(&child, Some(&root));
        assert!(assessment.overall_drift_score > 0.0);
    }

    #[test]
    fn test_handle_drift_none() {
        let detector = DriftDetector::new(DriftThreshold::default());
        let mut layer = create_test_layer();
        let result = detector.handle_drift(&mut layer, CorrectiveAction::None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().action_taken, CorrectiveAction::None);
    }

    #[test]
    fn test_handle_drift_rollback() {
        let detector = DriftDetector::new(DriftThreshold::default());
        let mut layer = create_test_layer();
        layer.drift_status.is_drifting = true;
        layer.drift_status.drift_angle = 0.8;

        let result = detector.handle_drift(&mut layer, CorrectiveAction::Rollback);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.new_drift_status.is_drifting);
    }

    #[test]
    fn test_set_drift_threshold() {
        let mut detector = DriftDetector::new(DriftThreshold::default());
        let new_threshold = DriftThreshold {
            warning: 0.2,
            moderate: 0.4,
            severe: 0.6,
            critical: 0.8,
        };
        detector.set_drift_threshold(new_threshold.clone());
        assert!((detector.threshold().warning - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_severity_determination() {
        let detector = DriftDetector::new(DriftThreshold::default());
        let layer = create_test_layer();

        let assessment = detector.detect_drift(&layer, None);
        assert_eq!(assessment.severity, DriftSeverity::None);
    }
}
