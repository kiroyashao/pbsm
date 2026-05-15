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
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

use crate::error::PredictionError;
use crate::modules::common::{
    AttentionStatusReader, BeliefGraphReader, BeliefGraphWriter, EventPublisher, EventSource,
    NullBeliefGraphReader, NullBeliefGraphWriter, NullEventPublisher, PBSMEvent,
    PredictionCreatedPayload, PredictionEvent, PredictionExpiredPayload,
    PredictionFalsifiedPayload, PredictionVerifiedPayload, SubscriptionError,
};
use crate::types::filter::{
    ErrorPattern, PredictionFilter, PredictionList, PredictionStatistics, ResidualHistory,
    ResidualTrend, TimeRange, TrendDirection,
};
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
#[derive(Debug, Clone)]
pub struct PredictionEngineConfig {
    pub max_active_predictions: usize,
    pub min_confidence_threshold: f64,
    pub default_validity_duration_ms: i64,
    pub max_validity_duration_ms: i64,
    pub warning_threshold: f64,
    pub error_threshold: f64,
    pub critical_threshold: f64,
    pub tolerance_margin: f64,
    pub enable_cascading_effects: bool,
    pub max_cascade_depth: u32,
    pub context_completeness_threshold: f64,
    pub step_expiry_enabled: bool,
    pub event_driven_expiry_enabled: bool,
}

impl Default for PredictionEngineConfig {
    fn default() -> Self {
        Self {
            max_active_predictions: 1000,
            min_confidence_threshold: 0.3,
            default_validity_duration_ms: 30000,
            max_validity_duration_ms: 300000,
            warning_threshold: 0.3,
            error_threshold: 0.7,
            critical_threshold: 1.0,
            tolerance_margin: 0.05,
            enable_cascading_effects: true,
            max_cascade_depth: 3,
            context_completeness_threshold: 0.5,
            step_expiry_enabled: true,
            event_driven_expiry_enabled: true,
        }
    }
}

pub struct PredictionEngine {
    predictions: RwLock<HashMap<String, Prediction>>,
    generator: PredictionGenerator,
    verifier: PredictionVerifier,
    belief_graph: Arc<dyn BeliefGraphReader>,
    belief_writer: Arc<dyn BeliefGraphWriter>,
    event_publisher: Arc<dyn EventPublisher>,
    config: PredictionEngineConfig,
    subscribers: RwLock<HashMap<String, Vec<Box<dyn Fn(PBSMEvent) + Send + Sync>>>>,
    attention_reader: Option<Arc<dyn AttentionStatusReader>>,
    current_step: Arc<AtomicU64>,
}

impl PredictionEngine {
    /// 创建新的预测引擎实例，使用默认组件
    ///
    /// # 返回
    /// * 预测引擎实例
    pub fn new() -> Self {
        let belief_writer: Arc<dyn BeliefGraphWriter> = Arc::new(NullBeliefGraphWriter);
        Self {
            predictions: RwLock::new(HashMap::new()),
            generator: PredictionGenerator::default(),
            verifier: PredictionVerifier::new(belief_writer.clone()),
            belief_graph: Arc::new(NullBeliefGraphReader),
            belief_writer,
            event_publisher: Arc::new(NullEventPublisher),
            config: PredictionEngineConfig::default(),
            subscribers: RwLock::new(HashMap::new()),
            attention_reader: None,
            current_step: Arc::new(AtomicU64::new(0)),
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
        let belief_writer: Arc<dyn BeliefGraphWriter> = Arc::new(NullBeliefGraphWriter);
        let generator = PredictionGenerator::with_defaults(belief_graph.clone())
            .with_event_publisher(event_publisher.clone());
        Self {
            predictions: RwLock::new(HashMap::new()),
            generator,
            verifier: PredictionVerifier::new(belief_writer.clone()),
            belief_graph,
            belief_writer,
            event_publisher,
            config: PredictionEngineConfig::default(),
            subscribers: RwLock::new(HashMap::new()),
            attention_reader: None,
            current_step: Arc::new(AtomicU64::new(0)),
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
    pub fn with_belief_writer(mut self, belief_writer: Arc<dyn BeliefGraphWriter>) -> Self {
        self.belief_writer = belief_writer.clone();
        self.verifier = PredictionVerifier::new(belief_writer);
        self
    }

    pub fn with_config(mut self, config: PredictionEngineConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_attention_reader(mut self, reader: Arc<dyn AttentionStatusReader>) -> Self {
        self.attention_reader = Some(reader);
        self
    }

    pub async fn create_prediction(
        &self,
        action_request: ActionRequest,
        context_hint: Option<crate::modules::prediction_engine::ContextHint>,
    ) -> Result<Prediction, PredictionError> {
        let mut prediction = self
            .generator
            .create_prediction(action_request, context_hint)
            .await?;

        if let Some(ref reader) = self.attention_reader {
            let status = reader.get_attention_status();
            if status.parameter > 0.7 {
                prediction.validity_window.duration_ms =
                    (prediction.validity_window.duration_ms as f64 * 1.5) as i64;
                prediction.metadata.confidence = (prediction.metadata.confidence * 0.9)
                    .max(self.config.min_confidence_threshold);
            } else if status.parameter < 0.3 {
                prediction.validity_window.duration_ms =
                    (prediction.validity_window.duration_ms as f64 * 0.6) as i64;
                prediction.metadata.confidence = (prediction.metadata.confidence * 1.1).min(1.0);
            }
        }

        prediction.validity_window.created_at_step = Some(self.current_step.load(Ordering::SeqCst));

        let id = prediction.prediction_id.to_string();
        self.predictions.write().insert(id, prediction.clone());

        let event = {
            let mut e = PBSMEvent::new(PredictionEvent::PredictionCreated(
                PredictionCreatedPayload {
                    prediction_id: prediction.prediction_id,
                    action_type: prediction.associated_action.action_type,
                    target_node: prediction.associated_action.target_node.clone(),
                    expected_change_count: prediction.expected_changes.len(),
                },
            ));
            e.source = EventSource {
                module: "M2".into(),
                instance_id: None,
            };
            e
        };

        self.publish_and_notify(&prediction.prediction_id.to_string(), event);

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
        let (result, event) = {
            let mut predictions = self.predictions.write();
            let prediction = predictions
                .get_mut(prediction_id)
                .ok_or_else(|| PredictionError::NotFound {
                    message: prediction_id.to_string(),
                    code: "PEV-E001".to_string(),
                })?;

            let result = self
                .verifier
                .verify_prediction(prediction, observation)
                .await?;

            let event = match prediction.status {
                PredictionState::Verified => {
                    let mut e = PBSMEvent::new(PredictionEvent::PredictionVerified(
                        PredictionVerifiedPayload {
                            prediction_id: prediction.prediction_id,
                            match_level: result.match_level,
                            confidence_delta: 0.05,
                        },
                    ));
                    e.source = EventSource {
                        module: "M2".into(),
                        instance_id: None,
                    };
                    Some(e)
                }
                PredictionState::Falsified => {
                    let mut e = PBSMEvent::new(PredictionEvent::PredictionFalsified(
                        PredictionFalsifiedPayload {
                            prediction_id: prediction.prediction_id,
                            match_level: result.match_level,
                            severity: format!("{:?}", result.residual.severity_assessment.level),
                            overall_degree: result.residual.overall_degree,
                            affected_beliefs: result.affected_beliefs.clone(),
                        },
                    ));
                    e.source = EventSource {
                        module: "M2".into(),
                        instance_id: None,
                    };
                    Some(e)
                }
                _ => None,
            };

            (result, event)
        };

        if let Some(e) = event {
            self.publish_and_notify(prediction_id, e);
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
        include_history: Option<bool>,
    ) -> Result<Prediction, PredictionError> {
        let prediction = self
            .predictions
            .read()
            .get(prediction_id)
            .cloned()
            .ok_or_else(|| PredictionError::NotFound {
                message: prediction_id.to_string(),
                code: "PEV-E001".to_string(),
            })?;

        if include_history == Some(true) {
            Ok(prediction)
        } else {
            let mut p = prediction;
            if p.status_history.len() > 1 {
                let last = p.status_history.pop().unwrap();
                p.status_history.clear();
                p.status_history.push(last);
            }
            Ok(p)
        }
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
            .ok_or_else(|| PredictionError::NotFound {
                message: prediction_id.to_string(),
                code: "PEV-E001".to_string(),
            })?;

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
        time_range: Option<TimeRange>,
    ) -> PredictionStatistics {
        let predictions = self.predictions.read();

        let filtered: Vec<&Prediction> = if let Some(ref range) = time_range {
            predictions
                .values()
                .filter(|p| {
                    p.metadata.created_at >= range.start && p.metadata.created_at <= range.end
                })
                .collect()
        } else {
            predictions.values().collect()
        };

        let total = filtered.len() as u64;
        let mut by_status: HashMap<String, u64> = HashMap::new();
        let mut by_match_level: HashMap<String, u64> = HashMap::new();
        let mut by_severity: HashMap<String, u64> = HashMap::new();

        let mut total_residual = 0.0;
        let mut verified_count = 0u64;
        let mut falsified_count = 0u64;
        let mut latency_sum = 0.0_f64;
        let mut latency_count = 0u64;
        let mut error_pattern_counts: HashMap<String, u64> = HashMap::new();

        for pred in &filtered {
            *by_status
                .entry(format!("{:?}", pred.status))
                .or_insert(0) += 1;

            if let Some(ref residual) = pred.residuals {
                total_residual += residual.overall_degree.abs();
                *by_match_level
                    .entry(format!("{:?}", residual.match_level))
                    .or_insert(0) += 1;
                *by_severity
                    .entry(format!("{:?}", residual.severity_assessment.level))
                    .or_insert(0) += 1;

                match pred.status {
                    PredictionState::Verified => {
                        verified_count += 1;
                        if let Some(verified_at) = pred.metadata.verified_at {
                            let latency =
                                (verified_at - pred.metadata.created_at).num_milliseconds() as f64;
                            latency_sum += latency;
                            latency_count += 1;
                        }
                    }
                    PredictionState::Falsified => {
                        falsified_count += 1;
                        let latency = (residual.metadata.computed_at
                            - pred.metadata.created_at)
                            .num_milliseconds() as f64;
                        latency_sum += latency;
                        latency_count += 1;
                        for component in &residual.component_residuals {
                            *error_pattern_counts
                                .entry(component.attribute.clone())
                                .or_insert(0) += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        let non_pending = filtered
            .iter()
            .filter(|p| p.status != PredictionState::Pending)
            .count() as f64;

        let mut error_patterns: Vec<_> = error_pattern_counts.into_iter().collect();
        error_patterns.sort_by_key(|b| std::cmp::Reverse(b.1));
        let top_error_patterns = error_patterns
            .into_iter()
            .take(5)
            .map(|(pattern, count)| ErrorPattern { pattern, count })
            .collect();

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
            average_latency_ms: if latency_count > 0 {
                latency_sum / latency_count as f64
            } else {
                0.0
            },
            top_error_patterns,
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
    pub fn get_prediction_residual_history(
        &self,
        prediction_id: &str,
    ) -> Result<ResidualHistory, PredictionError> {
        let prediction = self
            .predictions
            .read()
            .get(prediction_id)
            .cloned()
            .ok_or_else(|| PredictionError::NotFound {
                message: prediction_id.to_string(),
                code: "PEV-E001".to_string(),
            })?;

        let residuals = prediction
            .residuals
            .map(|r| vec![r])
            .unwrap_or_default();

        let computed_at: Vec<_> = residuals
            .iter()
            .map(|r| r.metadata.computed_at)
            .collect();

        let current_avg = residuals
            .iter()
            .map(|r| r.overall_degree)
            .next()
            .unwrap_or(0.0);

        Ok(ResidualHistory {
            prediction_id: prediction_id.to_string(),
            residuals,
            computed_at,
            trend: ResidualTrend {
                trend: TrendDirection::Stable,
                previous_average: 0.0,
                current_average: current_avg,
            },
            annotations: Vec::new(),
        })
    }

    pub fn subscribe_to_prediction(
        &self,
        prediction_id: &str,
        callback: Box<dyn Fn(PBSMEvent) + Send + Sync>,
    ) -> Result<String, SubscriptionError> {
        let subscription_id = Uuid::new_v4().to_string();
        let mut subscribers = self.subscribers.write();
        subscribers
            .entry(prediction_id.to_string())
            .or_insert_with(Vec::new)
            .push(callback);
        Ok(subscription_id)
    }

    pub fn advance_step(&self) {
        self.current_step.fetch_add(1, Ordering::SeqCst);
    }

    pub fn cleanup_expired(&self) -> usize {
        let current_step_value = self.current_step.load(Ordering::SeqCst);
        let mut predictions = self.predictions.write();
        let mut removed = 0;
        let mut expired_info: Vec<(Uuid, u64)> = Vec::new();

        for pred in predictions.values_mut() {
            if pred.status == PredictionState::Pending {
                let expired = if self.config.step_expiry_enabled {
                    pred.validity_window.is_expired_at(Some(current_step_value))
                } else {
                    pred.validity_window.is_expired()
                };

                if expired {
                    let pred_id = pred.prediction_id;
                    let duration_ms = pred.validity_window.duration_ms as u64;
                    let _ = pred.transition_to(PredictionState::Expired, "Validity window expired");
                    expired_info.push((pred_id, duration_ms));
                    removed += 1;
                }
            }
        }

        drop(predictions);

        for (prediction_id, duration_ms) in expired_info {
            let pred_id_str = prediction_id.to_string();
            let event = {
                let mut e = PBSMEvent::new(PredictionEvent::PredictionExpired(
                    PredictionExpiredPayload {
                        prediction_id,
                        expiration_reason: "Validity window expired".to_string(),
                        duration_ms,
                    },
                ));
                e.source = EventSource {
                    module: "M2".into(),
                    instance_id: None,
                };
                e
            };
            self.publish_and_notify(&pred_id_str, event);
        }

        removed
    }

    fn publish_and_notify(&self, prediction_id: &str, event: PBSMEvent) {
        let _ = self.event_publisher.publish_event(event.clone());

        let subscribers = self.subscribers.read();
        if let Some(callbacks) = subscribers.get(prediction_id) {
            for callback in callbacks {
                callback(event.clone());
            }
        }
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
    use crate::modules::common::NullAttentionStatusReader;
    use crate::types::filter::CancellationReason;
    use crate::types::prediction::{ActionType, PredictionState};
    use std::sync::atomic::AtomicUsize;

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
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-1".to_string()),
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
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-1".to_string()),
        };

        let created = engine.create_prediction(action, None).await.unwrap();
        let id = created.prediction_id.to_string();

        let result = engine.cancel_prediction(&id, CancellationReason::UserRequest);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_prediction_by_id_with_history() {
        let engine = PredictionEngine::new();

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-1".to_string()),
        };

        let created = engine.create_prediction(action, None).await.unwrap();
        let id = created.prediction_id.to_string();

        let with_history = engine.get_prediction_by_id(&id, Some(true)).unwrap();
        assert!(with_history.status_history.len() >= 1);

        let without_history = engine.get_prediction_by_id(&id, Some(false)).unwrap();
        assert_eq!(without_history.status_history.len(), 1);
    }

    #[tokio::test]
    async fn test_get_prediction_residual_history() {
        let engine = PredictionEngine::new();

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-1".to_string()),
        };

        let created = engine.create_prediction(action, None).await.unwrap();
        let id = created.prediction_id.to_string();

        let history = engine.get_prediction_residual_history(&id).unwrap();
        assert_eq!(history.prediction_id, id);
        assert!(history.residuals.is_empty());
    }

    #[tokio::test]
    async fn test_subscribe_to_prediction() {
        let engine = PredictionEngine::new();

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-1".to_string()),
        };

        let created = engine.create_prediction(action, None).await.unwrap();
        let id = created.prediction_id.to_string();

        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();
        let result = engine.subscribe_to_prediction(
            &id,
            Box::new(move |_event| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_advance_step() {
        let engine = PredictionEngine::new();
        assert_eq!(engine.current_step.load(Ordering::SeqCst), 0);
        engine.advance_step();
        assert_eq!(engine.current_step.load(Ordering::SeqCst), 1);
        engine.advance_step();
        assert_eq!(engine.current_step.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_prediction_created_at_step() {
        let engine = PredictionEngine::new();
        engine.advance_step();
        engine.advance_step();

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-1".to_string()),
        };

        let created = engine.create_prediction(action, None).await.unwrap();
        assert_eq!(created.validity_window.created_at_step, Some(2));
    }

    #[test]
    fn test_prediction_engine_config_default() {
        let config = PredictionEngineConfig::default();
        assert_eq!(config.max_active_predictions, 1000);
        assert_eq!(config.min_confidence_threshold, 0.3);
        assert_eq!(config.warning_threshold, 0.3);
        assert_eq!(config.error_threshold, 0.7);
        assert_eq!(config.critical_threshold, 1.0);
        assert_eq!(config.tolerance_margin, 0.05);
        assert!(config.enable_cascading_effects);
        assert_eq!(config.max_cascade_depth, 3);
        assert!(config.step_expiry_enabled);
        assert!(config.event_driven_expiry_enabled);
    }

    #[tokio::test]
    async fn test_with_belief_writer() {
        let engine = PredictionEngine::new().with_belief_writer(Arc::new(NullBeliefGraphWriter));

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-1".to_string()),
        };

        let result = engine.create_prediction(action, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_attention_reader() {
        let engine =
            PredictionEngine::new().with_attention_reader(Arc::new(NullAttentionStatusReader));

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-1".to_string()),
        };

        let result = engine.create_prediction(action, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subscriber_notified_on_create() {
        let engine = PredictionEngine::new();

        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let pred_id = {
            let action = ActionRequest {
                action_type: ActionType::ToolCall,
                action_name: "test".to_string(),
                parameters: serde_json::json!({"expected_value": "success"}),
                target_id: Some("node-1".to_string()),
            };

            let created = engine.create_prediction(action, None).await.unwrap();
            let id = created.prediction_id.to_string();

            engine
                .subscribe_to_prediction(
                    &id,
                    Box::new(move |_event| {
                        count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                )
                .unwrap();

            id
        };

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "test2".to_string(),
            parameters: serde_json::json!({"expected_value": "other"}),
            target_id: Some("node-2".to_string()),
        };

        let _other = engine.create_prediction(action, None).await.unwrap();

        engine
            .verify_prediction(
                &pred_id,
                Observation {
                    format: "json".to_string(),
                    data: serde_json::json!({"status": "success"}),
                    timestamp: chrono::Utc::now(),
                    source: "test".to_string(),
                },
            )
            .await
            .ok();

        assert!(call_count.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn test_cleanup_expired_with_step() {
        let engine = PredictionEngine::new();

        let mut predictions = engine.predictions.write();
        let mut pred = Prediction::new();
        pred.validity_window = crate::types::prediction::ValidityWindow::new_steps_window(2);
        pred.validity_window.created_at_step = Some(0);
        let id = pred.prediction_id.to_string();
        predictions.insert(id, pred);
        drop(predictions);

        engine.advance_step();
        engine.advance_step();
        engine.advance_step();

        let removed = engine.cleanup_expired();
        assert_eq!(removed, 1);
    }
}
