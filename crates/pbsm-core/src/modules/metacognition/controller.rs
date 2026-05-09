use std::sync::Arc;

use super::anomaly_detection::AnomalyDetector;
use super::attention::AttentionController;
use super::config::MetacognitiveConfig;
use super::error::Result;
use super::events::{MetacognitiveEventPublisher, NullMetacognitiveEventPublisher};
use super::forgetting::ForgettingExecutor;
use super::types::*;
use super::value_evaluation::ValueEvaluator;

pub struct MetacognitiveController {
    attention: AttentionController,
    value_evaluator: ValueEvaluator,
    forgetting_executor: ForgettingExecutor,
    anomaly_detector: AnomalyDetector,
    config: MetacognitiveConfig,
}

impl MetacognitiveController {
    pub fn new() -> Self {
        Self::with_config(MetacognitiveConfig::default())
    }

    pub fn with_config(config: MetacognitiveConfig) -> Self {
        let event_publisher: Arc<dyn MetacognitiveEventPublisher> =
            Arc::new(NullMetacognitiveEventPublisher);
        Self::with_components(config, event_publisher)
    }

    pub fn with_components(
        config: MetacognitiveConfig,
        event_publisher: Arc<dyn MetacognitiveEventPublisher>,
    ) -> Self {
        let attention = AttentionController::new(config.attention.clone(), event_publisher.clone());
        let value_evaluator =
            ValueEvaluator::new(config.value_evaluation.clone(), event_publisher.clone());
        let forgetting_executor =
            ForgettingExecutor::new(config.forgetting.clone(), event_publisher.clone());
        let anomaly_detector =
            AnomalyDetector::new(config.anomaly_detection.clone(), event_publisher);

        Self {
            attention,
            value_evaluator,
            forgetting_executor,
            anomaly_detector,
            config,
        }
    }

    pub async fn get_attention_status(&self) -> GetAttentionStatusResponse {
        self.attention.get_attention_status()
    }

    pub async fn adjust_attention(
        &self,
        request: AdjustAttentionRequest,
    ) -> Result<AdjustAttentionResponse> {
        self.attention.adjust_attention(request).await
    }

    pub fn set_attention_bounds(
        &self,
        request: SetAttentionBoundsRequest,
    ) -> Result<SetAttentionBoundsResponse> {
        self.attention.set_attention_bounds(request)
    }

    pub async fn evaluate_memory_value(
        &self,
        request: EvaluateMemoryValueRequest,
    ) -> Result<EvaluateMemoryValueResponse> {
        self.value_evaluator.evaluate_memory_value(request).await
    }

    pub fn update_value_weights(
        &self,
        request: UpdateValueWeightsRequest,
    ) -> Result<UpdateValueWeightsResponse> {
        self.value_evaluator.update_weights(request)
    }

    pub fn force_forget(&self, request: ForceForgetRequest) -> Result<ForceForgetResponse> {
        self.forgetting_executor.trigger_forget(request)
    }

    pub fn get_forget_status(&self) -> GetForgetStatusResponse {
        self.forgetting_executor.get_forget_status()
    }

    pub fn detect_anomalies(&self, window_size: Option<usize>) -> GetAnomalyReportResponse {
        let history = self.attention.get_adjustment_history();
        let report = self
            .anomaly_detector
            .detect_anomalies(&history, window_size);
        GetAnomalyReportResponse {
            has_anomalies: report.has_anomalies,
            severity: report.severity,
            anomalies: report.anomalies,
            last_check_timestamp: report.last_check_timestamp,
        }
    }

    pub fn trigger_intervention(
        &self,
        request: TriggerInterventionRequest,
    ) -> Result<TriggerInterventionResponse> {
        let state = self.attention.get_state();
        self.anomaly_detector
            .trigger_intervention(request, state.parameter)
    }

    pub fn get_config(&self) -> &MetacognitiveConfig {
        &self.config
    }

    pub fn attention_controller(&self) -> &AttentionController {
        &self.attention
    }

    pub fn value_evaluator(&self) -> &ValueEvaluator {
        &self.value_evaluator
    }

    pub fn forgetting_executor(&self) -> &ForgettingExecutor {
        &self.forgetting_executor
    }

    pub fn anomaly_detector(&self) -> &AnomalyDetector {
        &self.anomaly_detector
    }
}

impl Default for MetacognitiveController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_creation() {
        let controller = MetacognitiveController::new();
        let config = controller.get_config();
        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_attention_adjustment_flow() {
        let controller = MetacognitiveController::new();

        let status = controller.get_attention_status().await;
        assert!((status.attention_parameter - 0.5).abs() < f64::EPSILON);

        let result = controller
            .adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: None,
                trigger: AdjustmentTrigger::PredictionDeviation,
                override_mode: None,
            })
            .await
            .unwrap();

        assert!(result.new_value > result.previous_value);
    }

    #[tokio::test]
    async fn test_value_evaluation_flow() {
        let controller = MetacognitiveController::new();
        controller.value_evaluator().register_belief("node-1", 0.8);
        controller
            .value_evaluator()
            .set_belief_access_count("node-1", 5);
        controller
            .value_evaluator()
            .set_belief_last_accessed("node-1", 2);

        let result = controller
            .evaluate_memory_value(EvaluateMemoryValueRequest {
                node_ids: Some(vec!["node-1".to_string()]),
                all_active: None,
                include_factors: Some(true),
            })
            .await
            .unwrap();

        assert_eq!(result.value_scores.len(), 1);
        assert!(result.value_scores[0].total_score > 0.0);
    }

    #[test]
    fn test_forgetting_flow() {
        let controller = MetacognitiveController::new();
        controller.forgetting_executor().set_belief_age("n1", 20);

        let result = controller
            .force_forget(ForceForgetRequest {
                node_ids: vec!["n1".to_string()],
                force_flag: None,
                reason: ForgetReason::LowValue,
            })
            .unwrap();

        assert!(result.forgotten_ids.contains(&"n1".to_string()));
    }

    #[test]
    fn test_anomaly_detection_flow() {
        let controller = MetacognitiveController::new();

        let report = controller.detect_anomalies(None);
        assert!(!report.has_anomalies);
    }

    #[tokio::test]
    async fn test_set_attention_bounds() {
        let controller = MetacognitiveController::new();

        let result = controller
            .set_attention_bounds(SetAttentionBoundsRequest {
                min_value: Some(0.2),
                max_value: Some(0.8),
                reason: None,
            })
            .unwrap();

        assert!((result.new_bounds.min - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_weights() {
        let controller = MetacognitiveController::new();

        let new_weights = WeightConfiguration {
            goal_relevance_weight: 0.50,
            access_frequency_weight: 0.20,
            recency_weight: 0.15,
            residual_weight: 0.15,
        };

        let result = controller
            .update_value_weights(UpdateValueWeightsRequest {
                weights: new_weights,
                persist: None,
            })
            .unwrap();

        assert!(result.validation_result.is_valid);
    }

    #[test]
    fn test_get_forget_status() {
        let controller = MetacognitiveController::new();
        let status = controller.get_forget_status();
        assert_eq!(status.statistics.total_forgotten_this_session, 0);
    }

    #[tokio::test]
    async fn test_full_workflow() {
        let controller = MetacognitiveController::new();

        controller.value_evaluator().register_belief("b1", 0.9);
        controller.value_evaluator().register_belief("b2", 0.1);
        controller
            .value_evaluator()
            .set_belief_access_count("b1", 8);
        controller
            .value_evaluator()
            .set_belief_last_accessed("b1", 1);
        controller
            .value_evaluator()
            .set_belief_access_count("b2", 0);
        controller
            .value_evaluator()
            .set_belief_last_accessed("b2", 100);

        controller
            .adjust_attention(AdjustAttentionRequest {
                delta: None,
                target_value: Some(0.8),
                trigger: AdjustmentTrigger::UserOverride,
                override_mode: Some(true),
            })
            .await
            .unwrap();

        let status = controller.get_attention_status().await;
        assert!((status.attention_parameter - 0.8).abs() < f64::EPSILON);

        let eval = controller
            .evaluate_memory_value(EvaluateMemoryValueRequest {
                node_ids: Some(vec!["b1".to_string(), "b2".to_string()]),
                all_active: None,
                include_factors: Some(true),
            })
            .await
            .unwrap();

        assert_eq!(eval.value_scores.len(), 2);

        let report = controller.detect_anomalies(None);
        assert!(!report.has_anomalies);
    }
}
