//! 外部记忆存储核心数据类型定义
//!
//! 本模块定义了 M4 外部记忆存储的核心数据结构，包括：
//! - 记忆层类型：原始日志、快照、经验三层存储架构
//! - 检索与查询：支持上下文感知检索和问题导向检索
//! - 快照管理：信念状态和意图状态的完整快照
//! - 清理策略：多层记忆的自动清理与归档
//!
//! # 三层记忆架构
//!
//! | 层级 | 类型 | 保留策略 | 典型用途 |
//! |------|------|---------|---------|
//! | RawLog | 原始日志 | 短期保留 | 事件溯源、审计追踪 |
//! | Snapshot | 快照 | 中期保留 | 状态恢复、错误恢复 |
//! | Experience | 经验 | 长期保留 | 模式识别、知识积累 |
//!
//! # 性能约束
//!
//! - 检索响应时间：≤50ms（标准深度）
//! - 快照写入时间：≤100ms
//! - 快照恢复时间：≤200ms
//! - 清理操作：后台异步执行

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type Timestamp = i64;

pub fn validate_confidence(value: f64) -> Result<f64, String> {
    if !(0.0..=1.0).contains(&value) {
        Err(format!("Confidence must be in range [0.0, 1.0], got {}", value))
    } else {
        Ok(value)
    }
}

/// 记忆层枚举
///
/// # 层级说明
///
/// | 层级 | 描述 | 保留周期 |
/// |------|------|---------|
/// | RawLog | 原始事件日志 | 1-7天 |
/// | Snapshot | 状态快照 | 7-30天 |
/// | Experience | 结构化经验 | 永久 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoryLayer {
    RawLog,
    Snapshot,
    Experience,
}

/// 日志类型枚举
///
/// # 类型说明
///
/// | 类型 | 描述 |
/// |------|------|
/// | Dialogue | 对话记录 |
/// | ToolCall | 工具调用记录 |
/// | BeliefUpdate | 信念更新记录 |
/// | ExecutionTrace | 执行追踪记录 |
/// | SystemEvent | 系统事件记录 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LogType {
    Dialogue,
    ToolCall,
    BeliefUpdate,
    ExecutionTrace,
    SystemEvent,
}

/// 快照类型枚举
///
/// # 类型说明
///
/// | 类型 | 描述 | 触发方式 |
/// |------|------|---------|
/// | Manual | 手动快照 | 用户主动触发 |
/// | Automatic | 自动快照 | 系统定时触发 |
/// | Scheduled | 计划快照 | 按计划触发 |
/// | ErrorRecovery | 错误恢复快照 | 异常时自动触发 |
/// | SessionEnd | 会话结束快照 | 会话终止时触发 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SnapshotType {
    Manual,
    Automatic,
    Scheduled,
    ErrorRecovery,
    SessionEnd,
}

/// 压缩类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum CompressionType {
    None,
    #[default]
    Lz4,
    Zstd,
}

/// 注意力模式枚举
///
/// # 模式说明
///
/// | 模式 | 参数范围 | 描述 |
/// |------|---------|------|
/// | LowVigilance | α ≤ 0.3 | 低分辨率处理 |
/// | Moderate | 0.3 < α ≤ 0.7 | 常规评估 |
/// | HighResolution | α > 0.7 | 高度聚焦扫描 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttentionMode {
    LowVigilance,
    Moderate,
    HighResolution,
}

/// 意图状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntentionStatus {
    Active,
    Suspended,
    Completed,
    Abandoned,
}

/// 问题类型枚举
///
/// # 类型说明
///
/// | 类型 | 描述 |
/// |------|------|
/// | ToolExecutionFailure | 工具执行失败 |
/// | PredictionMismatch | 预测不匹配 |
/// | BeliefConflict | 信念冲突 |
/// | GoalAmbiguity | 目标模糊 |
/// | ResourceConstraint | 资源约束 |
/// | Unknown | 未知问题类型 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProblemType {
    ToolExecutionFailure,
    PredictionMismatch,
    BeliefConflict,
    GoalAmbiguity,
    ResourceConstraint,
    Unknown,
}

/// 模式类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PatternType {
    ErrorHandling,
    TaskPattern,
    ToolSequence,
    BeliefCorrection,
    GoalDecomposition,
}

/// 清理类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CleanupType {
    Lazy,
    Standard,
    Aggressive,
    Manual,
}

/// 清理范围枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CleanupScope {
    RawLogOnly,
    SnapshotOnly,
    ExperienceOnly,
    AllLayers,
    AllLayersPlusDeep,
}

/// 清理状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CleanupStatus {
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// 记忆加载状态枚举
///
/// # 状态流转
///
/// ```text
/// Idle → Triggered → Retrieving → Filtering → Transforming → Integrating → Completed
///                                                                    ↓
///                                                                 Failed
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoryLoadingState {
    Idle,
    Triggered,
    Retrieving,
    Filtering,
    Transforming,
    Integrating,
    Completed,
    Failed,
}

#[derive(Debug)]
pub struct MemoryLoadingStateMachine {
    current_state: MemoryLoadingState,
}

impl MemoryLoadingStateMachine {
    pub fn new() -> Self {
        Self {
            current_state: MemoryLoadingState::Idle,
        }
    }

    pub fn current_state(&self) -> MemoryLoadingState {
        self.current_state
    }

    pub fn transition(&mut self, next: MemoryLoadingState) -> Result<MemoryLoadingState, String> {
        let valid = match (&self.current_state, &next) {
            (MemoryLoadingState::Idle, MemoryLoadingState::Triggered) => true,
            (MemoryLoadingState::Triggered, MemoryLoadingState::Retrieving) => true,
            (MemoryLoadingState::Triggered, MemoryLoadingState::Failed) => true,
            (MemoryLoadingState::Retrieving, MemoryLoadingState::Filtering) => true,
            (MemoryLoadingState::Retrieving, MemoryLoadingState::Failed) => true,
            (MemoryLoadingState::Filtering, MemoryLoadingState::Transforming) => true,
            (MemoryLoadingState::Filtering, MemoryLoadingState::Failed) => true,
            (MemoryLoadingState::Transforming, MemoryLoadingState::Integrating) => true,
            (MemoryLoadingState::Transforming, MemoryLoadingState::Failed) => true,
            (MemoryLoadingState::Integrating, MemoryLoadingState::Completed) => true,
            (MemoryLoadingState::Integrating, MemoryLoadingState::Failed) => true,
            (MemoryLoadingState::Completed, MemoryLoadingState::Idle) => true,
            (MemoryLoadingState::Failed, MemoryLoadingState::Idle) => true,
            _ => false,
        };
        if !valid {
            return Err(format!(
                "Invalid state transition from {:?} to {:?}",
                self.current_state, next
            ));
        }
        self.current_state = next;
        Ok(self.current_state)
    }

    pub fn reset(&mut self) {
        self.current_state = MemoryLoadingState::Idle;
    }
}

impl Default for MemoryLoadingStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// 置信度缺口紧迫性枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GapUrgency {
    Low,
    Medium,
    High,
}

/// 检索深度枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum RetrievalDepth {
    Shallow,
    #[default]
    Standard,
    Deep,
}

/// 问题解决结果枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProblemOutcome {
    Success,
    Partial,
    Failed,
}

/// 状态恢复目标枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StateTarget {
    Full,
    BeliefOnly,
    IntentionOnly,
}

/// 来源引用结构体
///
/// 记录记忆条目的来源信息，支持跨层追溯
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReference {
    pub ref_type: String,
    pub ref_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogReferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_prediction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_belief_node_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_log_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotTrigger {
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    pub description: String,
}

/// 分页信息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationInfo {
    pub offset: usize,
    pub limit: usize,
    pub total_count: usize,
    pub has_more: bool,
}

/// 搜索元数据结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMetadata {
    pub search_duration_ms: i64,
    pub indexes_used: Vec<String>,
    pub cache_hit: bool,
}

/// 记忆条目结构体
///
/// # 字段说明
///
/// 统一的记忆条目表示，可来自任意记忆层。
/// 通过 `layer` 和 `memory_type` 字段区分来源和类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub entry_id: String,
    pub layer: MemoryLayer,
    pub memory_type: String,
    pub relevance_score: f64,
    pub confidence: f64,
    pub importance: f64,
    pub recency_score: f64,
    pub summary: String,
    pub content: serde_json::Value,
    pub source_references: Vec<SourceReference>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    pub access_count: usize,
}

/// 检索结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalResult {
    pub request_id: String,
    pub query_topic: String,
    pub total_matches: usize,
    pub results: Vec<MemoryEntry>,
    pub pagination: PaginationInfo,
    pub search_metadata: SearchMetadata,
}

/// 信念上下文结构体
///
/// 描述当前信念图中与某个主题相关的上下文信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeliefContext {
    pub topic: String,
    pub current_confidence: f64,
    pub related_entities: Vec<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_access_time: DateTime<Utc>,
}

/// 置信度缺口结构体
///
/// 标识当前知识中置信度不足的区域
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceGap {
    pub topic: String,
    pub required_confidence: f64,
    pub current_confidence: f64,
    pub urgency: GapUrgency,
}

/// 结构化断言结构体
///
/// 以主谓宾形式表示的知识断言
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredAssertion {
    pub assertion_type: String,
    pub subject_id: String,
    pub predicate: String,
    pub object_value: serde_json::Value,
    pub confidence: f64,
    pub source: String,
}

/// 集成建议结构体
///
/// 描述如何将新知识集成到现有信念图中
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationSuggestion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_id: Option<String>,
    pub action: String,
    pub priority: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_notes: Option<String>,
}

/// 知识束结构体
///
/// 从经验层检索到的结构化知识集合
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBundle {
    pub bundle_id: String,
    pub source_experience_ids: Vec<String>,
    pub source_snapshot_ids: Vec<String>,
    pub structured_assertions: Vec<StructuredAssertion>,
    pub integration_suggestions: Vec<IntegrationSuggestion>,
}

/// 上下文感知检索结果结构体
///
/// 基于当前信念状态和置信度缺口的检索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextualRetrievalResult {
    pub request_id: String,
    pub identified_gaps: Vec<ConfidenceGap>,
    pub retrieved_knowledge: Vec<KnowledgeBundle>,
    pub confidence_predictions: HashMap<String, f64>,
    pub confidence_improvement_estimate: f64,
}

/// 相似问题案例结构体
///
/// 从历史经验中检索到的相似问题及其解决方案
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarProblemCase {
    pub problem_id: String,
    pub problem_description: String,
    pub similarity_score: f64,
    pub resolution_steps: Vec<String>,
    pub outcome: ProblemOutcome,
    pub resolution_context: String,
}

/// 解决方案步骤结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolutionStep {
    pub step_number: u32,
    pub action_description: String,
    pub expected_outcome: String,
    pub adaptation_guidance: String,
    pub confidence: f64,
}

/// 问题检索结果结构体
///
/// 基于当前问题的历史经验检索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemRetrievalResult {
    pub request_id: String,
    pub original_problem: String,
    pub inferred_problem_type: ProblemType,
    pub similar_problems: Vec<SimilarProblemCase>,
    pub recommended_steps: Vec<SolutionStep>,
    pub adaptation_notes: Vec<String>,
    pub confidence: f64,
}

/// 快照元数据结构体
///
/// # 字段说明
///
/// - `created_at`: Unix 毫秒时间戳
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMetadata {
    pub snapshot_id: String,
    pub session_id: String,
    pub version: String,
    pub snapshot_type: SnapshotType,
    pub agent_id: String,
    pub trigger: SnapshotTrigger,
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_ratio: Option<f64>,
}

/// 信念状态结构体
///
/// 完整的信念图状态，包含节点、边、活跃预测和未解决残差
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeliefState {
    pub nodes: Vec<crate::modules::belief_graph::types::BeliefNode>,
    pub edges: Vec<crate::modules::belief_graph::types::RelationEdge>,
    pub active_predictions: Vec<serde_json::Value>,
    pub unresolved_residuals: Vec<serde_json::Value>,
}

/// 意图状态结构体
///
/// 当前意图栈的状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentionState {
    pub stack: Vec<Intention>,
    pub active_goal_pointer: usize,
    pub execution_depth: usize,
}

/// 注意力状态结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionState {
    pub parameter: f64,
    pub mode: AttentionMode,
    pub focus_areas: Vec<String>,
}

/// 意图结构体
///
/// # 字段说明
///
/// - `created_at`: Unix 毫秒时间戳
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Intention {
    pub intention_id: String,
    pub goal: String,
    pub status: IntentionStatus,
    pub confidence: f64,
    pub created_at: i64,
}

/// 完整快照结构体
///
/// 包含元数据和所有子状态的完整系统快照
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullSnapshot {
    pub metadata: SnapshotMetadata,
    pub belief_state: BeliefState,
    pub intention_state: IntentionState,
    pub attention_state: AttentionState,
    pub memory_index: serde_json::Value,
}

/// 写入快照结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteSnapshotResult {
    pub snapshot_id: String,
    pub file_path: String,
    pub file_size: usize,
    pub compressed_size: usize,
    pub checksum: String,
    pub compression_ratio: f64,
    pub node_count: usize,
    pub edge_count: usize,
    pub write_duration_ms: i64,
}

/// 恢复快照结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSnapshotResult {
    pub snapshot: FullSnapshot,
    pub restored: bool,
    pub duration_ms: i64,
    pub target_state: StateTarget,
}

/// 原始日志条目结构体
///
/// # 字段说明
///
/// - `timestamp`: Unix 毫秒时间戳
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawLogEntry {
    pub log_id: String,
    pub session_id: String,
    pub log_type: LogType,
    pub timestamp: i64,
    pub sequence_number: u64,
    pub payload: serde_json::Value,
    pub references: LogReferences,
}

/// 写入日志结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteLogResult {
    pub log_id: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceMetadata {
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_snapshot_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_log_ids: Option<Vec<String>>,
    pub verification_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceUsageStats {
    pub access_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accessed_at: Option<i64>,
    pub verification_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceRelationships {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_experience_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contradicts_experience_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refines_experience_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextConstraints {
    pub current_domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_belief_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_topics: Option<Vec<String>>,
    pub max_recency_days: Option<u32>,
}

/// 经验内容结构体
///
/// 结构化的经验知识表示
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceContent {
    pub title: String,
    pub summary: String,
    pub domain: String,
    pub pattern: PatternType,
    pub confidence: f64,
    pub context: serde_json::Value,
    pub knowledge: serde_json::Value,
    pub outcomes: serde_json::Value,
}

/// 经验结构体
///
/// 长期存储的结构化经验
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Experience {
    pub experience_id: String,
    pub metadata: ExperienceMetadata,
    pub content: ExperienceContent,
    pub usage_stats: ExperienceUsageStats,
    pub relationships: ExperienceRelationships,
}

/// 写入经验结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteExperienceResult {
    pub experience_id: String,
    pub verified: bool,
    pub timestamp: i64,
}

/// 清理策略结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPolicy {
    pub cleanup_type: CleanupType,
    pub scope: CleanupScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_days: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_importance: Option<f64>,
    #[serde(default)]
    pub dry_run: bool,
}

/// 清理错误结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupError {
    pub entry_id: String,
    pub error_code: String,
    pub error_message: String,
}

/// 清理统计结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupStatistics {
    pub scanned_entries: usize,
    pub deleted_entries: usize,
    pub archived_entries: usize,
    pub freed_space_bytes: usize,
    pub execution_duration_ms: i64,
}

/// 清理结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub cleanup_id: String,
    pub cleanup_type: CleanupType,
    pub scope: CleanupScope,
    pub status: CleanupStatus,
    pub statistics: CleanupStatistics,
    pub errors: Vec<CleanupError>,
    pub start_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
}

/// 存储统计结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub total_entries: usize,
    pub raw_log_count: usize,
    pub snapshot_count: usize,
    pub experience_count: usize,
    pub total_size_bytes: usize,
}

/// 记忆查询结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryQuery {
    pub topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_filter: Option<Vec<MemoryLayer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range_start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range_end: Option<i64>,
    #[serde(default)]
    pub include_raw_logs: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_layer_serialization() {
        let layer = MemoryLayer::RawLog;
        let json = serde_json::to_string(&layer).unwrap();
        assert_eq!(json, "\"RAW_LOG\"");

        let deserialized: MemoryLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, MemoryLayer::RawLog);
    }

    #[test]
    fn test_log_type_serialization() {
        let log_type = LogType::ToolCall;
        let json = serde_json::to_string(&log_type).unwrap();
        assert_eq!(json, "\"TOOL_CALL\"");

        let deserialized: LogType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, LogType::ToolCall);
    }

    #[test]
    fn test_snapshot_type_variants() {
        let variants = vec![
            SnapshotType::Manual,
            SnapshotType::Automatic,
            SnapshotType::Scheduled,
            SnapshotType::ErrorRecovery,
            SnapshotType::SessionEnd,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: SnapshotType = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, variant);
        }
    }

    #[test]
    fn test_compression_type_default() {
        assert_eq!(CompressionType::default(), CompressionType::Lz4);
    }

    #[test]
    fn test_retrieval_depth_default() {
        assert_eq!(RetrievalDepth::default(), RetrievalDepth::Standard);
    }

    #[test]
    fn test_attention_mode_serialization() {
        let mode = AttentionMode::HighResolution;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"HIGH_RESOLUTION\"");
    }

    #[test]
    fn test_intention_status_variants() {
        let variants = vec![
            IntentionStatus::Active,
            IntentionStatus::Suspended,
            IntentionStatus::Completed,
            IntentionStatus::Abandoned,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: IntentionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, variant);
        }
    }

    #[test]
    fn test_problem_type_serialization() {
        let pt = ProblemType::BeliefConflict;
        let json = serde_json::to_string(&pt).unwrap();
        assert_eq!(json, "\"BELIEF_CONFLICT\"");
    }

    #[test]
    fn test_cleanup_scope_serialization() {
        let scope = CleanupScope::AllLayersPlusDeep;
        let json = serde_json::to_string(&scope).unwrap();
        assert_eq!(json, "\"ALL_LAYERS_PLUS_DEEP\"");
    }

    #[test]
    fn test_memory_loading_state_flow() {
        let states = vec![
            MemoryLoadingState::Idle,
            MemoryLoadingState::Triggered,
            MemoryLoadingState::Retrieving,
            MemoryLoadingState::Filtering,
            MemoryLoadingState::Transforming,
            MemoryLoadingState::Integrating,
            MemoryLoadingState::Completed,
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let deserialized: MemoryLoadingState = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, state);
        }
    }

    #[test]
    fn test_source_reference_creation() {
        let sr = SourceReference {
            ref_type: "log".to_string(),
            ref_id: "log-123".to_string(),
            ref_path: Some("/logs/session-1".to_string()),
        };
        assert_eq!(sr.ref_type, "log");
        assert_eq!(sr.ref_id, "log-123");
        assert!(sr.ref_path.is_some());
    }

    #[test]
    fn test_source_reference_without_path() {
        let sr = SourceReference {
            ref_type: "snapshot".to_string(),
            ref_id: "snap-456".to_string(),
            ref_path: None,
        };
        let json = serde_json::to_string(&sr).unwrap();
        assert!(!json.contains("refPath"));
    }

    #[test]
    fn test_pagination_info() {
        let pi = PaginationInfo {
            offset: 0,
            limit: 20,
            total_count: 100,
            has_more: true,
        };
        assert_eq!(pi.offset, 0);
        assert!(pi.has_more);
    }

    #[test]
    fn test_search_metadata() {
        let sm = SearchMetadata {
            search_duration_ms: 15,
            indexes_used: vec!["belief_index".to_string(), "experience_index".to_string()],
            cache_hit: false,
        };
        assert_eq!(sm.search_duration_ms, 15);
        assert_eq!(sm.indexes_used.len(), 2);
        assert!(!sm.cache_hit);
    }

    #[test]
    fn test_memory_entry_serialization() {
        let entry = MemoryEntry {
            entry_id: "entry-test-001".to_string(),
            layer: MemoryLayer::Experience,
            memory_type: "pattern".to_string(),
            relevance_score: 0.85,
            confidence: 0.92,
            importance: 0.8,
            recency_score: 0.9,
            summary: "Test entry".to_string(),
            content: serde_json::json!({"key": "value"}),
            source_references: vec![],
            created_at: Utc::now(),
            access_count: 3,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.entry_id, entry.entry_id);
        assert_eq!(deserialized.layer, MemoryLayer::Experience);
        assert_eq!(deserialized.access_count, 3);
    }

    #[test]
    fn test_memory_entry_camel_case() {
        let entry = MemoryEntry {
            entry_id: "test".to_string(),
            layer: MemoryLayer::RawLog,
            memory_type: "dialogue".to_string(),
            relevance_score: 0.5,
            confidence: 0.7,
            importance: 0.6,
            recency_score: 0.8,
            summary: String::new(),
            content: serde_json::Value::Null,
            source_references: vec![],
            created_at: Utc::now(),
            access_count: 0,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("entryId"));
        assert!(json.contains("memoryType"));
        assert!(json.contains("relevanceScore"));
        assert!(json.contains("sourceReferences"));
        assert!(json.contains("accessCount"));
    }

    #[test]
    fn test_retrieval_result() {
        let result = RetrievalResult {
            request_id: "req-001".to_string(),
            query_topic: "tool_usage".to_string(),
            total_matches: 5,
            results: vec![],
            pagination: PaginationInfo {
                offset: 0,
                limit: 10,
                total_count: 5,
                has_more: false,
            },
            search_metadata: SearchMetadata {
                search_duration_ms: 12,
                indexes_used: vec!["primary".to_string()],
                cache_hit: true,
            },
        };
        assert_eq!(result.total_matches, 5);
        assert!(result.search_metadata.cache_hit);
    }

    #[test]
    fn test_confidence_gap() {
        let gap = ConfidenceGap {
            topic: "rust_ownership".to_string(),
            required_confidence: 0.9,
            current_confidence: 0.5,
            urgency: GapUrgency::High,
        };
        assert_eq!(gap.urgency, GapUrgency::High);
        assert!((gap.required_confidence - gap.current_confidence - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_structured_assertion() {
        let assertion = StructuredAssertion {
            assertion_type: "capability".to_string(),
            subject_id: "tool-x".to_string(),
            predicate: "supports".to_string(),
            object_value: serde_json::json!("async_execution"),
            confidence: 0.88,
            source: "experience".to_string(),
        };
        let json = serde_json::to_string(&assertion).unwrap();
        let deserialized: StructuredAssertion = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.assertion_type, "capability");
        assert_eq!(deserialized.confidence, 0.88);
    }

    #[test]
    fn test_integration_suggestion_with_optional_fields() {
        let suggestion = IntegrationSuggestion {
            target_node_id: Some("node-1".to_string()),
            action: "merge".to_string(),
            priority: 3,
            conflict_notes: Some("potential overlap".to_string()),
        };
        let json = serde_json::to_string(&suggestion).unwrap();
        assert!(json.contains("targetNodeId"));
        assert!(json.contains("conflictNotes"));
    }

    #[test]
    fn test_integration_suggestion_without_optional_fields() {
        let suggestion = IntegrationSuggestion {
            target_node_id: None,
            action: "create".to_string(),
            priority: 1,
            conflict_notes: None,
        };
        let json = serde_json::to_string(&suggestion).unwrap();
        assert!(!json.contains("targetNodeId"));
        assert!(!json.contains("conflictNotes"));
    }

    #[test]
    fn test_knowledge_bundle() {
        let bundle = KnowledgeBundle {
            bundle_id: "bundle-001".to_string(),
            source_experience_ids: vec!["exp-1".to_string()],
            source_snapshot_ids: vec![],
            structured_assertions: vec![],
            integration_suggestions: vec![],
        };
        assert_eq!(bundle.bundle_id, "bundle-001");
        assert_eq!(bundle.source_experience_ids.len(), 1);
    }

    #[test]
    fn test_contextual_retrieval_result() {
        let result = ContextualRetrievalResult {
            request_id: "req-002".to_string(),
            identified_gaps: vec![ConfidenceGap {
                topic: "api_design".to_string(),
                required_confidence: 0.8,
                current_confidence: 0.3,
                urgency: GapUrgency::Medium,
            }],
            retrieved_knowledge: vec![],
            confidence_predictions: {
                let mut map = HashMap::new();
                map.insert("api_design".to_string(), 0.75);
                map
            },
            confidence_improvement_estimate: 0.45,
        };
        assert_eq!(result.identified_gaps.len(), 1);
        assert_eq!(result.confidence_predictions.get("api_design"), Some(&0.75));
    }

    #[test]
    fn test_similar_problem_case() {
        let case = SimilarProblemCase {
            problem_id: "prob-001".to_string(),
            problem_description: "Tool timeout".to_string(),
            similarity_score: 0.82,
            resolution_steps: vec!["Retry with backoff".to_string()],
            outcome: ProblemOutcome::Success,
            resolution_context: "network_issue".to_string(),
        };
        assert_eq!(case.outcome, ProblemOutcome::Success);
        assert_eq!(case.similarity_score, 0.82);
    }

    #[test]
    fn test_solution_step() {
        let step = SolutionStep {
            step_number: 1,
            action_description: "Check network".to_string(),
            expected_outcome: "Connection restored".to_string(),
            adaptation_guidance: "Increase timeout if needed".to_string(),
            confidence: 0.9,
        };
        assert_eq!(step.step_number, 1);
    }

    #[test]
    fn test_problem_retrieval_result() {
        let result = ProblemRetrievalResult {
            request_id: "req-003".to_string(),
            original_problem: "Build failure".to_string(),
            inferred_problem_type: ProblemType::ToolExecutionFailure,
            similar_problems: vec![],
            recommended_steps: vec![],
            adaptation_notes: vec!["Check dependencies".to_string()],
            confidence: 0.7,
        };
        assert_eq!(
            result.inferred_problem_type,
            ProblemType::ToolExecutionFailure
        );
    }

    #[test]
    fn test_snapshot_metadata() {
        let meta = SnapshotMetadata {
            snapshot_id: "snap-001".to_string(),
            session_id: "sess-001".to_string(),
            version: "1.0".to_string(),
            snapshot_type: SnapshotType::Automatic,
            agent_id: "agent-1".to_string(),
            trigger: SnapshotTrigger {
                event_type: "scheduled".to_string(),
                event_id: None,
                description: "Scheduled snapshot".to_string(),
            },
            created_at: 1700000000000_i64,
            checksum: None,
            compression_ratio: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: SnapshotMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.snapshot_id, "snap-001");
        assert_eq!(deserialized.snapshot_type, SnapshotType::Automatic);
    }

    #[test]
    fn test_intention() {
        let intention = Intention {
            intention_id: "int-001".to_string(),
            goal: "Fix build error".to_string(),
            status: IntentionStatus::Active,
            confidence: 0.85,
            created_at: 1700000000000_i64,
        };
        assert_eq!(intention.status, IntentionStatus::Active);
    }

    #[test]
    fn test_intention_state() {
        let state = IntentionState {
            stack: vec![Intention {
                intention_id: "int-001".to_string(),
                goal: "Refactor module".to_string(),
                status: IntentionStatus::Active,
                confidence: 0.9,
                created_at: 1700000000000_i64,
            }],
            active_goal_pointer: 0,
            execution_depth: 1,
        };
        assert_eq!(state.stack.len(), 1);
        assert_eq!(state.active_goal_pointer, 0);
    }

    #[test]
    fn test_attention_state() {
        let state = AttentionState {
            parameter: 0.8,
            mode: AttentionMode::HighResolution,
            focus_areas: vec!["error_handling".to_string()],
        };
        assert_eq!(state.mode, AttentionMode::HighResolution);
    }

    #[test]
    fn test_raw_log_entry() {
        let entry = RawLogEntry {
            log_id: "log-001".to_string(),
            session_id: "sess-001".to_string(),
            log_type: LogType::Dialogue,
            timestamp: 1700000000000_i64,
            sequence_number: 42,
            payload: serde_json::json!({"message": "hello"}),
            references: LogReferences {
                related_prediction_id: None,
                related_belief_node_ids: None,
                parent_log_id: None,
            },
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: RawLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.log_type, LogType::Dialogue);
        assert_eq!(deserialized.sequence_number, 42);
    }

    #[test]
    fn test_write_log_result() {
        let result = WriteLogResult {
            log_id: "log-001".to_string(),
            timestamp: 1700000000000_i64,
        };
        assert_eq!(result.log_id, "log-001");
    }

    #[test]
    fn test_experience_content() {
        let content = ExperienceContent {
            title: "Error Recovery Pattern".to_string(),
            summary: "How to recover from tool failures".to_string(),
            domain: "error_handling".to_string(),
            pattern: PatternType::ErrorHandling,
            confidence: 0.85,
            context: serde_json::json!({}),
            knowledge: serde_json::json!({}),
            outcomes: serde_json::json!({}),
        };
        assert_eq!(content.pattern, PatternType::ErrorHandling);
    }

    #[test]
    fn test_experience() {
        let exp = Experience {
            experience_id: "exp-001".to_string(),
            metadata: ExperienceMetadata {
                source_type: "agent".to_string(),
                source_snapshot_ids: None,
                source_log_ids: None,
                verification_count: 1,
                last_used_at: None,
                tags: None,
            },
            content: ExperienceContent {
                title: "Test".to_string(),
                summary: String::new(),
                domain: "test".to_string(),
                pattern: PatternType::TaskPattern,
                confidence: 0.9,
                context: serde_json::Value::Null,
                knowledge: serde_json::Value::Null,
                outcomes: serde_json::Value::Null,
            },
            usage_stats: ExperienceUsageStats {
                access_count: 5,
                last_accessed_at: None,
                verification_count: 1,
            },
            relationships: ExperienceRelationships {
                related_experience_ids: None,
                contradicts_experience_ids: None,
                refines_experience_ids: None,
            },
        };
        let json = serde_json::to_string(&exp).unwrap();
        let deserialized: Experience = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.experience_id, "exp-001");
    }

    #[test]
    fn test_write_experience_result() {
        let result = WriteExperienceResult {
            experience_id: "exp-001".to_string(),
            verified: true,
            timestamp: 1700000000000_i64,
        };
        assert!(result.verified);
    }

    #[test]
    fn test_cleanup_policy() {
        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::RawLogOnly,
            max_age_days: Some(7),
            min_importance: Some(0.3),
            dry_run: false,
        };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("maxAgeDays"));
        assert!(json.contains("minImportance"));
    }

    #[test]
    fn test_cleanup_policy_dry_run() {
        let policy = CleanupPolicy {
            cleanup_type: CleanupType::Aggressive,
            scope: CleanupScope::AllLayers,
            max_age_days: None,
            min_importance: None,
            dry_run: true,
        };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(!json.contains("maxAgeDays"));
        assert!(!json.contains("minImportance"));
        assert!(json.contains("dryRun"));
    }

    #[test]
    fn test_cleanup_error() {
        let error = CleanupError {
            entry_id: "entry-001".to_string(),
            error_code: "PERMISSION_DENIED".to_string(),
            error_message: "Cannot delete protected entry".to_string(),
        };
        assert_eq!(error.error_code, "PERMISSION_DENIED");
    }

    #[test]
    fn test_cleanup_statistics() {
        let stats = CleanupStatistics {
            scanned_entries: 1000,
            deleted_entries: 150,
            archived_entries: 50,
            freed_space_bytes: 1024 * 1024,
            execution_duration_ms: 250,
        };
        assert_eq!(stats.scanned_entries, 1000);
        assert_eq!(stats.freed_space_bytes, 1048576);
    }

    #[test]
    fn test_cleanup_result() {
        let result = CleanupResult {
            cleanup_id: "cleanup-001".to_string(),
            cleanup_type: CleanupType::Standard,
            scope: CleanupScope::AllLayers,
            status: CleanupStatus::Completed,
            statistics: CleanupStatistics {
                scanned_entries: 500,
                deleted_entries: 50,
                archived_entries: 10,
                freed_space_bytes: 512000,
                execution_duration_ms: 100,
            },
            errors: vec![],
            start_time: 1700000000000_i64,
            end_time: Some(1700000000100_i64),
        };
        assert_eq!(result.status, CleanupStatus::Completed);
        assert!(result.end_time.is_some());
    }

    #[test]
    fn test_cleanup_result_without_end_time() {
        let result = CleanupResult {
            cleanup_id: "cleanup-002".to_string(),
            cleanup_type: CleanupType::Manual,
            scope: CleanupScope::SnapshotOnly,
            status: CleanupStatus::InProgress,
            statistics: CleanupStatistics {
                scanned_entries: 200,
                deleted_entries: 0,
                archived_entries: 0,
                freed_space_bytes: 0,
                execution_duration_ms: 50,
            },
            errors: vec![],
            start_time: 1700000000000_i64,
            end_time: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("endTime"));
    }

    #[test]
    fn test_storage_stats() {
        let stats = StorageStats {
            total_entries: 10000,
            raw_log_count: 8000,
            snapshot_count: 1500,
            experience_count: 500,
            total_size_bytes: 1024 * 1024 * 512,
        };
        assert_eq!(stats.total_entries, 10000);
    }

    #[test]
    fn test_memory_query() {
        let query = MemoryQuery {
            topic: "error_handling".to_string(),
            confidence_threshold: Some(0.7),
            layer_filter: Some(vec![MemoryLayer::Experience]),
            time_range_start: Some(1700000000000_i64),
            time_range_end: None,
            include_raw_logs: false,
        };
        let json = serde_json::to_string(&query).unwrap();
        let deserialized: MemoryQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.topic, "error_handling");
        assert_eq!(deserialized.confidence_threshold, Some(0.7));
        assert!(!deserialized.include_raw_logs);
    }

    #[test]
    fn test_memory_query_minimal() {
        let query = MemoryQuery {
            topic: "test".to_string(),
            confidence_threshold: None,
            layer_filter: None,
            time_range_start: None,
            time_range_end: None,
            include_raw_logs: false,
        };
        let json = serde_json::to_string(&query).unwrap();
        assert!(!json.contains("confidenceThreshold"));
        assert!(!json.contains("layerFilter"));
        assert!(!json.contains("timeRangeStart"));
        assert!(!json.contains("timeRangeEnd"));
    }

    #[test]
    fn test_write_snapshot_result() {
        let result = WriteSnapshotResult {
            snapshot_id: "snap-001".to_string(),
            file_path: "/data/snapshots/snap-001.bin".to_string(),
            file_size: 4096,
            compressed_size: 2048,
            checksum: "abc123".to_string(),
            compression_ratio: 0.5,
            node_count: 100,
            edge_count: 300,
            write_duration_ms: 45,
        };
        assert!((result.compression_ratio - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_all_enums_roundtrip() {
        let layers = vec![
            MemoryLayer::RawLog,
            MemoryLayer::Snapshot,
            MemoryLayer::Experience,
        ];
        for layer in &layers {
            let json = serde_json::to_string(layer).unwrap();
            let de: MemoryLayer = serde_json::from_str(&json).unwrap();
            assert_eq!(de, *layer);
        }

        let problem_types = vec![
            ProblemType::ToolExecutionFailure,
            ProblemType::PredictionMismatch,
            ProblemType::BeliefConflict,
            ProblemType::GoalAmbiguity,
            ProblemType::ResourceConstraint,
            ProblemType::Unknown,
        ];
        for pt in &problem_types {
            let json = serde_json::to_string(pt).unwrap();
            let de: ProblemType = serde_json::from_str(&json).unwrap();
            assert_eq!(de, *pt);
        }

        let pattern_types = vec![
            PatternType::ErrorHandling,
            PatternType::TaskPattern,
            PatternType::ToolSequence,
            PatternType::BeliefCorrection,
            PatternType::GoalDecomposition,
        ];
        for pt in &pattern_types {
            let json = serde_json::to_string(pt).unwrap();
            let de: PatternType = serde_json::from_str(&json).unwrap();
            assert_eq!(de, *pt);
        }
    }
}
