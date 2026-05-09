use serde::{Deserialize, Serialize};

use super::error::{IntentionStackError, Result};
use super::types::DriftThreshold;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentionStackConfig {
    pub max_stack_depth: usize,
    pub max_stack_capacity: usize,
    pub drift_threshold: DriftThreshold,
    pub max_revert_depth: usize,
    pub root_visibility_threshold: f64,
    pub default_step_timeout: i64,
    pub default_max_retries: usize,
    pub max_checkpoints_per_layer: usize,
}

impl Default for IntentionStackConfig {
    fn default() -> Self {
        Self {
            max_stack_depth: 20,
            max_stack_capacity: 500,
            drift_threshold: DriftThreshold::default(),
            max_revert_depth: 5,
            root_visibility_threshold: 0.6,
            default_step_timeout: 30000,
            default_max_retries: 3,
            max_checkpoints_per_layer: 20,
        }
    }
}

impl IntentionStackConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_stack_depth == 0 || self.max_stack_depth > 50 {
            return Err(IntentionStackError::Internal(format!(
                "max_stack_depth must be in range [1, 50], got {}",
                self.max_stack_depth
            )));
        }
        if self.max_stack_capacity == 0 || self.max_stack_capacity > 10000 {
            return Err(IntentionStackError::Internal(format!(
                "max_stack_capacity must be in range [1, 10000], got {}",
                self.max_stack_capacity
            )));
        }
        if self.max_revert_depth == 0 || self.max_revert_depth > self.max_stack_depth {
            return Err(IntentionStackError::Internal(format!(
                "max_revert_depth must be in range [1, {}], got {}",
                self.max_stack_depth, self.max_revert_depth
            )));
        }
        if !(0.0..=1.0).contains(&self.root_visibility_threshold) {
            return Err(IntentionStackError::Internal(format!(
                "root_visibility_threshold must be in range [0, 1], got {}",
                self.root_visibility_threshold
            )));
        }
        if self.max_checkpoints_per_layer == 0 {
            return Err(IntentionStackError::Internal(format!(
                "max_checkpoints_per_layer must be > 0, got {}",
                self.max_checkpoints_per_layer
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = IntentionStackConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_zero_depth_invalid() {
        let mut config = IntentionStackConfig::default();
        config.max_stack_depth = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_zero_capacity_invalid() {
        let mut config = IntentionStackConfig::default();
        config.max_stack_capacity = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_revert_depth_exceeds_stack_depth() {
        let mut config = IntentionStackConfig::default();
        config.max_revert_depth = 30;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_visibility_threshold() {
        let mut config = IntentionStackConfig::default();
        config.root_visibility_threshold = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_zero_checkpoints_invalid() {
        let mut config = IntentionStackConfig::default();
        config.max_checkpoints_per_layer = 0;
        assert!(config.validate().is_err());
    }
}
