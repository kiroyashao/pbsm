use serde::{Deserialize, Serialize};

use super::error::{MetacognitiveError, Result};
use super::types::WeightConfiguration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub min_attention: f64,
    pub max_attention: f64,
    pub default_attention: f64,
    pub decay_rate: f64,
    pub boost_step: f64,
    pub time_decay_rate: f64,
    pub max_adjustment: f64,
    pub min_adjustment_interval_ms: u64,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            min_attention: 0.1,
            max_attention: 1.0,
            default_attention: 0.5,
            decay_rate: 0.05,
            boost_step: 0.4,
            time_decay_rate: 0.001,
            max_adjustment: 0.3,
            min_adjustment_interval_ms: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueEvaluationConfig {
    pub weights: WeightConfiguration,
    pub recency_decay_lambda: f64,
    pub access_window_size: usize,
    pub max_access_threshold: usize,
    pub forget_threshold: f64,
}

impl Default for ValueEvaluationConfig {
    fn default() -> Self {
        Self {
            weights: WeightConfiguration::default(),
            recency_decay_lambda: 0.05,
            access_window_size: 50,
            max_access_threshold: 10,
            forget_threshold: 0.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgettingConfig {
    pub forget_threshold: f64,
    pub max_active_beliefs: usize,
    pub min_survival_steps: usize,
    pub forget_cooldown_steps: usize,
    pub max_defer_steps: usize,
    pub batch_forgive_interval: usize,
    pub residual_defer_threshold: f64,
}

impl Default for ForgettingConfig {
    fn default() -> Self {
        Self {
            forget_threshold: 0.2,
            max_active_beliefs: 500,
            min_survival_steps: 10,
            forget_cooldown_steps: 20,
            max_defer_steps: 200,
            batch_forgive_interval: 50,
            residual_defer_threshold: 0.7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectionConfig {
    pub coverage_threshold: f64,
    pub oscillation_threshold: usize,
    pub drift_threshold: f64,
    pub lock_threshold: usize,
    pub anomaly_check_interval: usize,
    pub anomaly_history_size: usize,
}

impl Default for AnomalyDetectionConfig {
    fn default() -> Self {
        Self {
            coverage_threshold: 0.3,
            oscillation_threshold: 5,
            drift_threshold: 0.2,
            lock_threshold: 100,
            anomaly_check_interval: 25,
            anomaly_history_size: 100,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetacognitiveConfig {
    pub attention: AttentionConfig,
    pub value_evaluation: ValueEvaluationConfig,
    pub forgetting: ForgettingConfig,
    pub anomaly_detection: AnomalyDetectionConfig,
}

impl MetacognitiveConfig {
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=0.5).contains(&self.attention.min_attention) {
            return Err(MetacognitiveError::ConfigurationError(
                "min_attention must be in range [0, 0.5]".to_string(),
            ));
        }
        if !(0.5..=1.0).contains(&self.attention.max_attention) {
            return Err(MetacognitiveError::ConfigurationError(
                "max_attention must be in range [0.5, 1]".to_string(),
            ));
        }
        if self.attention.min_attention >= self.attention.max_attention {
            return Err(MetacognitiveError::ConfigurationError(
                "min_attention must be less than max_attention".to_string(),
            ));
        }
        if !(0.0..=0.1).contains(&self.attention.decay_rate) {
            return Err(MetacognitiveError::ConfigurationError(
                "decay_rate must be in range (0, 0.1]".to_string(),
            ));
        }
        if self.attention.decay_rate <= 0.0 {
            return Err(MetacognitiveError::ConfigurationError(
                "decay_rate must be greater than 0".to_string(),
            ));
        }
        if !(0.3..=0.5).contains(&self.attention.boost_step) {
            return Err(MetacognitiveError::ConfigurationError(
                "boost_step must be in range [0.3, 0.5]".to_string(),
            ));
        }
        if !(0.0..=0.01).contains(&self.attention.time_decay_rate) {
            return Err(MetacognitiveError::ConfigurationError(
                "time_decay_rate must be in range (0, 0.01]".to_string(),
            ));
        }
        if self.attention.time_decay_rate <= 0.0 {
            return Err(MetacognitiveError::ConfigurationError(
                "time_decay_rate must be greater than 0".to_string(),
            ));
        }
        if !(0.1..=0.5).contains(&self.attention.max_adjustment) {
            return Err(MetacognitiveError::ConfigurationError(
                "max_adjustment must be in range [0.1, 0.5]".to_string(),
            ));
        }

        self.value_evaluation.weights.validate()?;

        if self.value_evaluation.recency_decay_lambda <= 0.0
            || self.value_evaluation.recency_decay_lambda > 0.1
        {
            return Err(MetacognitiveError::ConfigurationError(
                "recency_decay_lambda must be in range (0, 0.1]".to_string(),
            ));
        }

        if !(0.1..=0.5).contains(&self.forgetting.forget_threshold) {
            return Err(MetacognitiveError::ConfigurationError(
                "forget_threshold must be in range [0.1, 0.5]".to_string(),
            ));
        }
        if !(100..=1000).contains(&self.forgetting.max_active_beliefs) {
            return Err(MetacognitiveError::ConfigurationError(
                "max_active_beliefs must be in range [100, 1000]".to_string(),
            ));
        }

        if !(0.1..=0.5).contains(&self.anomaly_detection.coverage_threshold) {
            return Err(MetacognitiveError::ConfigurationError(
                "coverage_threshold must be in range [0.1, 0.5]".to_string(),
            ));
        }
        if !(3..=10).contains(&self.anomaly_detection.oscillation_threshold) {
            return Err(MetacognitiveError::ConfigurationError(
                "oscillation_threshold must be in range [3, 10]".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = MetacognitiveConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_min_attention() {
        let mut config = MetacognitiveConfig::default();
        config.attention.min_attention = 0.6;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_min_ge_max_attention() {
        let mut config = MetacognitiveConfig::default();
        config.attention.min_attention = 0.5;
        config.attention.max_attention = 0.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_boost_step() {
        let mut config = MetacognitiveConfig::default();
        config.attention.boost_step = 0.2;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_forget_threshold() {
        let mut config = MetacognitiveConfig::default();
        config.forgetting.forget_threshold = 0.6;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_weights() {
        let mut config = MetacognitiveConfig::default();
        config.value_evaluation.weights.goal_relevance_weight = 0.5;
        config.value_evaluation.weights.access_frequency_weight = 0.5;
        config.value_evaluation.weights.recency_weight = 0.5;
        config.value_evaluation.weights.residual_weight = 0.5;
        assert!(config.validate().is_err());
    }
}
