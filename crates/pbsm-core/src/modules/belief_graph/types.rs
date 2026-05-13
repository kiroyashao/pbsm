//! 信念图管理器核心数据类型定义
//!
//! 本模块定义了信念图管理器（M1）的核心数据结构，包括：
//! - 信念节点：代表 Agent 对某个实体或概念的当前理解
//! - 关系边：描述信念节点之间的语义关联
//! - 查询规格：支持多种查询模式和过滤条件
//! - 快照结构：信念图的版本快照
//!
//! # 数据模型设计
//!
//! 信念图采用混合数据结构表示，以支持高效的查询和更新操作：
//! - 节点存储：HashMap<BeliefId, BeliefNode>
//! - 邻接表：HashMap<BeliefId, Vec<(EdgeId, BeliefId)>>
//! - 辅助索引：type_index, tag_index, name_index, confidence_index
//!
//! # 性能约束
//!
//! - 最大节点数：500（默认）
//! - 最大边数：2000（默认）
//! - 查询响应时间：≤10ms
//! - 更新响应时间：≤20ms

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// 信念节点唯一标识符
pub type BeliefId = Uuid;

/// 关系边唯一标识符
pub type EdgeId = Uuid;

/// 快照唯一标识符
pub type SnapshotId = Uuid;

/// 信念节点类型枚举
///
/// # 类型说明
///
/// |    类型   |     描述     |          典型属性             |
/// |----------|--------------|-----------------------------|
/// | User     | 用户实体      | name, role, permissions     |
/// | File     | 文件或文档     | path, content, status       |
/// | Tool     | 工具或服务     | name, version, capabilities |
/// | Variable | 变量或参数     | name, type, value           |
/// | Concept  | 抽象概念       | name, definition            |
/// | Event    | 事件或状态变更  | type, timestamp             |
/// | Agent    | 其他Agent     | id, capabilities            |
/// | Resource | 系统资源       | type, capacity              |
/// | Process  | 进程或任务     | name, status                |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BeliefNodeType {
    User,
    File,
    Tool,
    Variable,
    Concept,
    Event,
    Agent,
    Resource,
    Process,
}

/// 信念来源类型枚举
///
/// # 置信度默认值
///
/// | 类型 | 默认置信度 | 说明 |
/// |------|-----------|------|
/// | DirectObservation | 0.9 | 直接观察到的信息 |
/// | ToolReturn | 0.8 | 工具返回的数据 |
/// | UserInput | 0.7 | 用户显式提供的信息 |
/// | Derived | 0.5 | 从其他信念推导得出 |
/// | MemoryRestore | 0.6 | 从外部记忆恢复 |
/// | AgentSync | 0.65 | 从其他Agent同步 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceType {
    DirectObservation,
    ToolReturn,
    UserInput,
    Derived,
    MemoryRestore,
    AgentSync,
}

impl SourceType {
    /// 获取该来源类型的默认置信度
    pub fn default_confidence(&self) -> f64 {
        match self {
            SourceType::DirectObservation => 0.9,
            SourceType::ToolReturn => 0.8,
            SourceType::UserInput => 0.7,
            SourceType::Derived => 0.5,
            SourceType::MemoryRestore => 0.6,
            SourceType::AgentSync => 0.65,
        }
    }
}

/// 信念重要性等级枚举
///
/// # 等级说明
///
/// - **Critical**: 关键信念，影响核心决策
/// - **High**: 高重要性，频繁使用
/// - **Medium**: 中等重要性（默认）
/// - **Low**: 低重要性，可优先遗忘
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum ImportanceLevel {
    Critical,
    High,
    #[default]
    Medium,
    Low,
}

/// 属性值结构体
///
/// # 字段说明
///
/// - `value`: 属性值，支持 string/number/boolean/object/array
/// - `confidence`: 对该属性值的置信度，0.0-1.0
/// - `last_updated`: 该属性最后一次更新的时间
/// - `source`: 属性来源的标识符
/// - `source_type`: 来源类型，影响置信度默认值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeValue {
    pub value: serde_json::Value,
    pub confidence: f64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_updated: DateTime<Utc>,
    pub source: String,
    pub source_type: SourceType,
}

impl AttributeValue {
    /// 创建新的属性值
    ///
    /// # 参数
    /// * `value` - 属性值
    /// * `confidence` - 置信度
    /// * `source` - 来源标识
    /// * `source_type` - 来源类型
    pub fn new(
        value: serde_json::Value,
        confidence: f64,
        source: String,
        source_type: SourceType,
    ) -> Self {
        Self {
            value,
            confidence,
            last_updated: Utc::now(),
            source,
            source_type,
        }
    }
}

/// 节点元数据结构体
///
/// # 字段说明
///
/// - `version`: 节点版本号，每次修改后递增
/// - `created_at`: 节点创建时间
/// - `last_modified`: 最后一次修改时间
/// - `tags`: 主题标签列表，最多10个
/// - `importance`: 重要性等级
/// - `owner_agent_id`: 创建该节点的Agent标识
/// - `is_active`: 是否为活跃信念
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub version: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_modified: DateTime<Utc>,
    pub tags: Vec<String>,
    pub importance: ImportanceLevel,
    pub owner_agent_id: Option<String>,
    pub is_active: bool,
}

impl Default for NodeMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            last_modified: now,
            tags: Vec::new(),
            importance: ImportanceLevel::default(),
            owner_agent_id: None,
            is_active: true,
        }
    }
}

/// 信念节点结构体
///
/// # 设计说明
///
/// 信念节点是信念图的基本存储单元，代表 Agent 对某个实体或概念的当前理解。
/// 每个节点包含实体的属性信息和与其他节点的关联关系。
///
/// # 约束限制
///
/// - 名称长度：1-64字符
/// - 属性数量：最多50个
/// - 标签数量：最多10个
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefNode {
    pub node_id: BeliefId,
    pub node_type: BeliefNodeType,
    pub name: String,
    pub description: Option<String>,
    pub attributes: HashMap<String, AttributeValue>,
    pub outgoing_edges: Vec<EdgeId>,
    pub incoming_edges: Vec<EdgeId>,
    pub metadata: NodeMetadata,
}

impl BeliefNode {
    /// 创建新的信念节点
    ///
    /// # 参数
    /// * `node_type` - 节点类型
    /// * `name` - 节点名称
    /// * `_source` - 来源标识
    /// * `_source_type` - 来源类型
    pub fn new(
        node_type: BeliefNodeType,
        name: String,
        _source: String,
        _source_type: SourceType,
    ) -> Self {
        Self {
            node_id: Uuid::new_v4(),
            node_type,
            name,
            description: None,
            attributes: HashMap::new(),
            outgoing_edges: Vec::new(),
            incoming_edges: Vec::new(),
            metadata: NodeMetadata::default(),
        }
    }

    /// 设置节点属性
    pub fn with_attributes(mut self, attributes: HashMap<String, AttributeValue>) -> Self {
        self.attributes = attributes;
        self
    }

    /// 设置节点描述
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// 设置节点标签
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.metadata.tags = tags;
        self
    }

    /// 设置节点重要性
    pub fn with_importance(mut self, importance: ImportanceLevel) -> Self {
        self.metadata.importance = importance;
        self
    }

    /// 计算节点平均置信度
    ///
    /// # 返回
    /// 所有属性置信度的平均值，若无属性则返回0.5
    pub fn average_confidence(&self) -> f64 {
        if self.attributes.is_empty() {
            return 0.5;
        }
        let sum: f64 = self.attributes.values().map(|a| a.confidence).sum();
        sum / self.attributes.len() as f64
    }
}

/// 关系边类型枚举
///
/// # 类型说明
///
/// | 类型 | 描述 | 语义方向 |
/// |------|------|---------|
/// | Owns | 拥有关系 | source拥有target |
/// | DependsOn | 依赖关系 | source依赖target |
/// | Authorizes | 授权关系 | source授权target |
/// | Calls | 调用关系 | source调用target |
/// | Contains | 包含关系 | source包含target |
/// | RelatedTo | 关联关系 | source与target相关 |
/// | Enables | 启用关系 | source启用target |
/// | Blocks | 阻塞关系 | source阻塞target |
/// | Modifies | 修改关系 | source修改target |
/// | References | 引用关系 | source引用target |
/// | Precedes | 前置关系 | source在target之前 |
/// | Follows | 后续关系 | source在target之后 |
/// | SynchronizesWith | 同步关系 | source与target同步 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationEdgeType {
    Owns,
    DependsOn,
    Authorizes,
    Calls,
    Contains,
    RelatedTo,
    Enables,
    Blocks,
    Modifies,
    References,
    Precedes,
    Follows,
    SynchronizesWith,
}

/// 边来源类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeSourceType {
    Explicit,
    Inferred,
    Temporary,
}

/// 边元数据结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeMetadata {
    pub version: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_modified: DateTime<Utc>,
    pub source: String,
    pub source_type: EdgeSourceType,
    pub is_directional: bool,
}

impl Default for EdgeMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            last_modified: now,
            source: String::new(),
            source_type: EdgeSourceType::Explicit,
            is_directional: true,
        }
    }
}

/// 关系边结构体
///
/// # 设计说明
///
/// 关系边描述信念节点之间的语义关联，是图结构的核心组成部分。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationEdge {
    pub edge_id: EdgeId,
    pub edge_type: RelationEdgeType,
    pub source_node: BeliefId,
    pub target_node: BeliefId,
    pub attributes: HashMap<String, serde_json::Value>,
    pub confidence: f64,
    pub metadata: EdgeMetadata,
}

impl RelationEdge {
    /// 创建新的关系边
    pub fn new(
        edge_type: RelationEdgeType,
        source_node: BeliefId,
        target_node: BeliefId,
        confidence: f64,
    ) -> Self {
        Self {
            edge_id: Uuid::new_v4(),
            edge_type,
            source_node,
            target_node,
            attributes: HashMap::new(),
            confidence,
            metadata: EdgeMetadata::default(),
        }
    }
}

/// 信念更新策略枚举
///
/// # 策略说明
///
/// | 策略 | 描述 | 适用场景 |
/// |------|------|---------|
/// | Overwrite | 完全覆盖原值 | 确定性新信息 |
/// | IncrementalMerge | 增量合并 | 局部更新 |
/// | ConditionalReplace | 条件替换 | 仅当新值置信度更高时 |
/// | ConservativeMerge | 保守合并 | 保留所有历史值 |
/// | AverageBlend | 平均融合 | 多个可信来源 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum UpdateStrategy {
    Overwrite,
    IncrementalMerge,
    #[default]
    ConditionalReplace,
    ConservativeMerge,
    AverageBlend,
}

/// 查询类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueryType {
    ExactById,
    ByType,
    ByTag,
    ByName,
    ByAttribute,
    ByConfidenceRange,
    ByRelation,
    ByTimeRange,
    GraphTraversal,
}

/// 比较操作符枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
}

/// 边方向枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeDirection {
    Outgoing,
    Incoming,
    Both,
}

/// 查询规格结构体
///
/// # 字段说明
///
/// 支持多种查询条件的组合：
/// - 按ID精确查询
/// - 按类型/标签/名称查询
/// - 按属性值/置信度范围查询
/// - 按时间范围查询
/// - 图遍历查询
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuerySpecification {
    pub query_type: QueryType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<BeliefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_type: Option<BeliefNodeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_pattern: Option<String>,
    #[serde(default)]
    pub fuzzy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<ComparisonOperator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<BeliefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_type: Option<RelationEdgeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<EdgeDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_node_id: Option<BeliefId>,
    #[serde(default)]
    pub max_depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traversal_edge_types: Option<Vec<RelationEdgeType>>,
}

impl Default for QuerySpecification {
    fn default() -> Self {
        Self {
            query_type: QueryType::ExactById,
            node_id: None,
            node_type: None,
            tag: None,
            name_pattern: None,
            fuzzy: false,
            attribute_key: None,
            attribute_value: None,
            operator: None,
            min_confidence: None,
            max_confidence: None,
            source_id: None,
            edge_type: None,
            direction: None,
            start_time: None,
            end_time: None,
            start_node_id: None,
            max_depth: Some(3),
            traversal_edge_types: None,
        }
    }
}

/// 排序字段枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SortField {
    Relevance,
    Confidence,
    LastModified,
    Name,
}

/// 排序方向枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

fn default_limit() -> usize {
    50
}

/// 查询选项结构体
///
/// # 字段说明
///
/// - `limit`: 返回结果数量上限，默认50
/// - `offset`: 结果偏移量，用于分页
/// - `sort_by`: 排序字段
/// - `sort_order`: 排序方向
/// - `include_edges`: 是否包含关联边
/// - `include_metadata`: 是否包含完整元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOptions {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<SortField>,
    #[serde(default)]
    pub sort_order: SortOrder,
    #[serde(default)]
    pub include_edges: bool,
    #[serde(default)]
    pub include_metadata: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            limit: default_limit(),
            offset: 0,
            sort_by: None,
            sort_order: SortOrder::default(),
            include_edges: false,
            include_metadata: false,
        }
    }
}

/// 查询结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub items: Vec<BeliefNode>,
    pub total_count: usize,
    pub has_more: bool,
    pub execution_time_ms: f64,
}

/// 快照类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SnapshotType {
    Auto,
    Manual,
    Session,
    Sync,
    Fusion,
}

/// 冲突解决策略枚举
///
/// # 策略说明
///
/// | 策略 | 描述 |
/// |------|------|
/// | HighConfidenceWins | 高置信度优先 |
/// | SourcePriority | 来源优先级优先 |
/// | MostRecent | 最新优先 |
/// | TimeDecay | 时间衰减 |
/// | ManualReview | 人工审核 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum ResolutionStrategy {
    #[default]
    HighConfidenceWins,
    SourcePriority,
    MostRecent,
    TimeDecay,
    ManualReview,
}

/// 图配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphConfig {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub default_confidence: f64,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            max_nodes: 500,
            max_edges: 2000,
            default_confidence: 0.5,
        }
    }
}

/// 更新结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub success: bool,
    pub updated: bool,
    pub conflict_detected: bool,
    pub new_version: u64,
}

/// 删除结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub success: bool,
    pub deleted_node_id: BeliefId,
    pub deleted_edge_ids: Vec<EdgeId>,
}

/// 遍历结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraversalResult {
    pub visited_nodes: Vec<BeliefNode>,
    pub visited_edges: Vec<RelationEdge>,
    pub path_map: HashMap<BeliefId, Vec<EdgeId>>,
}

/// 推导步骤结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivationStep {
    pub node_id: BeliefId,
    pub edge_id: Option<EdgeId>,
    pub rule: String,
}

/// 推导结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivationResult {
    pub derived_node_id: BeliefId,
    pub derived_attributes: HashMap<String, AttributeValue>,
    pub confidence: f64,
    pub derivation_path: Vec<DerivationStep>,
    pub derivation_rule: String,
}

/// 快照元数据结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMetadata {
    pub snapshot_id: SnapshotId,
    pub version_number: u64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    pub snapshot_type: SnapshotType,
    pub trigger_reason: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub checksum: String,
}

/// 图快照结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSnapshot {
    pub metadata: SnapshotMetadata,
    pub nodes: Vec<BeliefNode>,
    pub edges: Vec<RelationEdge>,
}

/// 回滚结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackResult {
    pub success: bool,
    pub previous_version_id: Option<SnapshotId>,
    pub current_version_id: Option<SnapshotId>,
    pub nodes_created: Vec<BeliefId>,
    pub nodes_updated: Vec<BeliefId>,
    pub nodes_deleted: Vec<BeliefId>,
    pub invalidated_predictions: Vec<String>,
}

/// 融合统计结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionStatistics {
    pub nodes_processed: usize,
    pub nodes_created: usize,
    pub nodes_updated: usize,
    pub conflicts_resolved: usize,
    pub conflicts_deferred: usize,
}

/// 冲突记录结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictRecord {
    pub node_id: BeliefId,
    pub attribute: String,
    pub local_value: serde_json::Value,
    pub external_value: serde_json::Value,
    pub local_confidence: f64,
    pub external_confidence: f64,
}

/// 融合结果结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionResult {
    pub success: bool,
    pub statistics: Option<FusionStatistics>,
    pub deferred_conflicts: Option<Vec<ConflictRecord>>,
}

/// 信念快照结构体（用于跨Agent通信）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeliefSnapshot {
    pub nodes: Vec<BeliefNode>,
    pub edges: Vec<RelationEdge>,
}

/// 融合配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<ResolutionStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_callback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserve_local_metadata: Option<bool>,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            strategy: Some(ResolutionStrategy::default()),
            conflict_callback: None,
            preserve_local_metadata: Some(true),
        }
    }
}

/// 图统计信息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStatistics {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_type: HashMap<BeliefNodeType, usize>,
    pub edges_by_type: HashMap<RelationEdgeType, usize>,
    pub average_confidence: f64,
    pub high_confidence_count: usize,
    pub low_confidence_count: usize,
}

/// 置信度索引结构体
///
/// # 字段说明
///
/// - `high`: 高置信度节点集合（confidence >= 0.7）
/// - `medium`: 中置信度节点集合（0.4 <= confidence < 0.7）
/// - `low`: 低置信度节点集合（confidence < 0.4）
#[derive(Debug, Clone, Default)]
pub struct ConfidenceIndex {
    pub high: HashSet<BeliefId>,
    pub medium: HashSet<BeliefId>,
    pub low: HashSet<BeliefId>,
}

impl ConfidenceIndex {
    /// 根据置信度对节点进行分类
    pub fn classify(&self, _node_id: BeliefId, avg_confidence: f64) -> ConfidenceLevel {
        if avg_confidence >= 0.7 {
            ConfidenceLevel::High
        } else if avg_confidence >= 0.4 {
            ConfidenceLevel::Medium
        } else {
            ConfidenceLevel::Low
        }
    }
}

/// 置信度等级枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_type_default_confidence() {
        assert_eq!(SourceType::DirectObservation.default_confidence(), 0.9);
        assert_eq!(SourceType::ToolReturn.default_confidence(), 0.8);
        assert_eq!(SourceType::UserInput.default_confidence(), 0.7);
        assert_eq!(SourceType::Derived.default_confidence(), 0.5);
        assert_eq!(SourceType::MemoryRestore.default_confidence(), 0.6);
        assert_eq!(SourceType::AgentSync.default_confidence(), 0.65);
    }

    #[test]
    fn test_belief_node_creation() {
        let node = BeliefNode::new(
            BeliefNodeType::User,
            "Alice".to_string(),
            "test".to_string(),
            SourceType::UserInput,
        );
        assert_eq!(node.node_type, BeliefNodeType::User);
        assert_eq!(node.name, "Alice");
        assert_eq!(node.metadata.version, 1);
    }

    #[test]
    fn test_belief_node_average_confidence() {
        let mut node = BeliefNode::new(
            BeliefNodeType::File,
            "test.txt".to_string(),
            "test".to_string(),
            SourceType::DirectObservation,
        );

        assert_eq!(node.average_confidence(), 0.5);

        node.attributes.insert(
            "size".to_string(),
            AttributeValue::new(
                serde_json::json!(1024),
                0.9,
                "test".to_string(),
                SourceType::DirectObservation,
            ),
        );
        node.attributes.insert(
            "modified".to_string(),
            AttributeValue::new(
                serde_json::json!("2024-01-01"),
                0.7,
                "test".to_string(),
                SourceType::DirectObservation,
            ),
        );

        assert!((node.average_confidence() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_relation_edge_creation() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let edge = RelationEdge::new(RelationEdgeType::Owns, source, target, 0.9);

        assert_eq!(edge.edge_type, RelationEdgeType::Owns);
        assert_eq!(edge.source_node, source);
        assert_eq!(edge.target_node, target);
        assert_eq!(edge.confidence, 0.9);
    }

    #[test]
    fn test_update_strategy_default() {
        assert_eq!(
            UpdateStrategy::default(),
            UpdateStrategy::ConditionalReplace
        );
    }

    #[test]
    fn test_resolution_strategy_default() {
        assert_eq!(
            ResolutionStrategy::default(),
            ResolutionStrategy::HighConfidenceWins
        );
    }

    #[test]
    fn test_query_specification_default() {
        let spec = QuerySpecification::default();
        assert_eq!(spec.query_type, QueryType::ExactById);
        assert_eq!(spec.max_depth, Some(3));
    }

    #[test]
    fn test_graph_config_default() {
        let config = GraphConfig::default();
        assert_eq!(config.max_nodes, 500);
        assert_eq!(config.max_edges, 2000);
        assert_eq!(config.default_confidence, 0.5);
    }
}
