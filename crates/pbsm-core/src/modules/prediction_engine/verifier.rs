//! 预测验证器实现
//!
//! 本模块实现了预测验证的核心逻辑。
//! 预测验证器负责将实际观测与预测进行比对，计算残差并触发相应的状态转换。
//!
//! # 核心职责
//!
//! - 验证预测与观测是否匹配
//! - 调用残差计算器计算预测残差
//! - 根据残差结果决定下一步动作
//! - 管理预测的生命周期（取消操作）

use super::residual_calculator::ResidualCalculator;
use crate::error::PredictionError;
use crate::types::prediction::{NextAction, Observation, Prediction, VerificationResult};
use crate::types::residual::{MatchLevel, SeverityLevel};

/// 预测验证器结构体，负责验证预测并计算残差
///
/// # 设计说明
///
/// 验证器是预测引擎的核心组件之一，负责：
/// - 调用残差计算器进行多维度残差计算
/// - 根据残差结果更新预测状态
/// - 返回验证结果和下一步建议
pub struct PredictionVerifier {
    calculator: ResidualCalculator,
    confidence_increment: f64,
    confidence_decrement: f64,
}

impl PredictionVerifier {
    /// 创建新的验证器实例
    pub fn new() -> Self {
        Self {
            calculator: ResidualCalculator::new(),
            confidence_increment: 0.05,
            confidence_decrement: 0.15,
        }
    }

    /// 使用指定的残差计算器创建验证器
    ///
    /// # 参数
    /// * `calculator` - 残差计算器实例
    pub fn with_calculator(calculator: ResidualCalculator) -> Self {
        Self {
            calculator,
            confidence_increment: 0.05,
            confidence_decrement: 0.15,
        }
    }

    /// 验证预测
    ///
    /// # 参数
    /// * `prediction` - 待验证的预测（可变引用，会被更新）
    /// * `observation` - 实际观测结果
    ///
    /// # 返回
    /// * `Ok(VerificationResult)` - 验证结果
    /// * `Err(PredictionError)` - 验证失败
    ///
    /// # 处理流程
    ///
    /// 1. 检查预测状态是否为终态
    /// 2. 检查预测是否已过期
    /// 3. 调用残差计算器计算残差
    /// 4. 根据残差结果决定下一步动作
    /// 5. 更新预测状态
    pub fn verify_prediction(
        &self,
        prediction: &mut Prediction,
        observation: Observation,
    ) -> Result<VerificationResult, PredictionError> {
        if prediction.status.is_terminal() {
            return Err(PredictionError::AlreadyVerified);
        }

        if prediction.validity_window.is_expired() {
            prediction.status = crate::types::prediction::PredictionState::Expired;
            return Err(PredictionError::Expired);
        }

        let residual = self.calculator.compute_residual(
            prediction.prediction_id,
            &prediction.expected_changes,
            &observation,
        );

        prediction.residuals = Some(residual.clone());
        let next_action = self.determine_next_action(&residual);

        let new_state = match residual.match_level {
            MatchLevel::Exact => crate::types::prediction::PredictionState::Verified,
            MatchLevel::Partial => {
                if residual.severity_assessment.level == SeverityLevel::Warning {
                    crate::types::prediction::PredictionState::Verified
                } else {
                    crate::types::prediction::PredictionState::Falsified
                }
            }
            MatchLevel::Mismatch => crate::types::prediction::PredictionState::Falsified,
        };

        prediction.transition_to(new_state, "Verification completed")?;

        Ok(VerificationResult {
            prediction_id: prediction.prediction_id,
            match_level: residual.match_level,
            residual,
            affected_beliefs: Vec::new(),
            next_action,
        })
    }

    /// 根据残差确定下一步动作
    ///
    /// # 参数
    /// * `residual` - 残差结果
    ///
    /// # 返回
    /// * NextAction 枚举值
    ///
    /// # 决策规则
    ///
    /// - SeverityLevel::None/Warning → Log
    /// - SeverityLevel::Error → Revise
    /// - SeverityLevel::Critical → Rollback
    fn determine_next_action(&self, residual: &crate::types::residual::Residual) -> NextAction {
        match residual.severity_assessment.level {
            SeverityLevel::None | SeverityLevel::Warning => NextAction::Log,
            SeverityLevel::Error => NextAction::Revise,
            SeverityLevel::Critical => NextAction::Rollback,
        }
    }

    /// 取消预测
    ///
    /// # 参数
    /// * `prediction` - 待取消的预测
    /// * `reason` - 取消原因
    ///
    /// # 返回
    /// * `Ok(())` - 取消成功
    /// * `Err(PredictionError)` - 取消失败（状态不合法）
    pub fn cancel_prediction(
        &self,
        prediction: &mut Prediction,
        reason: &str,
    ) -> Result<(), PredictionError> {
        if prediction.status.is_terminal() {
            return Err(PredictionError::AlreadyVerified);
        }

        prediction.transition_to(crate::types::prediction::PredictionState::Cancelled, reason)?;
        Ok(())
    }

    /// 根据匹配级别和严重程度获取置信度变化量
    ///
    /// # 参数
    /// * `match_level` - 匹配级别
    /// * `severity` - 严重程度
    ///
    /// # 返回
    /// * 置信度变化量（正值表示提升，负值表示下降）
    pub fn get_confidence_delta(&self, match_level: MatchLevel, severity: SeverityLevel) -> f64 {
        match (match_level, severity) {
            (MatchLevel::Exact, _) => self.confidence_increment,
            (MatchLevel::Partial, SeverityLevel::None)
            | (MatchLevel::Partial, SeverityLevel::Warning) => 0.0,
            (MatchLevel::Partial, SeverityLevel::Error) => -self.confidence_decrement * 0.5,
            (MatchLevel::Partial, SeverityLevel::Critical) => -self.confidence_decrement,
            (MatchLevel::Mismatch, _) => -self.confidence_decrement,
        }
    }
}

impl Default for PredictionVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::prediction::{
        ActionRequest, ActionType, ChangeType, Prediction, PredictionState,
    };
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_prediction() -> Prediction {
        let mut prediction = Prediction::new();
        prediction.prediction_id = Uuid::new_v4();
        prediction.status = PredictionState::Pending;
        prediction.expected_changes = vec![crate::types::prediction::ExpectedChange {
            change_id: Uuid::new_v4(),
            node_id: "test-node".to_string(),
            attribute: Some("status".to_string()),
            expected_value: serde_json::json!("success"),
            previous_value: serde_json::json!("pending"),
            change_type: ChangeType::Modify,
            expected_confidence: 0.9,
            derivation_path: vec![],
        }];
        prediction
    }

    #[test]
    fn test_verify_exact_match() {
        let verifier = PredictionVerifier::new();
        let mut prediction = create_test_prediction();

        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "success"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let result = verifier.verify_prediction(&mut prediction, observation);
        assert!(result.is_ok());

        let verification = result.unwrap();
        assert!(verification.residual.overall_degree < 0.3);
        assert!(
            prediction.status == PredictionState::Verified
                || prediction.status == PredictionState::Pending
        );
    }

    #[test]
    fn test_verify_mismatch() {
        let verifier = PredictionVerifier::new();
        let mut prediction = create_test_prediction();

        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "failed"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let result = verifier.verify_prediction(&mut prediction, observation);
        assert!(result.is_ok());

        let verification = result.unwrap();
        assert_eq!(verification.match_level, MatchLevel::Mismatch);
        assert_eq!(prediction.status, PredictionState::Falsified);
    }

    #[test]
    fn test_verify_already_verified() {
        let verifier = PredictionVerifier::new();
        let mut prediction = create_test_prediction();
        prediction.status = PredictionState::Verified;

        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "success"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let result = verifier.verify_prediction(&mut prediction, observation);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PredictionError::AlreadyVerified
        ));
    }

    #[test]
    fn test_cancel_prediction() {
        let verifier = PredictionVerifier::new();
        let mut prediction = create_test_prediction();

        let result = verifier.cancel_prediction(&mut prediction, "User cancelled");
        assert!(result.is_ok());
        assert_eq!(prediction.status, PredictionState::Cancelled);
    }

    #[test]
    fn test_next_action_determination() {
        let verifier = PredictionVerifier::new();

        let residual = crate::types::residual::Residual::new(Uuid::new_v4(), Utc::now());
        assert_eq!(verifier.determine_next_action(&residual), NextAction::Log);
    }
}
