//! 信念图核心数据结构
//!
//! 本模块定义了信念图的核心数据结构，包括：
//! - 邻接表：存储节点之间的边关系
//! - 索引结构：加速各类查询操作
//! - BeliefGraph：主图结构
//!
//! # 数据结构设计
//!
//! 信念图采用混合数据结构表示，以支持高效的查询和更新操作：
//!
//! ## 节点存储
//! - 使用 `HashMap<BeliefId, BeliefNode>` 存储所有信念节点
//! - 支持 O(1) 的节点查找
//!
//! ## 邻接表
//! - 使用双向邻接表存储边的关系
//! - 支持 O(k) 的邻居节点查找（k为邻居数量）
//!
//! ## 辅助索引
//! - `type_index`: 按节点类型索引，加速类型查询
//! - `tag_index`: 按标签索引，加速标签查询
//! - `name_index`: 按名称索引，支持精确和模糊匹配
//! - `confidence_index`: 按置信度等级索引，加速置信度范围查询
//! - `time_index`: 按时间索引，支持时间范围查询

use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;

use super::types::{BeliefId, BeliefNode, EdgeId, RelationEdge};
use crate::modules::common::{
    BeliefGraphError, BeliefGraphReader, BeliefGraphWriter, BeliefQuerySpec, BeliefState as CommonBeliefState,
};

use super::types::*;

/// 邻接表结构体
///
/// # 设计说明
///
/// 邻接表用于存储图中节点之间的边关系，采用双向存储：
/// - `outgoing`: 从每个节点出发的边映射
/// - `incoming`: 指向每个节点的边映射
///
/// 这种双向存储设计支持：
/// - O(k) 的出边查找（k为出边数量）
/// - O(k) 的入边查找（k为入边数量）
/// - 高效的边删除操作
#[derive(Debug, Clone, Default)]
pub struct AdjacencyList {
    pub outgoing: HashMap<BeliefId, Vec<(EdgeId, BeliefId)>>,
    pub incoming: HashMap<BeliefId, Vec<(EdgeId, BeliefId)>>,
}

impl AdjacencyList {
    /// 创建新的邻接表
    pub fn new() -> Self {
        Self {
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
        }
    }

    /// 添加一条边到邻接表
    ///
    /// # 参数
    /// * `edge_id` - 边ID
    /// * `source` - 源节点ID
    /// * `target` - 目标节点ID
    pub fn add_edge(&mut self, edge_id: EdgeId, source: BeliefId, target: BeliefId) {
        self.outgoing
            .entry(source)
            .or_default()
            .push((edge_id, target));
        self.incoming
            .entry(target)
            .or_default()
            .push((edge_id, source));
    }

    /// 从邻接表移除一条边
    ///
    /// # 参数
    /// * `edge_id` - 边ID
    /// * `source` - 源节点ID
    /// * `target` - 目标节点ID
    pub fn remove_edge(&mut self, edge_id: EdgeId, source: BeliefId, target: BeliefId) {
        if let Some(edges) = self.outgoing.get_mut(&source) {
            edges.retain(|(eid, _)| *eid != edge_id);
        }
        if let Some(edges) = self.incoming.get_mut(&target) {
            edges.retain(|(eid, _)| *eid != edge_id);
        }
    }

    /// 获取节点的所有出边
    ///
    /// # 参数
    /// * `node_id` - 节点ID
    ///
    /// # 返回
    /// * `Vec<(EdgeId, BeliefId)>` - 边的ID和目标节点ID列表
    pub fn get_outgoing_edges(&self, node_id: BeliefId) -> Vec<(EdgeId, BeliefId)> {
        self.outgoing.get(&node_id).cloned().unwrap_or_default()
    }

    /// 获取节点的所有入边
    ///
    /// # 参数
    /// * `node_id` - 节点ID
    ///
    /// # 返回
    /// * `Vec<(EdgeId, BeliefId)>` - 边的ID和源节点ID列表
    pub fn get_incoming_edges(&self, node_id: BeliefId) -> Vec<(EdgeId, BeliefId)> {
        self.incoming.get(&node_id).cloned().unwrap_or_default()
    }

    /// 移除与节点相关的所有边信息
    ///
    /// # 参数
    /// * `node_id` - 节点ID
    pub fn remove_node(&mut self, node_id: BeliefId) {
        if let Some(outgoing) = self.outgoing.remove(&node_id) {
            let edge_ids_to_remove: HashSet<EdgeId> =
                outgoing.iter().map(|(eid, _)| *eid).collect();
            for (_, target) in outgoing {
                if let Some(incoming) = self.incoming.get_mut(&target) {
                    incoming.retain(|(eid, _)| !edge_ids_to_remove.contains(eid));
                }
            }
        }
        if let Some(incoming) = self.incoming.remove(&node_id) {
            let edge_ids_to_remove: HashSet<EdgeId> =
                incoming.iter().map(|(eid, _)| *eid).collect();
            for (_, source) in incoming {
                if let Some(outgoing) = self.outgoing.get_mut(&source) {
                    outgoing.retain(|(eid, _)| !edge_ids_to_remove.contains(eid));
                }
            }
        }
    }

    /// 清空邻接表
    pub fn clear(&mut self) {
        self.outgoing.clear();
        self.incoming.clear();
    }
}

/// 图索引结构体
///
/// # 索引类型说明
///
/// | 索引名称 | 索引键 | 用途 |
/// |---------|-------|------|
/// | type_index | node_type | 按类型快速检索节点 |
/// | tag_index | tag | 按标签快速检索节点 |
/// | name_index | name (normalized) | 按名称模糊匹配 |
/// | confidence_index | confidence | 按置信度范围检索 |
/// | time_index | lastModified | 按时间范围检索 |
#[derive(Debug, Clone, Default)]
pub struct GraphIndexes {
    pub type_index: HashMap<BeliefNodeType, HashSet<BeliefId>>,
    pub tag_index: HashMap<String, HashSet<BeliefId>>,
    pub name_index: HashMap<String, HashSet<BeliefId>>,
    pub confidence_index: ConfidenceIndex,
    pub time_index: Vec<BeliefId>,
    pub time_stamps: HashMap<BeliefId, chrono::DateTime<chrono::Utc>>,
}

impl GraphIndexes {
    /// 创建新的图索引
    pub fn new() -> Self {
        Self {
            type_index: HashMap::new(),
            tag_index: HashMap::new(),
            name_index: HashMap::new(),
            confidence_index: ConfidenceIndex::default(),
            time_index: Vec::new(),
            time_stamps: HashMap::new(),
        }
    }

    /// 添加节点到索引
    ///
    /// # 参数
    /// * `node` - 信念节点引用
    ///
    /// # 索引更新说明
    ///
    /// 添加节点时会同时更新以下索引：
    /// - type_index: 添加到对应类型的集合
    /// - tag_index: 添加到每个标签对应的集合
    /// - name_index: 使用规范化后的名称添加
    /// - confidence_index: 根据平均置信度添加到对应等级
    /// - time_index: 添加到时间索引列表末尾
    pub fn add_node(&mut self, node: &BeliefNode) {
        self.type_index
            .entry(node.node_type)
            .or_default()
            .insert(node.node_id);

        for tag in &node.metadata.tags {
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .insert(node.node_id);
        }

        let normalized_name = node.name.to_lowercase();
        self.name_index
            .entry(normalized_name)
            .or_default()
            .insert(node.node_id);

        let avg_conf = node.average_confidence();
        let level = self.confidence_index.classify(node.node_id, avg_conf);
        match level {
            ConfidenceLevel::High => self.confidence_index.high.insert(node.node_id),
            ConfidenceLevel::Medium => self.confidence_index.medium.insert(node.node_id),
            ConfidenceLevel::Low => self.confidence_index.low.insert(node.node_id),
        };

        self.time_stamps.insert(node.node_id, node.metadata.last_modified);
        self.time_index.push(node.node_id);
    }

    /// 从索引移除节点
    ///
    /// # 参数
    /// * `node` - 信念节点引用
    pub fn remove_node(&mut self, node: &BeliefNode) {
        if let Some(ids) = self.type_index.get_mut(&node.node_type) {
            ids.remove(&node.node_id);
        }

        for tag in &node.metadata.tags {
            if let Some(ids) = self.tag_index.get_mut(tag) {
                ids.remove(&node.node_id);
            }
        }

        let normalized_name = node.name.to_lowercase();
        if let Some(ids) = self.name_index.get_mut(&normalized_name) {
            ids.remove(&node.node_id);
        }

        self.confidence_index.high.remove(&node.node_id);
        self.confidence_index.medium.remove(&node.node_id);
        self.confidence_index.low.remove(&node.node_id);

        self.time_stamps.remove(&node.node_id);
        self.time_index.retain(|id| *id != node.node_id);
    }

    /// 更新节点的置信度索引
    ///
    /// # 参数
    /// * `node_id` - 节点ID
    /// * `old_confidence` - 更新前的置信度
    /// * `new_confidence` - 更新后的置信度
    ///
    /// # 说明
    ///
    /// 仅当置信度跨越等级边界时才移动节点：
    /// - High: confidence >= 0.7
    /// - Medium: 0.4 <= confidence < 0.7
    /// - Low: confidence < 0.4
    pub fn update_confidence(
        &mut self,
        node_id: BeliefId,
        old_confidence: f64,
        new_confidence: f64,
    ) {
        let old_level = if old_confidence >= 0.7 {
            ConfidenceLevel::High
        } else if old_confidence >= 0.4 {
            ConfidenceLevel::Medium
        } else {
            ConfidenceLevel::Low
        };

        let new_level = if new_confidence >= 0.7 {
            ConfidenceLevel::High
        } else if new_confidence >= 0.4 {
            ConfidenceLevel::Medium
        } else {
            ConfidenceLevel::Low
        };

        if old_level != new_level {
            match old_level {
                ConfidenceLevel::High => {
                    self.confidence_index.high.remove(&node_id);
                }
                ConfidenceLevel::Medium => {
                    self.confidence_index.medium.remove(&node_id);
                }
                ConfidenceLevel::Low => {
                    self.confidence_index.low.remove(&node_id);
                }
            }
            match new_level {
                ConfidenceLevel::High => {
                    self.confidence_index.high.insert(node_id);
                }
                ConfidenceLevel::Medium => {
                    self.confidence_index.medium.insert(node_id);
                }
                ConfidenceLevel::Low => {
                    self.confidence_index.low.insert(node_id);
                }
            }
        }
    }

    /// 按类型查询节点ID集合
    pub fn query_by_type(&self, node_type: BeliefNodeType) -> HashSet<BeliefId> {
        self.type_index.get(&node_type).cloned().unwrap_or_default()
    }

    /// 按标签查询节点ID集合
    pub fn query_by_tag(&self, tag: &str) -> HashSet<BeliefId> {
        self.tag_index.get(tag).cloned().unwrap_or_default()
    }

    /// 按名称精确查询节点ID集合
    ///
    /// # 参数
    /// * `name` - 节点名称（不区分大小写）
    pub fn query_by_name_exact(&self, name: &str) -> HashSet<BeliefId> {
        let normalized = name.to_lowercase();
        self.name_index
            .get(&normalized)
            .cloned()
            .unwrap_or_default()
    }

    /// 按置信度范围查询节点ID集合
    ///
    /// # 参数
    /// * `min` - 最小置信度（可选）
    /// * `max` - 最大置信度（可选）
    pub fn query_by_confidence_range(
        &self,
        min: Option<f64>,
        max: Option<f64>,
    ) -> HashSet<BeliefId> {
        let mut result = HashSet::new();
        let min_v = min.unwrap_or(0.0);
        let max_v = max.unwrap_or(1.0);

        if min_v < 0.4 && max_v > 0.0 {
            result.extend(&self.confidence_index.low);
        }
        if min_v < 0.7 && max_v >= 0.4 {
            result.extend(&self.confidence_index.medium);
        }
        if max_v >= 0.7 {
            result.extend(&self.confidence_index.high);
        }

        result
    }

    /// 查询高置信度节点ID集合（confidence >= 0.7）
    pub fn query_by_high_confidence(&self) -> HashSet<BeliefId> {
        self.confidence_index.high.clone()
    }

    /// 查询最近更新的节点
    ///
    /// # 参数
    /// * `hours` - 时间窗口（小时）
    pub fn query_recent(&self, hours: i64) -> Vec<BeliefId> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        self.time_index
            .iter()
            .filter(|id| {
                self.time_stamps
                    .get(id)
                    .map(|t| *t >= cutoff)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// 清空所有索引
    pub fn clear(&mut self) {
        self.type_index.clear();
        self.tag_index.clear();
        self.name_index.clear();
        self.confidence_index = ConfidenceIndex::default();
        self.time_index.clear();
        self.time_stamps.clear();
    }
}

/// 信念图主结构体
///
/// # 设计说明
///
/// BeliefGraph 是信念图管理器的核心数据结构，维护：
/// - 节点存储：所有信念节点
/// - 边存储：所有关系边
/// - 邻接表：节点间的边关系
/// - 索引结构：加速查询的辅助索引
/// - 快照历史：版本快照列表
/// - 配置信息：图的配置参数
/// - 版本号：图结构的版本
///
/// # 并发控制
///
/// 使用 `parking_lot::RwLock` 实现并发控制：
/// - 读锁：支持多个并发读操作
/// - 写锁：独占访问，用于修改操作
pub struct BeliefGraph {
    nodes: RwLock<HashMap<BeliefId, BeliefNode>>,
    edges: RwLock<HashMap<EdgeId, RelationEdge>>,
    adjacency: RwLock<AdjacencyList>,
    indexes: RwLock<GraphIndexes>,
    snapshots: RwLock<Vec<GraphSnapshot>>,
    config: GraphConfig,
    version: RwLock<u64>,
    event_publisher: Option<Arc<dyn crate::modules::common::BeliefGraphEventPublisher>>,
}

impl BeliefGraph {
    /// 使用指定配置创建信念图
    ///
    /// # 参数
    /// * `config` - 图配置
    pub fn new(config: GraphConfig) -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
            adjacency: RwLock::new(AdjacencyList::new()),
            indexes: RwLock::new(GraphIndexes::new()),
            snapshots: RwLock::new(Vec::new()),
            config,
            version: RwLock::new(0),
            event_publisher: None,
        }
    }

    pub fn with_event_publisher(
        config: GraphConfig,
        publisher: Arc<dyn crate::modules::common::BeliefGraphEventPublisher>,
    ) -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
            adjacency: RwLock::new(AdjacencyList::new()),
            indexes: RwLock::new(GraphIndexes::new()),
            snapshots: RwLock::new(Vec::new()),
            config,
            version: RwLock::new(0),
            event_publisher: Some(publisher),
        }
    }

    fn emit_event(&self, event: crate::modules::common::BeliefGraphEvent) {
        if let Some(ref publisher) = self.event_publisher {
            let _ = publisher.publish(event);
        }
    }

    pub fn publish_event(&self, event: crate::modules::common::BeliefGraphEvent) {
        self.emit_event(event);
    }

    /// 使用默认配置创建信念图
    ///
    /// # 默认配置说明
    ///
    /// - max_nodes: 500
    /// - max_edges: 2000
    /// - default_confidence: 0.5
    pub fn with_default_config() -> Self {
        Self::new(GraphConfig::default())
    }

    /// 根据ID获取信念节点
    ///
    /// # 参数
    /// * `node_id` - 节点ID
    ///
    /// # 返回
    /// * `Option<BeliefNode>` - 节点的克隆（如果存在）
    pub fn get_node(&self, node_id: BeliefId) -> Option<BeliefNode> {
        self.nodes.read().get(&node_id).cloned()
    }

    /// 根据ID获取关系边
    ///
    /// # 参数
    /// * `edge_id` - 边ID
    ///
    /// # 返回
    /// * `Option<RelationEdge>` - 边的克隆（如果存在）
    pub fn get_edge(&self, edge_id: EdgeId) -> Option<RelationEdge> {
        self.edges.read().get(&edge_id).cloned()
    }

    /// 获取当前节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.read().len()
    }

    /// 获取当前边数量
    pub fn edge_count(&self) -> usize {
        self.edges.read().len()
    }

    /// 获取当前图版本号
    pub fn version(&self) -> u64 {
        *self.version.read()
    }

    /// 获取图的配置引用
    pub fn config(&self) -> &GraphConfig {
        &self.config
    }

    /// 获取节点存储的读锁
    pub fn nodes(&self) -> &RwLock<HashMap<BeliefId, BeliefNode>> {
        &self.nodes
    }

    /// 获取边存储的读锁
    pub fn edges(&self) -> &RwLock<HashMap<EdgeId, RelationEdge>> {
        &self.edges
    }

    /// 获取邻接表的写锁
    pub fn adjacency_mut(&self) -> &RwLock<AdjacencyList> {
        &self.adjacency
    }

    /// 获取索引的写锁
    pub fn indexes_mut(&self) -> &RwLock<GraphIndexes> {
        &self.indexes
    }

    /// 获取版本号的写锁
    pub fn version_mut(&self) -> &RwLock<u64> {
        &self.version
    }

    /// 获取图的统计信息
    ///
    /// # 返回
    /// * `GraphStatistics` - 包含各种统计指标的結構体
    pub fn get_statistics(&self) -> GraphStatistics {
        let nodes = self.nodes.read();
        let edges = self.edges.read();

        let mut nodes_by_type: HashMap<BeliefNodeType, usize> = HashMap::new();
        let mut edges_by_type: HashMap<super::types::RelationEdgeType, usize> = HashMap::new();
        let mut total_confidence = 0.0;
        let mut high_count = 0;
        let mut low_count = 0;

        for node in nodes.values() {
            let avg_conf = node.average_confidence();
            total_confidence += avg_conf;
            if avg_conf >= 0.7 {
                high_count += 1;
            }
            if avg_conf < 0.4 {
                low_count += 1;
            }
            *nodes_by_type.entry(node.node_type).or_insert(0) += 1;
        }

        for edge in edges.values() {
            *edges_by_type.entry(edge.edge_type).or_insert(0) += 1;
        }

        let avg_conf = if !nodes.is_empty() {
            total_confidence / nodes.len() as f64
        } else {
            0.0
        };

        GraphStatistics {
            total_nodes: nodes.len(),
            total_edges: edges.len(),
            nodes_by_type,
            edges_by_type,
            average_confidence: avg_conf,
            high_confidence_count: high_count,
            low_confidence_count: low_count,
        }
    }

    /// 获取邻接表的克隆
    pub fn get_adjacency(&self) -> AdjacencyList {
        self.adjacency.read().clone()
    }

    /// 获取索引的克隆
    pub fn get_indexes(&self) -> GraphIndexes {
        self.indexes.read().clone()
    }

    /// 获取所有快照的克隆
    pub fn get_snapshots(&self) -> Vec<GraphSnapshot> {
        self.snapshots.read().clone()
    }

    pub fn snapshots(&self) -> parking_lot::RwLockReadGuard<'_, Vec<GraphSnapshot>> {
        self.snapshots.read()
    }

    /// 添加快照到历史
    ///
    /// # 参数
    /// * `snapshot` - 图快照
    pub fn add_snapshot(&self, snapshot: GraphSnapshot) {
        let mut snapshots = self.snapshots.write();
        snapshots.push(snapshot);
        let max = self.config.max_snapshots;
        while snapshots.len() > max {
            snapshots.remove(0);
        }
    }

    /// 根据ID获取快照
    ///
    /// # 参数
    /// * `snapshot_id` - 快照ID
    pub fn get_snapshot(&self, snapshot_id: SnapshotId) -> Option<GraphSnapshot> {
        self.snapshots
            .read()
            .iter()
            .find(|s| s.metadata.snapshot_id == snapshot_id)
            .cloned()
    }

    /// 清空图的所有数据
    ///
    /// # 说明
    ///
    /// 此操作会清空所有节点、边、邻接表、索引和快照，
    /// 并重置版本号为0。通常用于回滚操作。
    pub fn clear(&self) {
        self.nodes.write().clear();
        self.edges.write().clear();
        self.adjacency.write().clear();
        self.indexes.write().clear();
        self.snapshots.write().clear();
        *self.version.write() = 0;
    }
}

impl Default for BeliefGraph {
    fn default() -> Self {
        Self::with_default_config()
    }
}

fn convert_node(node: &BeliefNode) -> crate::modules::common::BeliefNode {
    let confidence = if node.attributes.is_empty() {
        1.0
    } else {
        node.attributes
            .values()
            .map(|av| av.confidence)
            .sum::<f64>()
            / node.attributes.len() as f64
    };
    let attribute_confidences: std::collections::HashMap<String, f64> = node
        .attributes
        .iter()
        .map(|(k, v)| (k.clone(), v.confidence))
        .collect();
    crate::modules::common::BeliefNode {
        node_id: node.node_id.to_string(),
        node_type: format!("{:?}", node.node_type),
        attributes: node
            .attributes
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect(),
        confidence,
        attribute_confidences,
        created_at: node.metadata.created_at,
        updated_at: node.metadata.last_modified,
    }
}

fn convert_edge(edge: &RelationEdge) -> crate::modules::common::RelationEdge {
    crate::modules::common::RelationEdge {
        edge_id: edge.edge_id,
        source_node: edge.source_node.to_string(),
        target_node: edge.target_node.to_string(),
        edge_type: format!("{:?}", edge.edge_type),
        confidence: edge.confidence,
    }
}

#[async_trait]
impl BeliefGraphReader for BeliefGraph {
    async fn query_belief_by_id(
        &self,
        node_id: &str,
    ) -> Result<Option<crate::modules::common::BeliefNode>, BeliefGraphError> {
        let uuid = uuid::Uuid::parse_str(node_id).map_err(|e| {
            BeliefGraphError::NodeNotFound(format!("Invalid UUID {}: {}", node_id, e))
        })?;
        Ok(self.get_node(uuid).as_ref().map(convert_node))
    }

    async fn query_beliefs(
        &self,
        query_spec: BeliefQuerySpec,
    ) -> Result<Vec<crate::modules::common::BeliefNode>, BeliefGraphError> {
        let nodes = self.nodes.read();
        let mut result: Vec<crate::modules::common::BeliefNode> = nodes
            .values()
            .filter(|n| {
                if let Some(ref nt) = query_spec.node_type {
                    format!("{:?}", n.node_type) == *nt
                } else {
                    true
                }
            })
            .map(convert_node)
            .collect();

        if let Some(threshold) = query_spec.confidence_threshold {
            result.retain(|n| n.confidence >= threshold);
        }

        Ok(result)
    }

    async fn get_belief_state(
        &self,
        belief_ids: &[String],
    ) -> Result<CommonBeliefState, BeliefGraphError> {
        let nodes = self.nodes.read();
        let edges = self.edges.read();

        let converted_nodes: Vec<crate::modules::common::BeliefNode> = if belief_ids.is_empty() {
            nodes.values().map(convert_node).collect()
        } else {
            belief_ids
                .iter()
                .filter_map(|id| {
                    uuid::Uuid::parse_str(id)
                        .ok()
                        .and_then(|uuid| nodes.get(&uuid).map(convert_node))
                })
                .collect()
        };

        let node_ids: HashSet<BeliefId> = if belief_ids.is_empty() {
            nodes.keys().cloned().collect()
        } else {
            belief_ids
                .iter()
                .filter_map(|id| uuid::Uuid::parse_str(id).ok())
                .collect()
        };

        let converted_edges: Vec<crate::modules::common::RelationEdge> = edges
            .values()
            .filter(|e| node_ids.contains(&e.source_node) || node_ids.contains(&e.target_node))
            .map(convert_edge)
            .collect();

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for n in &converted_nodes {
            hasher.update(n.node_id.as_bytes());
        }
        for e in &converted_edges {
            hasher.update(e.edge_id.as_bytes());
        }
        let hash = format!("{:x}", hasher.finalize());

        Ok(CommonBeliefState {
            nodes: converted_nodes,
            edges: converted_edges,
            hash,
        })
    }

    async fn get_outgoing_edges(
        &self,
        node_id: &str,
    ) -> Result<Vec<crate::modules::common::RelationEdge>, BeliefGraphError> {
        let uuid = uuid::Uuid::parse_str(node_id).map_err(|e| {
            BeliefGraphError::NodeNotFound(format!("Invalid UUID {}: {}", node_id, e))
        })?;
        let adj = self.adjacency.read();
        let outgoing = adj.outgoing.get(&uuid).cloned().unwrap_or_default();
        drop(adj);
        let edges = self.edges.read();
        let result: Vec<crate::modules::common::RelationEdge> = outgoing
            .iter()
            .filter_map(|(edge_id, _)| edges.get(edge_id).map(convert_edge))
            .collect();
        Ok(result)
    }
}

#[async_trait]
impl BeliefGraphWriter for BeliefGraph {
    async fn update_belief_confidence(
        &self,
        node_id: &str,
        attribute: &str,
        new_confidence: f64,
    ) -> Result<(), BeliefGraphError> {
        let uuid = uuid::Uuid::parse_str(node_id).map_err(|e| {
            BeliefGraphError::NodeNotFound(format!("Invalid UUID {}: {}", node_id, e))
        })?;
        let mut nodes = self.nodes.write();
        let node = nodes
            .get_mut(&uuid)
            .ok_or_else(|| BeliefGraphError::NodeNotFound(node_id.to_string()))?;
        if let Some(attr) = node.attributes.get_mut(attribute) {
            let old_confidence = attr.confidence;
            attr.confidence = new_confidence;
            attr.last_updated = chrono::Utc::now();
            drop(nodes);
            let mut indexes = self.indexes.write();
            indexes.update_confidence(uuid, old_confidence, new_confidence);
            Ok(())
        } else {
            Err(BeliefGraphError::NodeNotFound(format!(
                "Attribute '{}' not found on node {}",
                attribute, node_id
            )))
        }
    }

    async fn mark_belief_for_revision(
        &self,
        belief_id: &str,
        reason: &str,
    ) -> Result<(), BeliefGraphError> {
        let uuid = uuid::Uuid::parse_str(belief_id).map_err(|e| {
            BeliefGraphError::NodeNotFound(format!("Invalid UUID {}: {}", belief_id, e))
        })?;
        let mut nodes = self.nodes.write();
        let node = nodes
            .get_mut(&uuid)
            .ok_or_else(|| BeliefGraphError::NodeNotFound(belief_id.to_string()))?;
        node.metadata.importance = ImportanceLevel::High;
        node.metadata.last_modified = chrono::Utc::now();
        node.metadata.version += 1;
        let _ = reason;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_adjacency_list() {
        let mut adj = AdjacencyList::new();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let edge_id = Uuid::new_v4();

        adj.add_edge(edge_id, source, target);

        let outgoing = adj.get_outgoing_edges(source);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].0, edge_id);
        assert_eq!(outgoing[0].1, target);

        let incoming = adj.get_incoming_edges(target);
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].0, edge_id);
        assert_eq!(incoming[0].1, source);
    }

    #[test]
    fn test_graph_indexes() {
        let mut indexes = GraphIndexes::new();

        let mut node = BeliefNode::new(
            BeliefNodeType::User,
            "Alice".to_string(),
            "test".to_string(),
            SourceType::UserInput,
        );
        node = node.with_tags(vec!["test".to_string(), "user".to_string()]);

        indexes.add_node(&node);

        let type_result = indexes.query_by_type(BeliefNodeType::User);
        assert!(type_result.contains(&node.node_id));

        let tag_result = indexes.query_by_tag("test");
        assert!(tag_result.contains(&node.node_id));

        let name_result = indexes.query_by_name_exact("Alice");
        assert!(name_result.contains(&node.node_id));
    }

    #[test]
    fn test_belief_graph_creation() {
        let graph = BeliefGraph::with_default_config();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
        assert_eq!(graph.version(), 0);
    }

    #[test]
    fn test_graph_statistics() {
        let graph = BeliefGraph::with_default_config();
        let stats = graph.get_statistics();

        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.total_edges, 0);
        assert_eq!(stats.average_confidence, 0.0);
    }
}
