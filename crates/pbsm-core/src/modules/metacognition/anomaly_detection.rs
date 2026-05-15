use chrono::Utc;
use parking_lot::RwLock;
use std::sync::Arc;

use super::config::AnomalyDetectionConfig;
use super::error::Result;
use super::events::{MetacognitiveEvent, MetacognitiveEventPublisher};
use super::types::{
    AdjustmentRecord, Anomaly, AnomalyReport, AnomalySeverity, AnomalyType,
    GetAnomalyReportRequest, GetAnomalyReportResponse, Intervention,
    TriggerInterventionRequest, TriggerInterventionResponse,
};

pub struct AnomalyDetector {
    config: AnomalyDetectionConfig,
    event_publisher: Arc<dyn MetacognitiveEventPublisher>,
    last_report: RwLock<Option<AnomalyReport>>,
}

impl AnomalyDetector {
    pub fn new(
        config: AnomalyDetectionConfig,
        event_publisher: Arc<dyn MetacognitiveEventPublisher>,
    ) -> Self {
        Self {
            config,
            event_publisher,
            last_report: RwLock::new(None),
        }
    }

    pub fn detect_anomalies(
        &self,
        history: &[AdjustmentRecord],
        window_size: Option<usize>,
    ) -> AnomalyReport {
        let window_size = window_size.unwrap_or(self.config.anomaly_history_size);
        let mut anomalies = Vec::new();

        if let Some(anomaly) = self.detect_oscillation(history, window_size) {
            anomalies.push(anomaly);
        }

        if let Some(anomaly) = self.detect_locked(history, window_size) {
            anomalies.push(anomaly);
        }

        if let Some(anomaly) = self.detect_excessive_focus(history, window_size) {
            anomalies.push(anomaly);
        }

        if let Some(anomaly) = self.detect_drift(history, window_size) {
            anomalies.push(anomaly);
        }

        let severity = calculate_overall_severity(&anomalies);

        let report = AnomalyReport {
            has_anomalies: !anomalies.is_empty(),
            severity,
            anomalies,
            last_check_timestamp: Utc::now(),
        };

        if report.has_anomalies {
            for anomaly in &report.anomalies {
                let _ =
                    self.event_publisher
                        .publish(MetacognitiveEvent::AttentionAnomalyDetected {
                            anomaly_type: anomaly.anomaly_type,
                            severity: anomaly.severity,
                            evidence: anomaly.evidence.clone(),
                        });
            }
        }

        *self.last_report.write() = Some(report.clone());
        report
    }

    fn detect_oscillation(
        &self,
        history: &[AdjustmentRecord],
        window_size: usize,
    ) -> Option<Anomaly> {
        if history.len() < 3 {
            return None;
        }

        let recent: Vec<&AdjustmentRecord> = history.iter().rev().take(window_size).collect();
        if recent.len() < 3 {
            return None;
        }

        let mut direction_changes = 0usize;
        let deltas: Vec<f64> = recent
            .windows(2)
            .map(|w| w[0].new_value - w[1].new_value)
            .collect();

        for window in deltas.windows(2) {
            if window[0].signum() != window[1].signum() && window[0] != 0.0 && window[1] != 0.0 {
                direction_changes += 1;
            }
        }

        if direction_changes > self.config.oscillation_threshold {
            let severity = if direction_changes > self.config.oscillation_threshold * 2 {
                AnomalySeverity::High
            } else if direction_changes > self.config.oscillation_threshold {
                AnomalySeverity::Medium
            } else {
                AnomalySeverity::Low
            };

            Some(Anomaly {
                anomaly_type: AnomalyType::Oscillation,
                severity,
                evidence: serde_json::json!({
                    "direction_changes": direction_changes,
                    "threshold": self.config.oscillation_threshold
                }),
                detected_at: Utc::now(),
                recommendation: "增加调整延迟参数，平滑参数变化".to_string(),
            })
        } else {
            None
        }
    }

    fn detect_locked(&self, history: &[AdjustmentRecord], window_size: usize) -> Option<Anomaly> {
        if history.len() < 10 {
            return None;
        }

        let recent: Vec<&AdjustmentRecord> = history.iter().rev().take(window_size).collect();
        if recent.len() < 10 {
            return None;
        }

        let current_value = recent[0].new_value;
        let at_boundary =
            current_value <= 0.1 + f64::EPSILON || current_value >= 1.0 - f64::EPSILON;

        if at_boundary {
            let all_at_boundary = recent
                .iter()
                .all(|r| (r.new_value - current_value).abs() < f64::EPSILON);

            if all_at_boundary && recent.len() >= self.config.lock_threshold.min(10) {
                return Some(Anomaly {
                    anomaly_type: AnomalyType::Locked,
                    severity: AnomalySeverity::Medium,
                    evidence: serde_json::json!({
                        "locked_value": current_value,
                        "duration": recent.len()
                    }),
                    detected_at: Utc::now(),
                    recommendation: "强制重置参数到默认值".to_string(),
                });
            }
        }

        None
    }

    fn detect_excessive_focus(
        &self,
        history: &[AdjustmentRecord],
        _window_size: usize,
    ) -> Option<Anomaly> {
        if history.len() < 5 {
            return None;
        }

        let recent: Vec<&AdjustmentRecord> = history.iter().rev().take(10).collect();
        let high_focus_count = recent.iter().filter(|r| r.new_value > 0.7).count();

        if high_focus_count == recent.len() && recent.len() >= 5 {
            Some(Anomaly {
                anomaly_type: AnomalyType::ExcessiveFocus,
                severity: AnomalySeverity::Low,
                evidence: serde_json::json!({
                    "high_focus_count": high_focus_count,
                    "total_count": recent.len()
                }),
                detected_at: Utc::now(),
                recommendation: "建议切换焦点，检查目标覆盖率".to_string(),
            })
        } else {
            None
        }
    }

    fn detect_drift(&self, history: &[AdjustmentRecord], _window_size: usize) -> Option<Anomaly> {
        if history.len() < 10 {
            return None;
        }

        let recent: Vec<&AdjustmentRecord> = history.iter().rev().take(10).collect();
        let values: Vec<f64> = recent.iter().map(|r| r.new_value).collect();

        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;

        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        if std_dev < self.config.drift_threshold && mean > 0.3 && mean < 0.7 {
            return None;
        }

        let first_half: f64 =
            values[..values.len() / 2].iter().sum::<f64>() / (values.len() / 2) as f64;
        let second_half: f64 = values[values.len() / 2..].iter().sum::<f64>()
            / (values.len() - values.len() / 2) as f64;

        let drift_magnitude = (second_half - first_half).abs();
        if drift_magnitude > 0.3 {
            Some(Anomaly {
                anomaly_type: AnomalyType::Drift,
                severity: if drift_magnitude > 0.5 {
                    AnomalySeverity::Medium
                } else {
                    AnomalySeverity::Low
                },
                evidence: serde_json::json!({
                    "drift_magnitude": drift_magnitude,
                    "first_half_mean": first_half,
                    "second_half_mean": second_half
                }),
                detected_at: Utc::now(),
                recommendation: "重置焦点指向顶层目标相关信念".to_string(),
            })
        } else {
            None
        }
    }

    pub fn trigger_intervention(
        &self,
        request: TriggerInterventionRequest,
        current_alpha: f64,
    ) -> Result<TriggerInterventionResponse> {
        let mut interventions = Vec::new();
        let mut parameter_reset = false;

        let force_level = request.force_level.unwrap_or(AnomalySeverity::Medium);
        let anomaly_type = request.anomaly_type.unwrap_or(AnomalyType::Oscillation);

        match force_level {
            AnomalySeverity::Low => {
                interventions.push(Intervention {
                    anomaly_type,
                    action: "LOG_ONLY".to_string(),
                    result: "Warning logged".to_string(),
                });
            }
            AnomalySeverity::Medium => {
                let action = match anomaly_type {
                    AnomalyType::Oscillation => "INCREASE_DELAY",
                    AnomalyType::Drift => "FOCUS_RESET",
                    AnomalyType::ExcessiveFocus => "SUGGEST_SWITCH",
                    AnomalyType::Locked => "SUGGEST_RESET",
                };
                interventions.push(Intervention {
                    anomaly_type,
                    action: action.to_string(),
                    result: format!("Medium intervention applied for {:?}", anomaly_type),
                });
            }
            AnomalySeverity::High => {
                parameter_reset = true;
                interventions.push(Intervention {
                    anomaly_type,
                    action: "FORCE_RESET".to_string(),
                    result: format!("Forced reset from {} to 0.5", current_alpha),
                });
            }
            AnomalySeverity::None => {}
        }

        if parameter_reset {
            let _ = self
                .event_publisher
                .publish(MetacognitiveEvent::AttentionAnomalyResolved {
                    anomaly_type,
                    resolution: "FORCE_RESET".to_string(),
                });
        }

        Ok(TriggerInterventionResponse {
            interventions,
            parameter_reset,
        })
    }

    pub fn get_anomaly_report(
        &self,
        request: GetAnomalyReportRequest,
        history: &[AdjustmentRecord],
    ) -> Result<GetAnomalyReportResponse> {
        let report = self.detect_anomalies(history, request.window_size);
        Ok(GetAnomalyReportResponse {
            has_anomalies: report.has_anomalies,
            severity: report.severity,
            anomalies: report.anomalies,
            last_check_timestamp: report.last_check_timestamp,
        })
    }

    pub fn get_last_anomaly_report(&self) -> Option<AnomalyReport> {
        self.last_report.read().clone()
    }
}

fn calculate_overall_severity(anomalies: &[Anomaly]) -> AnomalySeverity {
    if anomalies.is_empty() {
        return AnomalySeverity::None;
    }

    let has_high = anomalies
        .iter()
        .any(|a| a.severity == AnomalySeverity::High);
    let has_medium = anomalies
        .iter()
        .any(|a| a.severity == AnomalySeverity::Medium);

    if has_high || anomalies.len() > 1 {
        AnomalySeverity::High
    } else if has_medium {
        AnomalySeverity::Medium
    } else {
        AnomalySeverity::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::metacognition::types::AdjustmentTrigger;

    fn create_detector() -> AnomalyDetector {
        AnomalyDetector::new(
            AnomalyDetectionConfig::default(),
            Arc::new(super::super::events::NullMetacognitiveEventPublisher),
        )
    }

    fn make_record(prev: f64, new: f64, trigger: AdjustmentTrigger) -> AdjustmentRecord {
        AdjustmentRecord {
            timestamp: Utc::now(),
            previous_value: prev,
            new_value: new,
            trigger,
        }
    }

    #[test]
    fn test_no_anomalies_with_empty_history() {
        let detector = create_detector();
        let report = detector.detect_anomalies(&[], None);
        assert!(!report.has_anomalies);
        assert_eq!(report.severity, AnomalySeverity::None);
    }

    #[test]
    fn test_oscillation_detection() {
        let detector = create_detector();
        let history: Vec<AdjustmentRecord> = (0..20)
            .map(|i| {
                let value = if i % 2 == 0 { 0.9 } else { 0.1 };
                make_record(0.5, value, AdjustmentTrigger::PredictionDeviation)
            })
            .collect();

        let report = detector.detect_anomalies(&history, None);
        let oscillation = report
            .anomalies
            .iter()
            .find(|a| a.anomaly_type == AnomalyType::Oscillation);
        assert!(oscillation.is_some());
    }

    #[test]
    fn test_no_oscillation_with_stable_history() {
        let detector = create_detector();
        let history: Vec<AdjustmentRecord> = (0..10)
            .map(|i| {
                make_record(
                    0.5 - i as f64 * 0.01,
                    0.5 - (i + 1) as f64 * 0.01,
                    AdjustmentTrigger::TimeDecay,
                )
            })
            .collect();

        let report = detector.detect_anomalies(&history, None);
        let oscillation = report
            .anomalies
            .iter()
            .find(|a| a.anomaly_type == AnomalyType::Oscillation);
        assert!(oscillation.is_none());
    }

    #[test]
    fn test_locked_detection() {
        let detector = create_detector();
        let history: Vec<AdjustmentRecord> = (0..15)
            .map(|_| make_record(1.0, 1.0, AdjustmentTrigger::PredictionDeviation))
            .collect();

        let report = detector.detect_anomalies(&history, None);
        let locked = report
            .anomalies
            .iter()
            .find(|a| a.anomaly_type == AnomalyType::Locked);
        assert!(locked.is_some());
    }

    #[test]
    fn test_no_locked_when_not_at_boundary() {
        let detector = create_detector();
        let history: Vec<AdjustmentRecord> = (0..15)
            .map(|_| make_record(0.5, 0.5, AdjustmentTrigger::TimeDecay))
            .collect();

        let report = detector.detect_anomalies(&history, None);
        let locked = report
            .anomalies
            .iter()
            .find(|a| a.anomaly_type == AnomalyType::Locked);
        assert!(locked.is_none());
    }

    #[test]
    fn test_severity_calculation_none() {
        assert_eq!(calculate_overall_severity(&[]), AnomalySeverity::None);
    }

    #[test]
    fn test_severity_calculation_high() {
        let anomalies = vec![Anomaly {
            anomaly_type: AnomalyType::Oscillation,
            severity: AnomalySeverity::High,
            evidence: serde_json::json!({}),
            detected_at: Utc::now(),
            recommendation: String::new(),
        }];
        assert_eq!(
            calculate_overall_severity(&anomalies),
            AnomalySeverity::High
        );
    }

    #[test]
    fn test_severity_calculation_multiple_low() {
        let anomalies = vec![
            Anomaly {
                anomaly_type: AnomalyType::Oscillation,
                severity: AnomalySeverity::Low,
                evidence: serde_json::json!({}),
                detected_at: Utc::now(),
                recommendation: String::new(),
            },
            Anomaly {
                anomaly_type: AnomalyType::Drift,
                severity: AnomalySeverity::Low,
                evidence: serde_json::json!({}),
                detected_at: Utc::now(),
                recommendation: String::new(),
            },
        ];
        assert_eq!(
            calculate_overall_severity(&anomalies),
            AnomalySeverity::High
        );
    }

    #[test]
    fn test_trigger_intervention_low() {
        let detector = create_detector();
        let result = detector
            .trigger_intervention(
                TriggerInterventionRequest {
                    anomaly_type: Some(AnomalyType::Oscillation),
                    force_level: Some(AnomalySeverity::Low),
                },
                0.5,
            )
            .unwrap();

        assert_eq!(result.interventions.len(), 1);
        assert_eq!(result.interventions[0].action, "LOG_ONLY");
        assert!(!result.parameter_reset);
    }

    #[test]
    fn test_trigger_intervention_high() {
        let detector = create_detector();
        let result = detector
            .trigger_intervention(
                TriggerInterventionRequest {
                    anomaly_type: Some(AnomalyType::Locked),
                    force_level: Some(AnomalySeverity::High),
                },
                1.0,
            )
            .unwrap();

        assert_eq!(result.interventions.len(), 1);
        assert!(result.parameter_reset);
    }

    #[test]
    fn test_trigger_intervention_medium_oscillation() {
        let detector = create_detector();
        let result = detector
            .trigger_intervention(
                TriggerInterventionRequest {
                    anomaly_type: Some(AnomalyType::Oscillation),
                    force_level: Some(AnomalySeverity::Medium),
                },
                0.5,
            )
            .unwrap();

        assert_eq!(result.interventions[0].action, "INCREASE_DELAY");
    }

    #[test]
    fn test_get_anomaly_report_initially_none() {
        let detector = create_detector();
        assert!(detector.get_last_anomaly_report().is_none());
    }

    #[test]
    fn test_get_anomaly_report_after_detection() {
        let detector = create_detector();
        let history: Vec<AdjustmentRecord> = (0..15)
            .map(|_| make_record(1.0, 1.0, AdjustmentTrigger::PredictionDeviation))
            .collect();
        detector.detect_anomalies(&history, None);

        assert!(detector.get_last_anomaly_report().is_some());
    }
}
