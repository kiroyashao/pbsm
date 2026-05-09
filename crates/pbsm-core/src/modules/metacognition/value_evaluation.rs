use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::config::ValueEvaluationConfig;
use super::error::Result;
use super::events::{MetacognitiveEvent, MetacognitiveEventPublisher};
use super::types::{
    EvaluateMemoryValueRequest, EvaluateMemoryValueResponse, MemoryValueResult,
    UpdateValueWeightsRequest, UpdateValueWeightsResponse, ValidationResult, ValueEvaluation,
    ValueFactors, ValueStatistics, WeightConfiguration,
};

pub struct ValueEvaluator {
    config: ValueEvaluationConfig,
    weight_config: RwLock<WeightConfiguration>,
    value_cache: RwLock<HashMap<String, ValueEvaluation>>,
    belief_access_counts: RwLock<HashMap<String, usize>>,
    belief_last_accessed: RwLock<HashMap<String, i64>>,
    belief_residual_association: RwLock<HashMap<String, f64>>,
    belief_goal_relevance: RwLock<HashMap<String, f64>>,
    event_publisher: Arc<dyn MetacognitiveEventPublisher>,
}

impl ValueEvaluator {
    pub fn new(
        config: ValueEvaluationConfig,
        event_publisher: Arc<dyn MetacognitiveEventPublisher>,
    ) -> Self {
        let weights = config.weights.clone();
        Self {
            config,
            weight_config: RwLock::new(weights),
            value_cache: RwLock::new(HashMap::new()),
            belief_access_counts: RwLock::new(HashMap::new()),
            belief_last_accessed: RwLock::new(HashMap::new()),
            belief_residual_association: RwLock::new(HashMap::new()),
            belief_goal_relevance: RwLock::new(HashMap::new()),
            event_publisher,
        }
    }

    pub async fn evaluate_memory_value(
        &self,
        request: EvaluateMemoryValueRequest,
    ) -> Result<EvaluateMemoryValueResponse> {
        let node_ids = if request.all_active.unwrap_or(false) {
            self.value_cache.read().keys().cloned().collect::<Vec<_>>()
        } else {
            request.node_ids.unwrap_or_default()
        };

        let weights = self.weight_config.read().clone();
        let _forget_threshold = self.config.weights.goal_relevance_weight;

        let mut results = Vec::new();

        for node_id in &node_ids {
            let evaluation = self.compute_value_score(node_id, &weights).await?;
            let forget_recommendation = evaluation.score < 0.2;

            let factors = evaluation.factors.clone();
            let result = MemoryValueResult {
                node_id: node_id.clone(),
                total_score: evaluation.score,
                factors: request
                    .include_factors
                    .unwrap_or(false)
                    .then_some(factors.clone()),
                forget_recommendation,
            };

            self.value_cache.write().insert(node_id.clone(), evaluation);

            let _ = self
                .event_publisher
                .publish(MetacognitiveEvent::MemoryValueCalculated {
                    node_id: node_id.clone(),
                    value_score: result.total_score,
                    factors,
                });

            results.push(result);
        }

        let statistics = compute_statistics(&results, 0.2);

        Ok(EvaluateMemoryValueResponse {
            value_scores: results,
            statistics,
        })
    }

    async fn compute_value_score(
        &self,
        node_id: &str,
        weights: &WeightConfiguration,
    ) -> Result<ValueEvaluation> {
        let factors = self.compute_value_factors(node_id).await?;

        let score = factors.goal_relevance * weights.goal_relevance_weight
            + factors.access_frequency * weights.access_frequency_weight
            + factors.recency * weights.recency_weight
            + factors.residual_association * weights.residual_weight;

        Ok(ValueEvaluation {
            node_id: node_id.to_string(),
            score,
            factors,
            last_calculated: Utc::now(),
        })
    }

    async fn compute_value_factors(&self, node_id: &str) -> Result<ValueFactors> {
        let goal_relevance = self
            .belief_goal_relevance
            .read()
            .get(node_id)
            .copied()
            .unwrap_or(0.5);

        let access_count = self
            .belief_access_counts
            .read()
            .get(node_id)
            .copied()
            .unwrap_or(0);
        let access_frequency =
            (access_count as f64 / self.config.max_access_threshold as f64).min(1.0);

        let last_accessed_steps = self
            .belief_last_accessed
            .read()
            .get(node_id)
            .copied()
            .unwrap_or(100);
        let recency = (-self.config.recency_decay_lambda * last_accessed_steps as f64).exp();

        let residual_association = self
            .belief_residual_association
            .read()
            .get(node_id)
            .copied()
            .unwrap_or(0.0);

        Ok(ValueFactors {
            goal_relevance,
            access_frequency,
            recency,
            residual_association,
        })
    }

    pub fn update_weights(
        &self,
        request: UpdateValueWeightsRequest,
    ) -> Result<UpdateValueWeightsResponse> {
        request.weights.validate()?;

        let previous_weights = self.weight_config.read().clone();
        *self.weight_config.write() = request.weights.clone();

        let new_weights = request.weights;

        Ok(UpdateValueWeightsResponse {
            previous_weights,
            new_weights,
            validation_result: ValidationResult {
                is_valid: true,
                error_message: None,
            },
        })
    }

    pub fn set_belief_access_count(&self, node_id: &str, count: usize) {
        self.belief_access_counts
            .write()
            .insert(node_id.to_string(), count);
    }

    pub fn set_belief_last_accessed(&self, node_id: &str, steps: i64) {
        self.belief_last_accessed
            .write()
            .insert(node_id.to_string(), steps);
    }

    pub fn set_belief_residual_association(&self, node_id: &str, association: f64) {
        self.belief_residual_association
            .write()
            .insert(node_id.to_string(), association);
    }

    pub fn set_belief_goal_relevance(&self, node_id: &str, relevance: f64) {
        self.belief_goal_relevance
            .write()
            .insert(node_id.to_string(), relevance);
    }

    pub fn get_weight_config(&self) -> WeightConfiguration {
        self.weight_config.read().clone()
    }

    pub fn get_cached_value(&self, node_id: &str) -> Option<ValueEvaluation> {
        self.value_cache.read().get(node_id).cloned()
    }

    pub fn register_belief(&self, node_id: &str, goal_relevance: f64) {
        self.belief_goal_relevance
            .write()
            .insert(node_id.to_string(), goal_relevance);
        self.belief_access_counts
            .write()
            .insert(node_id.to_string(), 0);
        self.belief_last_accessed
            .write()
            .insert(node_id.to_string(), 0);
        self.belief_residual_association
            .write()
            .insert(node_id.to_string(), 0.0);
    }

    pub fn remove_belief(&self, node_id: &str) {
        self.value_cache.write().remove(node_id);
        self.belief_access_counts.write().remove(node_id);
        self.belief_last_accessed.write().remove(node_id);
        self.belief_residual_association.write().remove(node_id);
        self.belief_goal_relevance.write().remove(node_id);
    }
}

fn compute_statistics(results: &[MemoryValueResult], threshold: f64) -> ValueStatistics {
    let scores: Vec<f64> = results.iter().map(|r| r.total_score).collect();

    let mean_score = if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    };

    let mut sorted = scores.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let median_score = if sorted.is_empty() {
        0.0
    } else if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };

    let below_threshold_count = results.iter().filter(|r| r.total_score < threshold).count();

    ValueStatistics {
        mean_score,
        median_score,
        below_threshold_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_evaluator() -> ValueEvaluator {
        ValueEvaluator::new(
            ValueEvaluationConfig::default(),
            Arc::new(super::super::events::NullMetacognitiveEventPublisher),
        )
    }

    #[test]
    fn test_default_weights_valid() {
        let weights = WeightConfiguration::default();
        assert!(weights.validate().is_ok());
    }

    #[test]
    fn test_weights_sum_not_one() {
        let weights = WeightConfiguration {
            goal_relevance_weight: 0.5,
            access_frequency_weight: 0.5,
            recency_weight: 0.5,
            residual_weight: 0.5,
        };
        assert!(weights.validate().is_err());
    }

    #[test]
    fn test_weights_out_of_range() {
        let weights = WeightConfiguration {
            goal_relevance_weight: 1.5,
            access_frequency_weight: 0.0,
            recency_weight: 0.0,
            residual_weight: -0.5,
        };
        assert!(weights.validate().is_err());
    }

    #[tokio::test]
    async fn test_evaluate_memory_value() {
        let evaluator = create_evaluator();
        evaluator.register_belief("node-1", 0.8);
        evaluator.set_belief_access_count("node-1", 5);
        evaluator.set_belief_last_accessed("node-1", 2);
        evaluator.set_belief_residual_association("node-1", 0.3);

        let result = evaluator
            .evaluate_memory_value(EvaluateMemoryValueRequest {
                node_ids: Some(vec!["node-1".to_string()]),
                all_active: None,
                include_factors: Some(true),
            })
            .await
            .unwrap();

        assert_eq!(result.value_scores.len(), 1);
        assert!(result.value_scores[0].total_score > 0.0);
        assert!(result.value_scores[0].factors.is_some());
    }

    #[tokio::test]
    async fn test_forget_recommendation() {
        let evaluator = create_evaluator();
        evaluator.register_belief("node-low", 0.0);
        evaluator.set_belief_access_count("node-low", 0);
        evaluator.set_belief_last_accessed("node-low", 100);
        evaluator.set_belief_residual_association("node-low", 0.0);

        let result = evaluator
            .evaluate_memory_value(EvaluateMemoryValueRequest {
                node_ids: Some(vec!["node-low".to_string()]),
                all_active: None,
                include_factors: None,
            })
            .await
            .unwrap();

        assert!(result.value_scores[0].forget_recommendation);
    }

    #[tokio::test]
    async fn test_statistics_calculation() {
        let evaluator = create_evaluator();
        evaluator.register_belief("n1", 0.9);
        evaluator.register_belief("n2", 0.0);
        evaluator.set_belief_last_accessed("n2", 100);
        evaluator.set_belief_residual_association("n2", 0.0);

        let result = evaluator
            .evaluate_memory_value(EvaluateMemoryValueRequest {
                node_ids: Some(vec!["n1".to_string(), "n2".to_string()]),
                all_active: None,
                include_factors: None,
            })
            .await
            .unwrap();

        assert!(result.statistics.below_threshold_count >= 1);
    }

    #[test]
    fn test_update_weights() {
        let evaluator = create_evaluator();
        let new_weights = WeightConfiguration {
            goal_relevance_weight: 0.50,
            access_frequency_weight: 0.20,
            recency_weight: 0.15,
            residual_weight: 0.15,
        };

        let result = evaluator
            .update_weights(UpdateValueWeightsRequest {
                weights: new_weights.clone(),
                persist: None,
            })
            .unwrap();

        assert!(result.validation_result.is_valid);
        assert!((result.new_weights.goal_relevance_weight - 0.50).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_weights_invalid() {
        let evaluator = create_evaluator();
        let bad_weights = WeightConfiguration {
            goal_relevance_weight: 0.5,
            access_frequency_weight: 0.5,
            recency_weight: 0.5,
            residual_weight: 0.5,
        };

        let result = evaluator.update_weights(UpdateValueWeightsRequest {
            weights: bad_weights,
            persist: None,
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_access_frequency_calculation() {
        let evaluator = create_evaluator();
        evaluator.register_belief("n1", 0.5);
        evaluator.set_belief_access_count("n1", 10);

        let result = evaluator
            .evaluate_memory_value(EvaluateMemoryValueRequest {
                node_ids: Some(vec!["n1".to_string()]),
                all_active: None,
                include_factors: Some(true),
            })
            .await
            .unwrap();

        let factors = result.value_scores[0].factors.as_ref().unwrap();
        assert!((factors.access_frequency - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_recency_calculation() {
        let evaluator = create_evaluator();
        evaluator.register_belief("n1", 0.5);
        evaluator.set_belief_last_accessed("n1", 0);

        let result = evaluator
            .evaluate_memory_value(EvaluateMemoryValueRequest {
                node_ids: Some(vec!["n1".to_string()]),
                all_active: None,
                include_factors: Some(true),
            })
            .await
            .unwrap();

        let factors = result.value_scores[0].factors.as_ref().unwrap();
        assert!((factors.recency - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_statistics_empty() {
        let stats = compute_statistics(&[], 0.2);
        assert!((stats.mean_score - 0.0).abs() < f64::EPSILON);
        assert!((stats.median_score - 0.0).abs() < f64::EPSILON);
        assert_eq!(stats.below_threshold_count, 0);
    }

    #[test]
    fn test_register_and_remove_belief() {
        let evaluator = create_evaluator();
        evaluator.register_belief("n1", 0.8);
        assert!(evaluator.get_cached_value("n1").is_none());

        evaluator.remove_belief("n1");
    }
}
