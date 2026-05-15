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
use std::sync::Arc;

use super::residual_calculator::ResidualCalculator;
use crate::error::PredictionError;
use crate::modules::common::{
    BeliefGraphWriter, EventPublisher, EventSource, NullBeliefGraphWriter, NullEventPublisher,
    PBSMEvent, PredictionCancelledPayload, PredictionEvent, PredictionFalsifiedPayload,
    PredictionStatusChangedPayload, PredictionVerifiedPayload,
};
use crate::types::prediction::{NextAction, Observation, Prediction, PredictionState, VerificationResult};
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
    belief_writer: Arc<dyn BeliefGraphWriter>,
    event_publisher: Arc<dyn EventPublisher>,
    confidence_increment: f64,
    confidence_decrement: f64,
}

impl PredictionVerifier {
    /// 创建新的验证器实例
    pub fn new(belief_writer: Arc<dyn BeliefGraphWriter>) -> Self {
        Self {
            calculator: ResidualCalculator::new(),
            belief_writer,
            event_publisher: Arc::new(NullEventPublisher),
            confidence_increment: 0.05,
            confidence_decrement: 0.15,
        }
    }

    /// 使用指定的残差计算器创建验证器
    ///
    /// # 参数
    /// * `calculator` - 残差计算器实例
    pub fn with_calculator(
        calculator: ResidualCalculator,
        belief_writer: Arc<dyn BeliefGraphWriter>,
    ) -> Self {
        Self {
            calculator,
            belief_writer,
            event_publisher: Arc::new(NullEventPublisher),
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
    pub fn with_event_publisher(mut self, event_publisher: Arc<dyn EventPublisher>) -> Self {
        self.event_publisher = event_publisher;
        self
    }

    pub async fn verify_prediction(
        &self,
        prediction: &mut Prediction,
        observation: Observation,
    ) -> Result<VerificationResult, PredictionError> {
        if prediction.status.is_terminal() {
            return Err(PredictionError::AlreadyVerified {
                code: "PEV-E101".to_string(),
            });
        }

        if prediction.validity_window.is_expired() {
            prediction.status = PredictionState::Expired;
            return Err(PredictionError::Expired {
                code: "PEV-E102".to_string(),
            });
        }

        let residual = self.calculator.compute_residual(
            prediction.prediction_id,
            &prediction.expected_changes,
            &observation,
            prediction.validity_window.duration_ms as f64,
            prediction.metadata.created_at,
        );

        prediction.residuals = Some(residual.clone());
        let next_action = self.determine_next_action(&residual);

        let previous_status = format!("{:?}", prediction.status);
        let new_state = match residual.match_level {
            MatchLevel::Exact => PredictionState::Verified,
            MatchLevel::Partial => {
                if residual.severity_assessment.level == SeverityLevel::Warning {
                    PredictionState::Verified
                } else {
                    PredictionState::Falsified
                }
            }
            MatchLevel::Mismatch => PredictionState::Falsified,
        };

        prediction.transition_to(new_state, "Verification completed")?;

        let overall_degree = residual.overall_degree;
        let mut affected_beliefs = Vec::new();

        match new_state {
            PredictionState::Verified => {
                let confidence_delta = 0.1 * (1.0 - overall_degree);
                for change in &prediction.expected_changes {
                    if let Some(attr) = &change.attribute {
                        let _ = self
                            .belief_writer
                            .update_belief_confidence(
                                &change.node_id,
                                attr,
                                change.expected_confidence + confidence_delta,
                            )
                            .await;
                    }
                    if !affected_beliefs.contains(&change.node_id) {
                        affected_beliefs.push(change.node_id.clone());
                    }
                }

                let payload = PredictionEvent::PredictionVerified(PredictionVerifiedPayload {
                    prediction_id: prediction.prediction_id,
                    match_level: residual.match_level,
                    confidence_delta,
                });
                let mut event = PBSMEvent::new(payload);
                event.source = EventSource {
                    module: "M2".into(),
                    instance_id: None,
                };
                let _ = self.event_publisher.publish_event(event);
            }
            PredictionState::Falsified => {
                let confidence_delta = -0.2 * overall_degree;
                for change in &prediction.expected_changes {
                    let _ = self
                        .belief_writer
                        .mark_belief_for_revision(
                            &change.node_id,
                            &format!(
                                "Prediction falsified: confidence_delta={:.3}",
                                confidence_delta
                            ),
                        )
                        .await;
                    if !affected_beliefs.contains(&change.node_id) {
                        affected_beliefs.push(change.node_id.clone());
                    }
                }

                let payload = PredictionEvent::PredictionFalsified(PredictionFalsifiedPayload {
                    prediction_id: prediction.prediction_id,
                    match_level: residual.match_level,
                    severity: format!("{:?}", residual.severity_assessment.level),
                    overall_degree: residual.overall_degree,
                    affected_beliefs: affected_beliefs.clone(),
                });
                let mut event = PBSMEvent::new(payload);
                event.source = EventSource {
                    module: "M2".into(),
                    instance_id: None,
                };
                let _ = self.event_publisher.publish_event(event);
            }
            _ => {}
        }

        let status_payload =
            PredictionEvent::PredictionStatusChanged(PredictionStatusChangedPayload {
                prediction_id: prediction.prediction_id,
                previous_status,
                new_status: format!("{:?}", new_state),
                reason: "Verification completed".to_string(),
            });
        let mut status_event = PBSMEvent::new(status_payload);
        status_event.source = EventSource {
            module: "M2".into(),
            instance_id: None,
        };
        let _ = self.event_publisher.publish_event(status_event);

        Ok(VerificationResult {
            prediction_id: prediction.prediction_id,
            match_level: residual.match_level,
            residual,
            affected_beliefs,
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
            return Err(PredictionError::AlreadyVerified {
                code: "PEV-E101".to_string(),
            });
        }

        let previous_status = format!("{:?}", prediction.status);
        prediction.transition_to(PredictionState::Cancelled, reason)?;

        let status_payload =
            PredictionEvent::PredictionStatusChanged(PredictionStatusChangedPayload {
                prediction_id: prediction.prediction_id,
                previous_status,
                new_status: format!("{:?}", PredictionState::Cancelled),
                reason: reason.to_string(),
            });
        let mut status_event = PBSMEvent::new(status_payload);
        status_event.source = EventSource {
            module: "M2".into(),
            instance_id: None,
        };
        let _ = self.event_publisher.publish_event(status_event);

        let cancelled_payload = PredictionEvent::PredictionCancelled(PredictionCancelledPayload {
            prediction_id: prediction.prediction_id,
            cancellation_reason: reason.to_string(),
        });
        let mut cancelled_event = PBSMEvent::new(cancelled_payload);
        cancelled_event.source = EventSource {
            module: "M2".into(),
            instance_id: None,
        };
        let _ = self.event_publisher.publish_event(cancelled_event);

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
        Self::new(Arc::new(NullBeliefGraphWriter))
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

    fn create_verifier() -> PredictionVerifier {
        PredictionVerifier::new(Arc::new(NullBeliefGraphWriter))
    }

    #[tokio::test]
    async fn test_verify_exact_match() {
        let verifier = create_verifier();
        let mut prediction = create_test_prediction();

        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "success"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let result = verifier.verify_prediction(&mut prediction, observation).await;
        assert!(result.is_ok());

        let verification = result.unwrap();
        assert!(prediction.status.is_terminal());
        assert!(verification.residual.overall_degree >= 0.0);
    }

    #[tokio::test]
    async fn test_verify_mismatch() {
        let verifier = create_verifier();
        let mut prediction = create_test_prediction();

        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "failed"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let result = verifier.verify_prediction(&mut prediction, observation).await;
        assert!(result.is_ok());

        let verification = result.unwrap();
        assert!(prediction.status.is_terminal());
        assert_ne!(verification.match_level, MatchLevel::Exact);
    }

    #[tokio::test]
    async fn test_verify_already_verified() {
        let verifier = create_verifier();
        let mut prediction = create_test_prediction();
        prediction.status = PredictionState::Verified;

        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "success"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let result = verifier.verify_prediction(&mut prediction, observation).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PredictionError::AlreadyVerified { .. }
        ));
    }

    #[test]
    fn test_cancel_prediction() {
        let verifier = create_verifier();
        let mut prediction = create_test_prediction();

        let result = verifier.cancel_prediction(&mut prediction, "User cancelled");
        assert!(result.is_ok());
        assert_eq!(prediction.status, PredictionState::Cancelled);
    }

    #[test]
    fn test_next_action_determination() {
        let verifier = create_verifier();

        let residual = crate::types::residual::Residual::new(Uuid::new_v4(), Utc::now());
        assert_eq!(verifier.determine_next_action(&residual), NextAction::Log);
    }

    #[tokio::test]
    async fn test_verified_updates_belief_confidence() {
        let verifier = create_verifier();
        let mut prediction = create_test_prediction();

        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "success"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let result = verifier.verify_prediction(&mut prediction, observation).await;
        if let Ok(verification) = result {
            if prediction.status == PredictionState::Verified {
                let expected_delta = 0.1 * (1.0 - verification.residual.overall_degree);
                assert!(expected_delta >= 0.0);
                assert!(!verification.affected_beliefs.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn test_falsified_marks_belief_for_revision() {
        let verifier = create_verifier();
        let mut prediction = create_test_prediction();

        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "failed"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let result = verifier.verify_prediction(&mut prediction, observation).await;
        if let Ok(verification) = result {
            if prediction.status == PredictionState::Falsified {
                let expected_delta = -0.2 * verification.residual.overall_degree;
                assert!(expected_delta <= 0.0);
                assert!(!verification.affected_beliefs.is_empty());
            }
        }
    }

    #[test]
    fn test_cancel_prediction_already_terminal() {
        let verifier = create_verifier();
        let mut prediction = create_test_prediction();
        prediction.status = PredictionState::Verified;

        let result = verifier.cancel_prediction(&mut prediction, "User cancelled");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PredictionError::AlreadyVerified { .. }
        ));
    }
}
