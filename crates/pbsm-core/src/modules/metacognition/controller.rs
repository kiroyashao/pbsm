use std::sync::Arc;

use super::anomaly_detection::AnomalyDetector;
use super::attention::AttentionController;
use super::config::MetacognitiveConfig;
use super::error::Result;
use super::events::{
    LoggingMetacognitiveEventPublisher, MetacognitiveEventPublisher,
    NullMetacognitiveEventPublisher,
};
use super::forgetting::ForgettingExecutor;
use super::types::*;
use super::value_evaluation::ValueEvaluator;
use crate::event_bus::{MetacognitiveEventAdapter, SystemEvent, SystemEventBus};
use crate::modules::common::BeliefGraphReader;

pub struct MetacognitiveController {
    attention: AttentionController,
    value_evaluator: ValueEvaluator,
    forgetting_executor: ForgettingExecutor,
    anomaly_detector: AnomalyDetector,
    belief_graph_reader: Option<Arc<dyn BeliefGraphReader>>,
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
        let logging_publisher: Arc<dyn MetacognitiveEventPublisher> =
            Arc::new(LoggingMetacognitiveEventPublisher::new(event_publisher));

        let attention =
            AttentionController::new(config.attention.clone(), logging_publisher.clone());
        let value_evaluator =
            ValueEvaluator::new(config.value_evaluation.clone(), logging_publisher.clone());
        let forgetting_executor =
            ForgettingExecutor::new(config.forgetting.clone(), logging_publisher.clone());
        let anomaly_detector =
            AnomalyDetector::new(config.anomaly_detection.clone(), logging_publisher);

        Self {
            attention,
            value_evaluator,
            forgetting_executor,
            anomaly_detector,
            belief_graph_reader: None,
            config,
        }
    }

    pub fn with_event_bus(config: MetacognitiveConfig, bus: Arc<SystemEventBus>) -> Self {
        let adapter: Arc<dyn MetacognitiveEventPublisher> =
            Arc::new(MetacognitiveEventAdapter::new(bus));
        Self::with_components(config, adapter)
    }

    pub fn set_belief_graph_reader(&mut self, reader: Arc<dyn BeliefGraphReader>) {
        self.belief_graph_reader = Some(reader);
    }

    pub async fn handle_system_event(&self, event: &SystemEvent) {
        match event {
            SystemEvent::Prediction(pred_event) => {
                use crate::modules::common::PredictionEvent;
                match pred_event {
                    PredictionEvent::PredictionVerified(_) => {
                        let _ = self.attention.adjust_attention(AdjustAttentionRequest {
                            delta: None,
                            target_value: None,
                            trigger: AdjustmentTrigger::PredictionVerified,
                            override_mode: None,
                        }).await;
                    }
                    PredictionEvent::ErrorResidualDetected(_)
                    | PredictionEvent::CriticalResidualDetected(_) => {
                        let _ = self.attention.adjust_attention(AdjustAttentionRequest {
                            delta: None,
                            target_value: None,
                            trigger: AdjustmentTrigger::PredictionDeviation,
                            override_mode: None,
                        }).await;
                    }
                    _ => {}
                }
            }
            SystemEvent::Metacognitive(_) => {}
            SystemEvent::Memory(_) => {}
            SystemEvent::IntentionStack(_) => {
                let _ = self.attention.adjust_attention(AdjustAttentionRequest {
                    delta: None,
                    target_value: None,
                    trigger: AdjustmentTrigger::IntentionChange,
                    override_mode: None,
                }).await;
            }
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

    pub async fn force_forget(&self, request: ForceForgetRequest) -> Result<ForceForgetResponse> {
        let eval_result = self
            .value_evaluator
            .evaluate_memory_value(EvaluateMemoryValueRequest {
                node_ids: Some(request.node_ids.clone()),
                all_active: None,
                include_factors: None,
            })
            .await?;

        let value_scores: std::collections::HashMap<String, f64> = eval_result
            .value_scores
            .iter()
            .map(|r| (r.node_id.clone(), r.total_score))
            .collect();

        self.forgetting_executor
            .trigger_forget(request, &value_scores)
    }

    pub fn get_forget_status(&self) -> GetForgetStatusResponse {
        self.forgetting_executor.get_forget_status()
    }

    pub fn detect_anomalies(&self, window_size: Option<usize>) -> Result<GetAnomalyReportResponse> {
        let history = self.attention.get_adjustment_history();
        self.anomaly_detector.get_anomaly_report(
            GetAnomalyReportRequest {
                include_details: None,
                window_size,
            },
            &history,
        )
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

    #[tokio::test]
    async fn test_forgetting_flow() {
        let controller = MetacognitiveController::new();
        controller.value_evaluator().register_belief("n1", 0.0);
        controller
            .value_evaluator()
            .set_belief_last_accessed("n1", 100);
        controller
            .value_evaluator()
            .set_belief_residual_association("n1", 0.0);
        controller.forgetting_executor().set_belief_age("n1", 20);

        let result = controller
            .force_forget(ForceForgetRequest {
                node_ids: vec!["n1".to_string()],
                force_flag: None,
                reason: ForgetReason::LowValue,
            })
            .await
            .unwrap();

        assert!(result.forgotten_ids.contains(&"n1".to_string()));
    }

    #[test]
    fn test_anomaly_detection_flow() {
        let controller = MetacognitiveController::new();

        let report = controller.detect_anomalies(None).unwrap();
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

        let report = controller.detect_anomalies(None).unwrap();
        assert!(!report.has_anomalies);
    }
}
