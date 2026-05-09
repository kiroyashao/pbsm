//! M4 外部记忆存储配置模块
//!
//! 提供记忆存储的配置参数定义、默认值及校验逻辑。
//! 基于 HLD-M4 附录 A 的配置规范。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::types::CompressionType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryConfig {
    pub storage_path: PathBuf,
    pub cache_size: usize,
    pub max_log_age_days: u32,
    pub compression_type: CompressionType,
    pub max_recent_sessions: u32,
    pub base_confidence_threshold: f64,
    pub cleanup_auto_trigger_threshold: f64,
    pub retrieval_default_limit: usize,
    pub importance_retention_bonus: f64,
    pub archive_threshold_days: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from("./data/memory"),
            cache_size: 100,
            max_log_age_days: 90,
            compression_type: CompressionType::Lz4,
            max_recent_sessions: 30,
            base_confidence_threshold: 0.4,
            cleanup_auto_trigger_threshold: 0.85,
            retrieval_default_limit: 20,
            importance_retention_bonus: 1.5,
            archive_threshold_days: 30,
        }
    }
}

impl MemoryConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.cache_size == 0 {
            return Err("cache_size must be greater than 0".to_string());
        }
        if self.max_log_age_days == 0 {
            return Err("max_log_age_days must be greater than 0".to_string());
        }
        if !(0.1..=0.9).contains(&self.base_confidence_threshold) {
            return Err(format!(
                "base_confidence_threshold must be in range [0.1, 0.9], got {}",
                self.base_confidence_threshold
            ));
        }
        if !(0.5..=0.99).contains(&self.cleanup_auto_trigger_threshold) {
            return Err(format!(
                "cleanup_auto_trigger_threshold must be in range [0.5, 0.99], got {}",
                self.cleanup_auto_trigger_threshold
            ));
        }
        if self.retrieval_default_limit == 0 {
            return Err("retrieval_default_limit must be greater than 0".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_storage_path() {
        let config = MemoryConfig::default();
        assert_eq!(config.storage_path, PathBuf::from("./data/memory"));
    }

    #[test]
    fn test_default_cache_size() {
        let config = MemoryConfig::default();
        assert_eq!(config.cache_size, 100);
    }

    #[test]
    fn test_default_max_log_age_days() {
        let config = MemoryConfig::default();
        assert_eq!(config.max_log_age_days, 90);
    }

    #[test]
    fn test_default_compression_type() {
        let config = MemoryConfig::default();
        assert_eq!(config.compression_type, CompressionType::Lz4);
    }

    #[test]
    fn test_default_max_recent_sessions() {
        let config = MemoryConfig::default();
        assert_eq!(config.max_recent_sessions, 30);
    }

    #[test]
    fn test_default_base_confidence_threshold() {
        let config = MemoryConfig::default();
        assert!((config.base_confidence_threshold - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_cleanup_auto_trigger_threshold() {
        let config = MemoryConfig::default();
        assert!((config.cleanup_auto_trigger_threshold - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_retrieval_default_limit() {
        let config = MemoryConfig::default();
        assert_eq!(config.retrieval_default_limit, 20);
    }

    #[test]
    fn test_default_importance_retention_bonus() {
        let config = MemoryConfig::default();
        assert!((config.importance_retention_bonus - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_archive_threshold_days() {
        let config = MemoryConfig::default();
        assert_eq!(config.archive_threshold_days, 30);
    }

    #[test]
    fn test_default_config_is_valid() {
        let config = MemoryConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_cache_size_zero() {
        let mut config = MemoryConfig::default();
        config.cache_size = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cache_size"));
    }

    #[test]
    fn test_validate_max_log_age_days_zero() {
        let mut config = MemoryConfig::default();
        config.max_log_age_days = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("max_log_age_days"));
    }

    #[test]
    fn test_validate_base_confidence_threshold_below_range() {
        let mut config = MemoryConfig::default();
        config.base_confidence_threshold = 0.05;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("base_confidence_threshold"));
    }

    #[test]
    fn test_validate_base_confidence_threshold_above_range() {
        let mut config = MemoryConfig::default();
        config.base_confidence_threshold = 0.95;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("base_confidence_threshold"));
    }

    #[test]
    fn test_validate_base_confidence_threshold_at_boundaries() {
        let mut config = MemoryConfig::default();
        config.base_confidence_threshold = 0.1;
        assert!(config.validate().is_ok());
        config.base_confidence_threshold = 0.9;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_cleanup_auto_trigger_threshold_below_range() {
        let mut config = MemoryConfig::default();
        config.cleanup_auto_trigger_threshold = 0.4;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("cleanup_auto_trigger_threshold"));
    }

    #[test]
    fn test_validate_cleanup_auto_trigger_threshold_above_range() {
        let mut config = MemoryConfig::default();
        config.cleanup_auto_trigger_threshold = 1.0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("cleanup_auto_trigger_threshold"));
    }

    #[test]
    fn test_validate_cleanup_auto_trigger_threshold_at_boundaries() {
        let mut config = MemoryConfig::default();
        config.cleanup_auto_trigger_threshold = 0.5;
        assert!(config.validate().is_ok());
        config.cleanup_auto_trigger_threshold = 0.99;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_retrieval_default_limit_zero() {
        let mut config = MemoryConfig::default();
        config.retrieval_default_limit = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("retrieval_default_limit"));
    }
}
