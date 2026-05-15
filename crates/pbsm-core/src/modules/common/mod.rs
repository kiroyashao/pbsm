//! 公共接口定义
//!
//! 本模块定义了预测引擎与外部系统交互的接口，包括：
//! - 信念图读写接口
//! - 事件发布接口
//! - 相关数据结构
//!
//! # 接口设计
//!
//! - BeliefGraphReader/BeliefGraphWriter：信念图读写接口
//! - EventPublisher：事件发布接口
//! - Null*：空实现，用于测试和默认配置
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::types::prediction::ActionType;
use crate::types::residual::{MatchLevel, Residual};

/// 信念图读取接口
///
/// # 设计说明
///
/// 该接口定义了预测引擎读取信念图所需的全部操作。
/// 实现者需要提供线程安全的信念查询能力。
#[async_trait]
pub trait BeliefGraphReader: Send + Sync {
    async fn query_belief_by_id(
        &self,
        node_id: &str,
    ) -> Result<Option<BeliefNode>, BeliefGraphError>;
    async fn query_beliefs(
        &self,
        query_spec: BeliefQuerySpec,
    ) -> Result<Vec<BeliefNode>, BeliefGraphError>;
    async fn get_belief_state(
        &self,
        belief_ids: &[String],
    ) -> Result<BeliefState, BeliefGraphError>;
    async fn get_outgoing_edges(
        &self,
        node_id: &str,
    ) -> Result<Vec<RelationEdge>, BeliefGraphError>;
    async fn get_incoming_edges(
        &self,
        node_id: &str,
    ) -> Result<Vec<RelationEdge>, BeliefGraphError>;
    async fn get_belief_history(
        &self,
        node_id: &str,
        range: BeliefHistoryRange,
    ) -> Result<Vec<BeliefVersion>, BeliefGraphError>;
}

/// 信念节点结构体
///
/// # 字段说明
///
/// - node_id：节点唯一标识符
/// - node_type：节点类型（如 Entity、Event、Action 等）
/// - attributes：节点属性（JSON格式）
/// - confidence：信念置信度 [0.0, 1.0]
/// - created_at/updated_at：时间戳
#[derive(Debug, Clone)]
pub struct BeliefNode {
    pub node_id: String,
    pub node_type: String,
    pub attributes: Value,
    pub confidence: f64,
    pub attribute_confidences: std::collections::HashMap<String, f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 关系边结构体
///
/// # 字段说明
///
/// - edge_id：边唯一标识符
/// - source_node：源节点ID
/// - target_node：目标节点ID
/// - edge_type：边类型（如因果、包含、关联等）
/// - confidence：边的置信度
#[derive(Debug, Clone)]
pub struct RelationEdge {
    pub edge_id: Uuid,
    pub source_node: String,
    pub target_node: String,
    pub edge_type: String,
    pub confidence: f64,
}

/// 信念状态结构体
///
/// # 用途
///
/// 用于批量获取多个信念节点的完整状态快照
#[derive(Debug, Clone)]
pub struct BeliefState {
    pub nodes: Vec<BeliefNode>,
    pub edges: Vec<RelationEdge>,
    pub hash: String,
}

/// 信念查询规格结构体
///
/// # 字段说明
///
/// - node_type：按节点类型过滤
/// - attributes：需要返回的属性列表
/// - confidence_threshold：最低置信度阈值
#[derive(Debug, Clone)]
pub struct BeliefQuerySpec {
    pub node_type: Option<String>,
    pub attributes: Option<Vec<String>>,
    pub confidence_threshold: Option<f64>,
}

/// 信念图错误类型
#[derive(Debug, Clone)]
pub enum BeliefGraphError {
    NodeNotFound(String),
    EdgeNotFound(String),
    ValidationError(String),
    NodeExists(String),
    EdgeExists(String),
    CapacityExceeded { nodes: usize, edges: usize },
    QueryFailed(String),
    SerializationError(String),
    InternalError(String),
}

impl std::fmt::Display for BeliefGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BeliefGraphError::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            BeliefGraphError::EdgeNotFound(id) => write!(f, "Edge not found: {}", id),
            BeliefGraphError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            BeliefGraphError::NodeExists(id) => write!(f, "Node already exists: {}", id),
            BeliefGraphError::EdgeExists(id) => write!(f, "Edge already exists: {}", id),
            BeliefGraphError::CapacityExceeded { nodes, edges } => {
                write!(f, "Capacity exceeded: nodes={}, edges={}", nodes, edges)
            }
            BeliefGraphError::QueryFailed(msg) => write!(f, "Query failed: {}", msg),
            BeliefGraphError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            BeliefGraphError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for BeliefGraphError {}

/// 信念图写入接口
///
/// # 设计说明
///
/// 该接口定义了更新信念图所需的操作。
/// 主要用于预测验证后对信念的修正。
#[derive(Debug, Clone)]
pub struct BeliefHistoryRange {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct BeliefVersion {
    pub version: u64,
    pub confidence: f64,
    pub attributes: Value,
    pub timestamp: DateTime<Utc>,
}

#[async_trait]
pub trait BeliefGraphWriter: Send + Sync {
    async fn update_belief_confidence(
        &self,
        node_id: &str,
        attribute: &str,
        new_confidence: f64,
    ) -> Result<(), BeliefGraphError>;

    async fn mark_belief_for_revision(
        &self,
        belief_id: &str,
        reason: &str,
    ) -> Result<(), BeliefGraphError>;
}

/// 事件发布接口
///
/// # 设计说明
///
/// 预测引擎通过该接口发布各类事件，供外部系统订阅处理。
pub trait EventPublisher: Send + Sync {
    fn publish_event(&self, event: PBSMEvent) -> Result<(), EventPublishError>;
}

/// 预测事件枚举
///
/// # 事件类型
///
/// - PredictionCreated：预测创建事件
/// - PredictionVerified：预测验证通过事件
/// - PredictionFalsified：预测被证伪事件
/// - ResidualComputed：残差计算完成事件
/// - *ResidualDetected：各严重程度残差检测事件
#[derive(Debug, Clone)]
pub enum PredictionEvent {
    PredictionCreated(PredictionCreatedPayload),
    PredictionVerified(PredictionVerifiedPayload),
    PredictionFalsified(PredictionFalsifiedPayload),
    ResidualComputed(ResidualComputedPayload),
    WarningResidualDetected(WarningResidualPayload),
    ErrorResidualDetected(ErrorResidualPayload),
    CriticalResidualDetected(CriticalResidualPayload),
    PredictionStatusChanged(PredictionStatusChangedPayload),
    PredictionExpired(PredictionExpiredPayload),
    PredictionCancelled(PredictionCancelledPayload),
    ResidualTrendAlert(ResidualTrendAlertPayload),
    PredictionEngineInitialized(PredictionEngineInitializedPayload),
    PredictionEngineError(PredictionEngineErrorPayload),
    ContextIntegrityWarning(ContextIntegrityWarningPayload),
    VerificationTimeout(VerificationTimeoutPayload),
}

/// 预测创建事件载荷
#[derive(Debug, Clone)]
pub struct PredictionCreatedPayload {
    pub prediction_id: Uuid,
    pub action_type: ActionType,
    pub target_node: Option<String>,
    pub expected_change_count: usize,
}

/// 预测验证通过事件载荷
#[derive(Debug, Clone)]
pub struct PredictionVerifiedPayload {
    pub prediction_id: Uuid,
    pub match_level: MatchLevel,
    pub confidence_delta: f64,
}

/// 预测被证伪事件载荷
#[derive(Debug, Clone)]
pub struct PredictionFalsifiedPayload {
    pub prediction_id: Uuid,
    pub match_level: MatchLevel,
    pub severity: String,
    pub overall_degree: f64,
    pub affected_beliefs: Vec<String>,
}

/// 残差计算完成事件载荷
#[derive(Debug, Clone)]
pub struct ResidualComputedPayload {
    pub prediction_id: Uuid,
    pub residual: Residual,
}

/// Warning级别残差检测事件载荷
#[derive(Debug, Clone)]
pub struct WarningResidualPayload {
    pub prediction_id: Uuid,
    pub residual: Residual,
    pub affected_beliefs: Vec<String>,
}

/// Error级别残差检测事件载荷
#[derive(Debug, Clone)]
pub struct ErrorResidualPayload {
    pub prediction_id: Uuid,
    pub residual: Residual,
    pub affected_beliefs: Vec<String>,
    pub suggested_action: String,
}

/// Critical级别残差检测事件载荷
#[derive(Debug, Clone)]
pub struct CriticalResidualPayload {
    pub prediction_id: Uuid,
    pub residual: Residual,
    pub affected_beliefs: Vec<String>,
    pub urgency: Urgency,
}

/// 紧急程度枚举
#[derive(Debug, Clone, Copy)]
pub enum Urgency {
    Immediate,
    High,
    Normal,
}

/// 事件发布错误类型
#[derive(Debug, Clone)]
pub struct PredictionStatusChangedPayload {
    pub prediction_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct PredictionExpiredPayload {
    pub prediction_id: Uuid,
    pub expiration_reason: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct PredictionCancelledPayload {
    pub prediction_id: Uuid,
    pub cancellation_reason: String,
}

#[derive(Debug, Clone)]
pub struct ResidualTrendAlertPayload {
    pub prediction_id: Uuid,
    pub trend: String,
    pub previous_average: f64,
    pub current_average: f64,
}

#[derive(Debug, Clone)]
pub struct PredictionEngineInitializedPayload {
    pub version: String,
    pub config_summary: String,
    pub active_prediction_count: usize,
}

#[derive(Debug, Clone)]
pub struct PredictionEngineErrorPayload {
    pub error: String,
    pub context: String,
    pub recovery_action: String,
}

#[derive(Debug, Clone)]
pub struct ContextIntegrityWarningPayload {
    pub prediction_id: Uuid,
    pub missing_fields: Vec<String>,
    pub completeness_score: f64,
}

#[derive(Debug, Clone)]
pub struct VerificationTimeoutPayload {
    pub prediction_id: Uuid,
    pub elapsed_ms: u64,
    pub threshold_ms: u64,
}

#[derive(Debug, Clone)]
pub enum EventPublishError {
    PublishFailed(String),
    EventTypeUnknown,
}

impl std::fmt::Display for EventPublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventPublishError::PublishFailed(msg) => write!(f, "Publish failed: {}", msg),
            EventPublishError::EventTypeUnknown => write!(f, "Unknown event type"),
        }
    }
}

impl std::error::Error for EventPublishError {}

/// 信念图读取接口的空实现
///
/// # 用途
///
/// 用于测试或当不需要信念图功能时的默认实现
#[derive(Debug, Clone)]
pub struct EventSource {
    pub module: String,
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PBSMEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub source: EventSource,
    pub timestamp: DateTime<Utc>,
    pub correlation_id: Option<String>,
    pub payload: PredictionEvent,
}

impl PBSMEvent {
    pub fn new(payload: PredictionEvent) -> Self {
        let event_type = match &payload {
            PredictionEvent::PredictionCreated(_) => "PredictionCreated",
            PredictionEvent::PredictionVerified(_) => "PredictionVerified",
            PredictionEvent::PredictionFalsified(_) => "PredictionFalsified",
            PredictionEvent::ResidualComputed(_) => "ResidualComputed",
            PredictionEvent::WarningResidualDetected(_) => "WarningResidualDetected",
            PredictionEvent::ErrorResidualDetected(_) => "ErrorResidualDetected",
            PredictionEvent::CriticalResidualDetected(_) => "CriticalResidualDetected",
            PredictionEvent::PredictionStatusChanged(_) => "PredictionStatusChanged",
            PredictionEvent::PredictionExpired(_) => "PredictionExpired",
            PredictionEvent::PredictionCancelled(_) => "PredictionCancelled",
            PredictionEvent::ResidualTrendAlert(_) => "ResidualTrendAlert",
            PredictionEvent::PredictionEngineInitialized(_) => "PredictionEngineInitialized",
            PredictionEvent::PredictionEngineError(_) => "PredictionEngineError",
            PredictionEvent::ContextIntegrityWarning(_) => "ContextIntegrityWarning",
            PredictionEvent::VerificationTimeout(_) => "VerificationTimeout",
        };
        PBSMEvent {
            event_id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            source: EventSource {
                module: "pbsm-core".to_string(),
                instance_id: None,
            },
            timestamp: Utc::now(),
            correlation_id: None,
            payload,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SubscriptionError {
    NotFound(String),
    AlreadySubscribed(String),
    InvalidCallback(String),
    InternalError(String),
}

impl std::fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionError::NotFound(id) => write!(f, "Subscription not found: {}", id),
            SubscriptionError::AlreadySubscribed(id) => {
                write!(f, "Already subscribed: {}", id)
            }
            SubscriptionError::InvalidCallback(msg) => {
                write!(f, "Invalid callback: {}", msg)
            }
            SubscriptionError::InternalError(msg) => {
                write!(f, "Internal error: {}", msg)
            }
        }
    }
}

impl std::error::Error for SubscriptionError {}

#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    pub subscription_id: String,
    pub prediction_id: String,
}

pub trait PredictionSubscriber: Send + Sync {
    fn subscribe(
        &self,
        prediction_id: &str,
        callback: Box<dyn Fn(PBSMEvent) + Send + Sync>,
    ) -> Result<String, SubscriptionError>;
    fn unsubscribe(&self, subscription_id: &str) -> Result<(), SubscriptionError>;
}

#[derive(Debug, Clone)]
pub struct AttentionStatus {
    pub parameter: f64,
    pub mode: String,
    pub focus_areas: Vec<String>,
}

pub trait AttentionStatusReader: Send + Sync {
    fn get_attention_status(&self) -> AttentionStatus;
    fn get_focus_areas(&self) -> Vec<String>;
}

pub struct NullAttentionStatusReader;

impl AttentionStatusReader for NullAttentionStatusReader {
    fn get_attention_status(&self) -> AttentionStatus {
        AttentionStatus {
            parameter: 0.0,
            mode: String::new(),
            focus_areas: Vec::new(),
        }
    }

    fn get_focus_areas(&self) -> Vec<String> {
        Vec::new()
    }
}

pub struct NullBeliefGraphReader;

#[async_trait]
impl BeliefGraphReader for NullBeliefGraphReader {
    async fn query_belief_by_id(
        &self,
        _node_id: &str,
    ) -> Result<Option<BeliefNode>, BeliefGraphError> {
        Ok(None)
    }

    async fn query_beliefs(
        &self,
        _query_spec: BeliefQuerySpec,
    ) -> Result<Vec<BeliefNode>, BeliefGraphError> {
        Ok(Vec::new())
    }

    async fn get_belief_state(
        &self,
        _belief_ids: &[String],
    ) -> Result<BeliefState, BeliefGraphError> {
        Ok(BeliefState {
            nodes: Vec::new(),
            edges: Vec::new(),
            hash: String::new(),
        })
    }

    async fn get_outgoing_edges(
        &self,
        _node_id: &str,
    ) -> Result<Vec<RelationEdge>, BeliefGraphError> {
        Ok(Vec::new())
    }

    async fn get_incoming_edges(
        &self,
        _node_id: &str,
    ) -> Result<Vec<RelationEdge>, BeliefGraphError> {
        Ok(Vec::new())
    }

    async fn get_belief_history(
        &self,
        _node_id: &str,
        _range: BeliefHistoryRange,
    ) -> Result<Vec<BeliefVersion>, BeliefGraphError> {
        Ok(Vec::new())
    }
}

/// 信念图写入接口的空实现
pub struct NullBeliefGraphWriter;

#[async_trait]
impl BeliefGraphWriter for NullBeliefGraphWriter {
    async fn update_belief_confidence(
        &self,
        _node_id: &str,
        _attribute: &str,
        _new_confidence: f64,
    ) -> Result<(), BeliefGraphError> {
        Ok(())
    }

    async fn mark_belief_for_revision(
        &self,
        _belief_id: &str,
        _reason: &str,
    ) -> Result<(), BeliefGraphError> {
        Ok(())
    }
}

/// 事件发布接口的空实现
///
/// # 用途
///
/// 静默丢弃所有事件，用于不需要事件通知的场景
pub struct NullEventPublisher;

impl EventPublisher for NullEventPublisher {
    fn publish_event(&self, _event: PBSMEvent) -> Result<(), EventPublishError> {
        Ok(())
    }
}

impl BeliefGraphEventPublisher for NullEventPublisher {
    fn publish(&self, _event: BeliefGraphEvent) -> Result<(), EventPublishError> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum BeliefGraphEvent {
    BeliefCreated { node_id: String, node_type: String, source: String },
    BeliefUpdated { node_id: String, update_type: String, old_confidence: f64, new_confidence: f64 },
    BeliefDeleted { node_id: String, cascade: bool },
    EdgeCreated { edge_id: String, source_node: String, target_node: String, edge_type: String },
    EdgeUpdated { edge_id: String, source_node: String, target_node: String, edge_type: String },
    EdgeDeleted { edge_id: String, source_node: String, target_node: String },
    SnapshotCreated { snapshot_id: String, version: u64 },
    RollbackCompleted { snapshot_id: String, target_version: u64 },
    FusionCompleted { snapshot_id: String, merged_count: usize, conflict_count: usize },
    ConflictDetected { node_id: String, local_confidence: f64, external_confidence: f64, strategy: String },
    ConfidenceThresholdCrossed { node_id: String, attribute: String, old_confidence: f64, new_confidence: f64, threshold: f64 },
    CapacityWarning { current_nodes: usize, max_nodes: usize },
    BeliefDerived { source_id: String, derived_id: String, confidence: f64 },
    GraphTraversed { start_node: String, visited_count: usize },
    AuditBeliefModified { node_id: String, modifier: String, change_description: String },
    AuditAccessPerformed { accessor: String, target_node_ids: Vec<String>, access_type: String },
    AuditRollbackExecuted { snapshot_id: String, operator: String },
}

pub trait BeliefGraphEventPublisher: Send + Sync {
    fn publish(&self, event: BeliefGraphEvent) -> Result<(), EventPublishError>;
}
