//! 预测引擎核心模块
//!
//! 本模块是预测引擎（M2）的顶层入口。
//!
//! # 架构概览
//!
//! 预测引擎由以下核心组件构成：
//! - **PredictionGenerator**：预测生成器，负责创建预测实例
//! - **PredictionVerifier**：预测验证器，负责验证预测并计算残差
//! - **PredictionStateMachine**：状态机，管理预测生命周期
//! - **ResidualCalculator**：残差计算器，执行多维度残差计算
//!
//! # 使用流程
//!
//! 1. 通过 `create_prediction` 创建预测
//! 2. 执行关联的动作
//! 3. 通过 `verify_prediction` 验证观测结果
//! 4. 系统自动更新状态并触发事件通知

pub mod generator;
pub mod pool;
pub mod residual_calculator;
pub mod state_machine;
pub mod verifier;

pub use generator::*;
pub use pool::*;
pub use residual_calculator::*;
pub use state_machine::*;
pub use verifier::*;

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::PredictionError;
use crate::modules::common::{
    BeliefGraphReader, EventPublisher, NullBeliefGraphReader, NullEventPublisher, PredictionEvent,
};
use crate::types::filter::{PredictionFilter, PredictionList, PredictionStatistics};
use crate::types::prediction::{ActionRequest, Observation, Prediction, PredictionState};

/// 预测引擎主结构体
///
/// # 设计说明
///
/// 预测引擎是整个预测系统的核心，负责：
/// - 维护所有预测实例的存储
/// - 协调生成器和验证器的工作
/// - 提供统一的预测管理接口
/// - 发布预测相关事件
#[allow(dead_code)]
pub struct PredictionEngine {
    predictions: RwLock<HashMap<String, Prediction>>,
    generator: PredictionGenerator,
    verifier: PredictionVerifier,
    belief_graph: Arc<dyn BeliefGraphReader>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl PredictionEngine {
    /// 创建新的预测引擎实例，使用默认组件
    ///
    /// # 返回
    /// * 预测引擎实例
    pub fn new() -> Self {
        Self {
            predictions: RwLock::new(HashMap::new()),
            generator: PredictionGenerator::default(),
            verifier: PredictionVerifier::new(),
            belief_graph: Arc::new(NullBeliefGraphReader),
            event_publisher: Arc::new(NullEventPublisher),
        }
    }

    /// 使用指定的信念图和事件发布器创建预测引擎
    ///
    /// # 参数
    /// * `belief_graph` - 信念图读取接口
    /// * `event_publisher` - 事件发布器
    ///
    /// # 返回
    /// * 配置好的预测引擎实例
    pub fn with_components(
        belief_graph: Arc<dyn BeliefGraphReader>,
        event_publisher: Arc<dyn EventPublisher>,
    ) -> Self {
        Self {
            predictions: RwLock::new(HashMap::new()),
            generator: PredictionGenerator::with_defaults(belief_graph.clone()),
            verifier: PredictionVerifier::new(),
            belief_graph,
            event_publisher,
        }
    }

    /// 创建新预测
    ///
    /// # 参数
    /// * `action_request` - 动作请求
    /// * `context_hint` - 上下文检索提示（可选）
    ///
    /// # 返回
    /// * `Ok(Prediction)` - 创建的预测实例
    /// * `Err(PredictionError)` - 创建失败
    ///
    /// # 处理流程
    ///
    /// 1. 调用生成器创建预测实例
    /// 2. 将预测存储到内部映射表
    /// 3. 发布 PredictionCreated 事件
    pub async fn create_prediction(
        &self,
        action_request: ActionRequest,
        context_hint: Option<crate::modules::prediction_engine::ContextHint>,
    ) -> Result<Prediction, PredictionError> {
        let prediction = self
            .generator
            .create_prediction(action_request, context_hint)
            .await?;

        let id = prediction.prediction_id.to_string();
        self.predictions.write().insert(id, prediction.clone());

        let _ = self
            .event_publisher
            .publish_event(PredictionEvent::PredictionCreated(
                crate::modules::common::PredictionCreatedPayload {
                    prediction_id: prediction.prediction_id,
                    action_type: prediction.associated_action.action_type,
                    target_node: prediction.associated_action.target_node.clone(),
                    expected_change_count: prediction.expected_changes.len(),
                },
            ));

        Ok(prediction)
    }

    /// 验证预测
    ///
    /// # 参数
    /// * `prediction_id` - 预测ID
    /// * `observation` - 实际观测结果
    ///
    /// # 返回
    /// * `Ok(VerificationResult)` - 验证结果
    /// * `Err(PredictionError)` - 验证失败
    ///
    /// # 处理流程
    ///
    /// 1. 从存储中获取预测实例
    /// 2. 调用验证器执行验证
    /// 3. 根据新状态发布相应事件（Verified/Falsified）
    pub async fn verify_prediction(
        &self,
        prediction_id: &str,
        observation: Observation,
    ) -> Result<crate::types::prediction::VerificationResult, PredictionError> {
        let mut predictions = self.predictions.write();
        let prediction = predictions
            .get_mut(prediction_id)
            .ok_or_else(|| PredictionError::NotFound(prediction_id.to_string()))?;

        let result = self.verifier.verify_prediction(prediction, observation)?;

        let event = match prediction.status {
            PredictionState::Verified => Some(PredictionEvent::PredictionVerified(
                crate::modules::common::PredictionVerifiedPayload {
                    prediction_id: prediction.prediction_id,
                    match_level: result.match_level,
                    confidence_delta: 0.05,
                },
            )),
            PredictionState::Falsified => Some(PredictionEvent::PredictionFalsified(
                crate::modules::common::PredictionFalsifiedPayload {
                    prediction_id: prediction.prediction_id,
                    match_level: result.match_level,
                    severity: format!("{:?}", result.residual.severity_assessment.level),
                    overall_degree: result.residual.overall_degree,
                    affected_beliefs: result.affected_beliefs.clone(),
                },
            )),
            _ => None,
        };

        if let Some(e) = event {
            let _ = self.event_publisher.publish_event(e);
        }

        Ok(result)
    }

    /// 根据ID获取预测
    ///
    /// # 参数
    /// * `prediction_id` - 预测ID
    /// * `_include_history` - 是否包含历史（未使用）
    ///
    /// # 返回
    /// * `Ok(Prediction)` - 预测实例
    /// * `Err(PredictionError)` - 预测不存在
    pub fn get_prediction_by_id(
        &self,
        prediction_id: &str,
        _include_history: Option<bool>,
    ) -> Result<Prediction, PredictionError> {
        self.predictions
            .read()
            .get(prediction_id)
            .cloned()
            .ok_or_else(|| PredictionError::NotFound(prediction_id.to_string()))
    }

    /// 获取活跃预测列表
    ///
    /// # 参数
    /// * `filter` - 过滤条件（可选）
    ///
    /// # 返回
    /// * PredictionList 包含匹配预测的列表和统计信息
    pub fn get_active_predictions(&self, filter: Option<PredictionFilter>) -> PredictionList {
        let filter = filter.unwrap_or_default();
        let predictions = self.predictions.read();

        let mut matching: Vec<Prediction> = predictions
            .values()
            .filter(|p| filter.matches(p))
            .cloned()
            .collect();

        matching.sort_by_key(|p| std::cmp::Reverse(p.metadata.created_at));

        let total = matching.len();
        let has_more = filter.limit.map(|l| total > l).unwrap_or(false);

        if let Some(limit) = filter.limit {
            matching.truncate(limit);
        }

        PredictionList {
            predictions: matching,
            total,
            has_more,
        }
    }

    /// 取消预测
    ///
    /// # 参数
    /// * `prediction_id` - 预测ID
    /// * `reason` - 取消原因
    ///
    /// # 返回
    /// * `Ok(true)` - 取消成功
    /// * `Err(PredictionError)` - 取消失败
    pub fn cancel_prediction(
        &self,
        prediction_id: &str,
        reason: crate::types::filter::CancellationReason,
    ) -> Result<bool, PredictionError> {
        let mut predictions = self.predictions.write();
        let prediction = predictions
            .get_mut(prediction_id)
            .ok_or_else(|| PredictionError::NotFound(prediction_id.to_string()))?;

        self.verifier
            .cancel_prediction(prediction, &format!("{:?}", reason))?;

        Ok(true)
    }

    /// 获取预测统计信息
    ///
    /// # 参数
    /// * `_time_range` - 时间范围过滤（未使用）
    ///
    /// # 返回
    /// * PredictionStatistics 统计信息结构体
    pub fn get_prediction_statistics(
        &self,
        _time_range: Option<crate::types::filter::TimeRange>,
    ) -> PredictionStatistics {
        let predictions = self.predictions.read();

        let total = predictions.len() as u64;
        let mut by_status: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        let mut by_match_level: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        let mut by_severity: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();

        let mut total_residual = 0.0;
        let mut verified_count = 0u64;
        let mut falsified_count = 0u64;

        for pred in predictions.values() {
            *by_status.entry(format!("{:?}", pred.status)).or_insert(0) += 1;

            if let Some(ref residual) = pred.residuals {
                total_residual += residual.overall_degree.abs();
                *by_match_level
                    .entry(format!("{:?}", residual.match_level))
                    .or_insert(0) += 1;
                *by_severity
                    .entry(format!("{:?}", residual.severity_assessment.level))
                    .or_insert(0) += 1;

                match pred.status {
                    PredictionState::Verified => verified_count += 1,
                    PredictionState::Falsified => falsified_count += 1,
                    _ => {}
                }
            }
        }

        let non_pending = predictions
            .values()
            .filter(|p| p.status != PredictionState::Pending)
            .count() as f64;

        PredictionStatistics {
            total,
            by_status,
            by_match_level,
            by_severity,
            average_residual: if non_pending > 0.0 {
                total_residual / non_pending
            } else {
                0.0
            },
            verification_rate: if non_pending > 0.0 {
                verified_count as f64 / non_pending
            } else {
                0.0
            },
            falsification_rate: if non_pending > 0.0 {
                falsified_count as f64 / non_pending
            } else {
                0.0
            },
            average_latency_ms: 0.0,
            top_error_patterns: Vec::new(),
        }
    }

    /// 获取待处理预测数量
    pub fn get_pending_count(&self) -> usize {
        self.predictions
            .read()
            .values()
            .filter(|p| p.status == PredictionState::Pending)
            .count()
    }

    /// 清理已过期的预测
    ///
    /// # 返回
    /// * 被清理的预测数量
    pub fn cleanup_expired(&self) -> usize {
        let mut predictions = self.predictions.write();
        let mut removed = 0;

        for pred in predictions.values_mut() {
            if pred.status == PredictionState::Pending && pred.validity_window.is_expired() {
                let _ = pred.transition_to(PredictionState::Expired, "Validity window expired");
                removed += 1;
            }
        }

        removed
    }
}

impl Default for PredictionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::filter::{CancellationReason, PredictionFilter};
    use crate::types::prediction::{ActionType, PredictionState};

    #[tokio::test]
    async fn test_create_prediction() {
        let engine = PredictionEngine::new();

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({}),
            target_id: Some("node-1".to_string()),
        };

        let result = engine.create_prediction(action, None).await;
        assert!(result.is_ok());

        let prediction = result.unwrap();
        assert_eq!(prediction.status, PredictionState::Pending);
    }

    #[tokio::test]
    async fn test_get_prediction_by_id() {
        let engine = PredictionEngine::new();

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({}),
            target_id: None,
        };

        let created = engine.create_prediction(action, None).await.unwrap();
        let id = created.prediction_id.to_string();

        let retrieved = engine.get_prediction_by_id(&id, None);
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap().prediction_id.to_string(), id);
    }

    #[tokio::test]
    async fn test_get_prediction_not_found() {
        let engine = PredictionEngine::new();
        let result = engine.get_prediction_by_id("non-existent", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_active_predictions() {
        let engine = PredictionEngine::new();

        let list = engine.get_active_predictions(None);
        assert!(list.predictions.is_empty());
        assert_eq!(list.total, 0);
    }

    #[test]
    fn test_statistics() {
        let engine = PredictionEngine::new();
        let stats = engine.get_prediction_statistics(None);
        assert_eq!(stats.total, 0);
    }

    #[tokio::test]
    async fn test_cancel_prediction() {
        let engine = PredictionEngine::new();

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({}),
            target_id: None,
        };

        let created = engine.create_prediction(action, None).await.unwrap();
        let id = created.prediction_id.to_string();

        let result = engine.cancel_prediction(&id, CancellationReason::UserRequest);
        assert!(result.is_ok());
    }
}
