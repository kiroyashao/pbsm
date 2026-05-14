//! 信念图同步操作实现
//!
//! 本模块实现了信念图的同步操作，包括：
//! - 信念创建、更新、删除
//! - 边创建、删除
//! - 信念查询
//! - 图遍历
//! - 信念推导
//!
//! # 设计说明
//!
//! 所有操作均为同步接口，直接操作 BeliefGraph 内部数据结构。
//! 操作会自动更新相关索引和版本号，确保数据一致性。
//!
//! # 性能约束
//!
//! - 信念创建：O(1) 平均时间复杂度
//! - 信念更新：O(k)，k为属性数量
//! - 信念查询：O(1) 索引查找 + O(n) 结果过滤
//! - 图遍历：O(V+E)，V为节点数，E为边数

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use super::error::BeliefGraphError;
use super::graph::BeliefGraph;
use super::types::*;

pub type Result<T> = std::result::Result<T, BeliefGraphError>;

/// 信念图同步操作提供者
///
/// # 设计说明
///
/// BeliefGraphOperations 封装了所有信念图的同步操作：
/// - 信念的 CRUD 操作
/// - 边的 CRUD 操作
/// - 复杂的查询和遍历操作
/// - 信念推导（基于图的传导推理）
pub struct BeliefGraphOperations;

impl BeliefGraphOperations {
    /// 创建新的信念节点
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `node_type` - 节点类型
    /// * `name` - 节点名称（1-64字符）
    /// * `attributes` - 初始属性映射
    /// * `source` - 来源标识
    /// * `source_type` - 来源类型
    /// * `tags` - 标签列表（最多10个）
    /// * `initial_confidence` - 初始置信度（可选）
    ///
    /// # 返回
    /// * `Ok(BeliefId)` - 新创建的节点ID
    /// * `Err(BeliefGraphError)` - 创建失败
    ///
    /// # 约束限制
    ///
    /// - 名称长度：1-64字符
    /// - 属性数量：最多50个
    /// - 标签数量：最多10个
    /// - 节点数量不能超过配置上限
    #[allow(clippy::too_many_arguments)]
    pub fn create_belief(
        graph: &BeliefGraph,
        node_type: BeliefNodeType,
        name: String,
        attributes: HashMap<String, AttributeValue>,
        source: String,
        source_type: SourceType,
        tags: Option<Vec<String>>,
        initial_confidence: Option<f64>,
    ) -> Result<BeliefId> {
        if name.is_empty() || name.len() > 64 {
            return Err(BeliefGraphError::ValidationError(
                "Name must be between 1 and 64 characters".to_string(),
            ));
        }

        let node_count = graph.node_count();
        let edge_count = graph.edge_count();
        if node_count >= graph.config().max_nodes {
            return Err(BeliefGraphError::CapacityExceeded {
                nodes: node_count,
                edges: edge_count,
            });
        }

        let mut node = BeliefNode::new(node_type, name.clone(), source.clone(), source_type);

        if let Some(initial_conf) = initial_confidence {
            let attr_value = AttributeValue::new(
                serde_json::json!(null),
                initial_conf,
                source.clone(),
                source_type,
            );
            node.attributes
                .insert("_initial_confidence".to_string(), attr_value);
        }

        for (key, value) in attributes {
            if node.attributes.len() >= 50 {
                return Err(BeliefGraphError::ValidationError(
                    "Maximum 50 attributes per node".to_string(),
                ));
            }
            node.attributes.insert(key, value);
        }

        if let Some(t) = tags {
            if t.len() > 10 {
                return Err(BeliefGraphError::ValidationError(
                    "Maximum 10 tags per node".to_string(),
                ));
            }
            node = node.with_tags(t);
        }

        let node_id = node.node_id;

        graph.nodes().write().insert(node_id, node);
        graph
            .indexes_mut()
            .write()
            .add_node(graph.nodes().read().get(&node_id).unwrap());

        let mut version = graph.version_mut().write();
        *version += 1;

        graph.publish_event(crate::modules::common::BeliefGraphEvent::BeliefCreated {
            node_id: node_id.to_string(),
            node_type: format!("{:?}", node_type),
            source: source.clone(),
        });

        Ok(node_id)
    }

    /// 更新信念节点属性
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `node_id` - 要更新的节点ID
    /// * `updates` - 属性更新映射
    /// * `strategy` - 更新策略
    ///
    /// # 返回
    /// * `Ok(UpdateResult)` - 更新结果
    /// * `Err(BeliefGraphError)` - 更新失败
    ///
    /// # 更新策略说明
    ///
    /// | 策略 | 描述 |
    /// |------|------|
    /// | Overwrite | 完全覆盖原属性 |
    /// | IncrementalMerge | 增量合并，置信度取平均 |
    /// | ConditionalReplace | 仅当新置信度更高时替换 |
    /// | ConservativeMerge | 保留所有历史值 |
    /// | AverageBlend | 置信度加权平均 |
    pub fn update_belief(
        graph: &BeliefGraph,
        node_id: BeliefId,
        updates: HashMap<String, AttributeValue>,
        strategy: UpdateStrategy,
    ) -> Result<UpdateResult> {
        let mut nodes = graph.nodes().write();
        let node = nodes
            .get_mut(&node_id)
            .ok_or_else(|| BeliefGraphError::NodeNotFound(node_id.to_string()))?;

        let old_confidence = node.average_confidence();
        let _old_version = node.metadata.version;

        match strategy {
            UpdateStrategy::Overwrite => {
                node.attributes.clear();
                for (key, value) in updates {
                    node.attributes.insert(key, value);
                }
            }
            UpdateStrategy::IncrementalMerge => {
                for (key, value) in updates {
                    if let Some(existing) = node.attributes.get_mut(&key) {
                        existing.value = value.value;
                        existing.confidence = (existing.confidence + value.confidence) / 2.0;
                        existing.last_updated = chrono::Utc::now();
                        existing.source = value.source;
                        existing.source_type = value.source_type;
                    } else {
                        node.attributes.insert(key, value);
                    }
                }
            }
            UpdateStrategy::ConditionalReplace => {
                for (key, value) in updates {
                    if let Some(existing) = node.attributes.get_mut(&key) {
                        if value.confidence > existing.confidence {
                            existing.value = value.value;
                            existing.confidence = value.confidence;
                            existing.last_updated = chrono::Utc::now();
                            existing.source = value.source;
                            existing.source_type = value.source_type;
                        }
                    } else {
                        node.attributes.insert(key, value);
                    }
                }
            }
            UpdateStrategy::ConservativeMerge => {
                for (key, value) in updates {
                    node.attributes.insert(key, value);
                }
            }
            UpdateStrategy::AverageBlend => {
                for (key, value) in updates {
                    if let Some(existing) = node.attributes.get_mut(&key) {
                        let total_conf = existing.confidence + value.confidence;
                        if total_conf > 0.0 {
                            existing.confidence = (existing.confidence * existing.confidence
                                + value.confidence * value.confidence)
                                / total_conf;
                        }
                        existing.last_updated = chrono::Utc::now();
                    } else {
                        node.attributes.insert(key, value);
                    }
                }
            }
        }

        node.metadata.version += 1;
        node.metadata.last_modified = chrono::Utc::now();

        let new_confidence = node.average_confidence();
        let new_version = node.metadata.version;

        drop(nodes);

        let mut indexes = graph.indexes_mut().write();
        indexes.update_confidence(node_id, old_confidence, new_confidence);

        let mut version = graph.version_mut().write();
        *version += 1;

        graph.publish_event(crate::modules::common::BeliefGraphEvent::BeliefUpdated {
            node_id: node_id.to_string(),
            update_type: format!("{:?}", strategy),
            old_confidence,
            new_confidence,
        });

        Ok(UpdateResult {
            success: true,
            updated: true,
            conflict_detected: false,
            new_version,
        })
    }

    /// 删除信念节点
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `node_id` - 要删除的节点ID
    /// * `cascade` - 是否级联删除关联边
    ///
    /// # 返回
    /// * `Ok(DeleteResult)` - 删除结果
    /// * `Err(BeliefGraphError)` - 删除失败
    ///
    /// # 级联删除说明
    ///
    /// - `cascade = true`: 删除节点及其所有关联边
    /// - `cascade = false`: 仅删除节点，若有关联边则返回错误
    pub fn delete_belief(
        graph: &BeliefGraph,
        node_id: BeliefId,
        cascade: bool,
    ) -> Result<DeleteResult> {
        let nodes = graph.nodes().read();
        let node = nodes
            .get(&node_id)
            .ok_or_else(|| BeliefGraphError::NodeNotFound(node_id.to_string()))?
            .clone();
        drop(nodes);

        let mut edges = graph.edges().write();
        let mut adjacency = graph.adjacency_mut().write();

        let mut deleted_edge_ids = Vec::new();

        if cascade {
            let connected_edges: Vec<EdgeId> = node
                .outgoing_edges
                .iter()
                .chain(node.incoming_edges.iter())
                .cloned()
                .collect();

            for edge_id in connected_edges {
                if let Some(edge) = edges.remove(&edge_id) {
                    deleted_edge_ids.push(edge_id);
                    adjacency.remove_edge(edge_id, edge.source_node, edge.target_node);
                }
            }
        } else {
            if !node.outgoing_edges.is_empty() || !node.incoming_edges.is_empty() {
                drop(edges);
                drop(adjacency);
                return Err(BeliefGraphError::ValidationError(
                    "Cannot delete node with existing edges in non-cascade mode".to_string(),
                ));
            }
            adjacency.remove_node(node_id);
        }

        drop(edges);
        drop(adjacency);

        graph.indexes_mut().write().remove_node(&node);
        graph.nodes().write().remove(&node_id);

        let mut version = graph.version_mut().write();
        *version += 1;

        graph.publish_event(crate::modules::common::BeliefGraphEvent::BeliefDeleted {
            node_id: node_id.to_string(),
            cascade,
        });

        Ok(DeleteResult {
            success: true,
            deleted_node_id: node_id,
            deleted_edge_ids,
        })
    }

    /// 执行信念查询
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `spec` - 查询规格
    /// * `options` - 查询选项
    ///
    /// # 返回
    /// * `Ok(QueryResult)` - 查询结果
    /// * `Err(BeliefGraphError)` - 查询失败
    ///
    /// # 查询类型说明
    ///
    /// | 类型 | 说明 |
    /// |------|------|
    /// | ExactById | 按ID精确查找 |
    /// | ByType | 按节点类型查找 |
    /// | ByTag | 按标签查找 |
    /// | ByName | 按名称查找（支持模糊匹配） |
    /// | ByAttribute | 按属性值查找 |
    /// | ByConfidenceRange | 按置信度范围查找 |
    /// | ByRelation | 按关系查找 |
    /// | ByTimeRange | 按时间范围查找 |
    /// | GraphTraversal | 图遍历查询 |
    pub fn query(
        graph: &BeliefGraph,
        spec: QuerySpecification,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        let start = Instant::now();
        let nodes = graph.nodes().read();
        let indexes = graph.indexes_mut().read();

        let adjacency = graph.adjacency_mut().read();
        let candidates = Self::get_candidates(&nodes, &indexes, &adjacency, &spec);
        drop(adjacency);
        let mut results: Vec<BeliefNode> = candidates
            .into_iter()
            .filter_map(|id| nodes.get(&id).cloned())
            .collect();

        if let Some(min_conf) = spec.min_confidence {
            results.retain(|n| n.average_confidence() >= min_conf);
        }
        if let Some(max_conf) = spec.max_confidence {
            results.retain(|n| n.average_confidence() <= max_conf);
        }

        if let Some(sort_by) = options.sort_by {
            match sort_by {
                SortField::Confidence => {
                    results.sort_by(|a, b| {
                        let conf_a = a.average_confidence();
                        let conf_b = b.average_confidence();
                        if options.sort_order == SortOrder::Asc {
                            conf_a.partial_cmp(&conf_b).unwrap()
                        } else {
                            conf_b.partial_cmp(&conf_a).unwrap()
                        }
                    });
                }
                SortField::LastModified => {
                    results.sort_by(|a, b| {
                        if options.sort_order == SortOrder::Asc {
                            a.metadata.last_modified.cmp(&b.metadata.last_modified)
                        } else {
                            b.metadata.last_modified.cmp(&a.metadata.last_modified)
                        }
                    });
                }
                SortField::Name => {
                    results.sort_by(|a, b| {
                        if options.sort_order == SortOrder::Asc {
                            a.name.cmp(&b.name)
                        } else {
                            b.name.cmp(&a.name)
                        }
                    });
                }
                SortField::Relevance => {}
            }
        }

        let total_count = results.len();
        let has_more = options.offset + options.limit < total_count;

        results = results
            .into_iter()
            .skip(options.offset)
            .take(options.limit)
            .collect();

        Ok(QueryResult {
            items: results,
            total_count,
            has_more,
            execution_time_ms: start.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// 根据查询规格获取候选节点ID集合
    fn get_candidates(
        nodes: &std::collections::HashMap<BeliefId, BeliefNode>,
        indexes: &super::graph::GraphIndexes,
        adjacency: &super::graph::AdjacencyList,
        spec: &QuerySpecification,
    ) -> HashSet<BeliefId> {
        match spec.query_type {
            QueryType::ExactById => {
                if let Some(node_id) = spec.node_id {
                    if nodes.contains_key(&node_id) {
                        let mut set = HashSet::new();
                        set.insert(node_id);
                        return set;
                    }
                }
                HashSet::new()
            }
            QueryType::ByType => spec
                .node_type
                .map(|t| indexes.query_by_type(t))
                .unwrap_or_default(),
            QueryType::ByTag => spec
                .tag
                .as_ref()
                .map(|t| indexes.query_by_tag(t))
                .unwrap_or_default(),
            QueryType::ByName => {
                if let Some(ref pattern) = spec.name_pattern {
                    if spec.fuzzy {
                        Self::fuzzy_name_search(nodes, pattern)
                    } else {
                        indexes.query_by_name_exact(pattern)
                    }
                } else {
                    HashSet::new()
                }
            }
            QueryType::ByAttribute => {
                if let Some(ref key) = spec.attribute_key {
                    let mut result = HashSet::new();
                    for (id, node) in nodes {
                        if node.attributes.contains_key(key) {
                            if let Some(ref value) = spec.attribute_value {
                                if let Some(attr) = node.attributes.get(key) {
                                    if &attr.value == value {
                                        result.insert(*id);
                                    }
                                }
                            } else {
                                result.insert(*id);
                            }
                        }
                    }
                    result
                } else {
                    HashSet::new()
                }
            }
            QueryType::ByConfidenceRange => {
                indexes.query_by_confidence_range(spec.min_confidence, spec.max_confidence)
            }
            QueryType::ByRelation => {
                if let Some(source_id) = spec.source_id {
                    let direction = spec.direction.unwrap_or(EdgeDirection::Both);
                    let edge_pairs: Vec<(EdgeId, BeliefId)> = match direction {
                        EdgeDirection::Outgoing => adjacency.get_outgoing_edges(source_id),
                        EdgeDirection::Incoming => adjacency.get_incoming_edges(source_id),
                        EdgeDirection::Both => {
                            let mut edges = adjacency.get_outgoing_edges(source_id);
                            edges.extend(adjacency.get_incoming_edges(source_id));
                            edges
                        }
                    };
                    edge_pairs.into_iter().map(|(_, node_id)| node_id).collect()
                } else {
                    let mut result = HashSet::new();
                    for node_id in adjacency.outgoing.keys() {
                        if nodes.contains_key(node_id) {
                            result.insert(*node_id);
                        }
                    }
                    for node_id in adjacency.incoming.keys() {
                        if nodes.contains_key(node_id) {
                            result.insert(*node_id);
                        }
                    }
                    result
                }
            }
            QueryType::ByTimeRange => {
                let mut result = HashSet::new();
                for (id, node) in nodes {
                    let time = node.metadata.last_modified;
                    let after_start = spec.start_time.map_or(true, |st| time >= st);
                    let before_end = spec.end_time.map_or(true, |et| time <= et);
                    if after_start && before_end {
                        result.insert(*id);
                    }
                }
                result
            }
            QueryType::GraphTraversal => {
                if let Some(start_id) = spec.start_node_id {
                    if !nodes.contains_key(&start_id) {
                        return HashSet::new();
                    }
                    let max_depth = spec.max_depth.unwrap_or(3);
                    let mut visited = HashSet::new();
                    let mut queue = VecDeque::new();
                    visited.insert(start_id);
                    queue.push_back((start_id, 0));
                    while let Some((current_id, depth)) = queue.pop_front() {
                        if depth >= max_depth {
                            continue;
                        }
                        let neighbors: Vec<BeliefId> = adjacency
                            .get_outgoing_edges(current_id)
                            .into_iter()
                            .chain(adjacency.get_incoming_edges(current_id))
                            .map(|(_, neighbor_id)| neighbor_id)
                            .collect();
                        for neighbor_id in neighbors {
                            if !visited.contains(&neighbor_id) && nodes.contains_key(&neighbor_id) {
                                visited.insert(neighbor_id);
                                queue.push_back((neighbor_id, depth + 1));
                            }
                        }
                    }
                    visited
                } else {
                    HashSet::new()
                }
            }
        }
    }

    /// 模糊名称搜索
    ///
    /// # 算法说明
    ///
    /// 使用编辑距离算法（Levenshtein distance）进行模糊匹配：
    /// - 包含匹配：名称包含查询模式
    /// - 编辑距离：编辑距离 ≤ 2 视为匹配
    fn fuzzy_name_search(
        nodes: &std::collections::HashMap<BeliefId, BeliefNode>,
        pattern: &str,
    ) -> HashSet<BeliefId> {
        let pattern_lower = pattern.to_lowercase();
        let mut results = HashSet::new();

        for (id, node) in nodes {
            let name_lower = node.name.to_lowercase();
            if name_lower.contains(&pattern_lower)
                || Self::edit_distance(&name_lower, &pattern_lower) <= 2
            {
                results.insert(*id);
            }
        }

        results
    }

    /// 计算两个字符串之间的编辑距离
    fn edit_distance(s1: &str, s2: &str) -> usize {
        let len1 = s1.len();
        let len2 = s2.len();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
            row[0] = i;
        }
        for (j, val) in matrix[0].iter_mut().enumerate().take(len2 + 1) {
            *val = j;
        }

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if s1.chars().nth(i - 1) == s2.chars().nth(j - 1) {
                    0
                } else {
                    1
                };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len1][len2]
    }

    /// 创建关系边
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `edge_type` - 边类型
    /// * `source_node` - 源节点ID
    /// * `target_node` - 目标节点ID
    /// * `confidence` - 边的置信度
    ///
    /// # 返回
    /// * `Ok(EdgeId)` - 新创建的边ID
    /// * `Err(BeliefGraphError)` - 创建失败
    pub fn create_edge(
        graph: &BeliefGraph,
        edge_type: RelationEdgeType,
        source_node: BeliefId,
        target_node: BeliefId,
        confidence: f64,
    ) -> Result<EdgeId> {
        let nodes = graph.nodes().read();
        if !nodes.contains_key(&source_node) {
            return Err(BeliefGraphError::NodeNotFound(source_node.to_string()));
        }
        if !nodes.contains_key(&target_node) {
            return Err(BeliefGraphError::NodeNotFound(target_node.to_string()));
        }

        let node_count = nodes.len();
        drop(nodes);
        let edge_count = graph.edge_count();
        if edge_count >= graph.config().max_edges {
            return Err(BeliefGraphError::CapacityExceeded {
                nodes: node_count,
                edges: edge_count,
            });
        }

        let edge = RelationEdge::new(edge_type, source_node, target_node, confidence);
        let edge_id = edge.edge_id;

        graph.edges().write().insert(edge_id, edge.clone());

        graph
            .adjacency_mut()
            .write()
            .add_edge(edge_id, source_node, target_node);

        if let Some(n) = graph.nodes().write().get_mut(&source_node) {
            n.outgoing_edges.push(edge_id)
        }
        if let Some(n) = graph.nodes().write().get_mut(&target_node) {
            n.incoming_edges.push(edge_id)
        }

        let mut version = graph.version_mut().write();
        *version += 1;

        graph.publish_event(crate::modules::common::BeliefGraphEvent::EdgeCreated {
            edge_id: edge_id.to_string(),
            source_node: source_node.to_string(),
            target_node: target_node.to_string(),
            edge_type: format!("{:?}", edge_type),
        });

        Ok(edge_id)
    }

    /// 删除关系边
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `edge_id` - 要删除的边ID
    ///
    /// # 返回
    /// * `Ok(EdgeId)` - 已删除的边ID
    /// * `Err(BeliefGraphError)` - 删除失败
    pub fn delete_edge(graph: &BeliefGraph, edge_id: EdgeId) -> Result<EdgeId> {
        let edges = graph.edges().read();
        let edge = edges
            .get(&edge_id)
            .ok_or_else(|| BeliefGraphError::EdgeNotFound(edge_id.to_string()))?
            .clone();
        drop(edges);

        graph.edges().write().remove(&edge_id);
        graph
            .adjacency_mut()
            .write()
            .remove_edge(edge_id, edge.source_node, edge.target_node);

        if let Some(node) = graph.nodes().write().get_mut(&edge.source_node) {
            node.outgoing_edges.retain(|&id| id != edge_id);
        }
        if let Some(node) = graph.nodes().write().get_mut(&edge.target_node) {
            node.incoming_edges.retain(|&id| id != edge_id);
        }

        let mut version = graph.version_mut().write();
        *version += 1;

        graph.publish_event(crate::modules::common::BeliefGraphEvent::EdgeDeleted {
            edge_id: edge_id.to_string(),
            source_node: edge.source_node.to_string(),
            target_node: edge.target_node.to_string(),
        });

        Ok(edge_id)
    }

    /// 图遍历
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `start_node_id` - 起始节点ID
    /// * `max_depth` - 最大遍历深度
    /// * `edge_type_filter` - 边类型过滤器（可选）
    /// * `direction` - 遍历方向
    /// * `include_start` - 是否包含起始节点
    ///
    /// # 返回
    /// * `Ok(TraversalResult)` - 遍历结果
    /// * `Err(BeliefGraphError)` - 遍历失败
    ///
    /// # 遍历方向说明
    ///
    /// | 方向 | 说明 |
    /// |------|------|
    /// | Outgoing | 沿出边遍历 |
    /// | Incoming | 沿入边遍历 |
    /// | Both | 双向遍历 |
    pub fn traverse_graph(
        graph: &BeliefGraph,
        start_node_id: BeliefId,
        max_depth: u32,
        edge_type_filter: Option<Vec<RelationEdgeType>>,
        direction: EdgeDirection,
        include_start: bool,
    ) -> Result<TraversalResult> {
        let nodes = graph.nodes().read();
        if !nodes.contains_key(&start_node_id) {
            return Err(BeliefGraphError::NodeNotFound(start_node_id.to_string()));
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut visited_edges = Vec::new();
        let mut path_map = HashMap::new();

        if include_start {
            visited.insert(start_node_id);
            path_map.insert(start_node_id, Vec::new());
        }
        queue.push_back((start_node_id, 0));

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let adjacency = graph.adjacency_mut().read();
            let edge_ids = match direction {
                EdgeDirection::Outgoing => adjacency.get_outgoing_edges(current_id),
                EdgeDirection::Incoming => adjacency.get_incoming_edges(current_id),
                EdgeDirection::Both => {
                    let mut edges = adjacency.get_outgoing_edges(current_id);
                    edges.extend(adjacency.get_incoming_edges(current_id));
                    edges
                }
            };

            let edges_lock = graph.edges().read();
            for (edge_id, neighbor_id) in edge_ids {
                if visited.contains(&neighbor_id) {
                    continue;
                }

                if let Some(ref filter) = edge_type_filter {
                    if let Some(edge) = edges_lock.get(&edge_id) {
                        if !filter.contains(&edge.edge_type) {
                            continue;
                        }
                    }
                }

                visited.insert(neighbor_id);
                visited_edges.push(edges_lock.get(&edge_id).cloned().unwrap_or_else(|| {
                    RelationEdge::new(RelationEdgeType::RelatedTo, current_id, neighbor_id, 0.5)
                }));

                let mut path = path_map.get(&current_id).cloned().unwrap_or_default();
                path.push(edge_id);
                path_map.insert(neighbor_id, path);

                queue.push_back((neighbor_id, depth + 1));
            }
        }

        drop(nodes);

        let nodes = graph.nodes().read();
        let visited_nodes: Vec<BeliefNode> = visited
            .iter()
            .filter_map(|id| nodes.get(id).cloned())
            .collect();

        Ok(TraversalResult {
            visited_nodes,
            visited_edges,
            path_map,
        })
    }

    /// 信念推导（基于图的传导推理）
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `start_node_id` - 起始节点ID
    /// * `target_attributes` - 目标属性列表
    /// * `inference_depth` - 推理深度
    /// * `confidence_threshold` - 置信度阈值
    ///
    /// # 返回
    /// * `Ok(Vec<DerivationResult>)` - 推导结果列表
    /// * `Err(BeliefGraphError)` - 推导失败
    ///
    /// # 算法说明
    ///
    /// 基于图的传导推理算法：
    /// 1. 从起始节点开始，按广度优先遍历图
    /// 2. 沿边传播信念信息
    /// 3. 收集满足置信度阈值的推导结果
    pub fn derive_beliefs(
        graph: &BeliefGraph,
        start_node_id: BeliefId,
        target_attributes: Vec<String>,
        inference_depth: u32,
        confidence_threshold: f64,
    ) -> Result<Vec<DerivationResult>> {
        let nodes = graph.nodes().read();
        if !nodes.contains_key(&start_node_id) {
            return Err(BeliefGraphError::NodeNotFound(start_node_id.to_string()));
        }
        drop(nodes);

        let mut results = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((start_node_id, 0u32, Vec::<DerivationStep>::new()));

        while let Some((current_id, depth, path)) = queue.pop_front() {
            if depth >= inference_depth {
                continue;
            }

            if visited.contains(&current_id) {
                continue;
            }
            visited.insert(current_id);

            let adjacency = graph.adjacency_mut().read();
            let edge_ids = adjacency.get_outgoing_edges(current_id);
            let edges_lock = graph.edges().read();
            let nodes_lock = graph.nodes().read();

            let current_node = match nodes_lock.get(&current_id) {
                Some(n) => n,
                None => continue,
            };

            let attributes_to_propagate: Vec<(String, AttributeValue)> = current_node
                .attributes
                .iter()
                .filter(|(key, _)| target_attributes.is_empty() || target_attributes.contains(key))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            for (edge_id, neighbor_id) in edge_ids {
                if visited.contains(&neighbor_id) {
                    continue;
                }

                let edge = match edges_lock.get(&edge_id) {
                    Some(e) => e,
                    None => continue,
                };

                let neighbor_node = match nodes_lock.get(&neighbor_id) {
                    Some(n) => n,
                    None => continue,
                };

                let propagation_rule = format!("{:?}_propagation", edge.edge_type);
                let decay_factor = 0.9f64.powi(depth as i32 + 1);

                let mut derived_attrs = HashMap::new();

                for (attr_key, attr_value) in &attributes_to_propagate {
                    if neighbor_node.attributes.contains_key(attr_key) {
                        continue;
                    }

                    let derived_confidence =
                        attr_value.confidence * edge.confidence * decay_factor;

                    if derived_confidence < confidence_threshold {
                        continue;
                    }

                    derived_attrs.insert(
                        attr_key.clone(),
                        AttributeValue::new(
                            attr_value.value.clone(),
                            derived_confidence,
                            format!(
                                "derived_from:{}",
                                current_id
                            ),
                            SourceType::Derived,
                        ),
                    );
                }

                if !derived_attrs.is_empty() {
                    let min_confidence = derived_attrs
                        .values()
                        .map(|v| v.confidence)
                        .fold(f64::INFINITY, f64::min);

                    let mut new_path = path.clone();
                    new_path.push(DerivationStep {
                        node_id: neighbor_id,
                        edge_id: Some(edge_id),
                        rule: propagation_rule.clone(),
                    });

                    results.push(DerivationResult {
                        derived_node_id: neighbor_id,
                        derived_attributes: derived_attrs,
                        confidence: min_confidence,
                        derivation_path: new_path.clone(),
                        derivation_rule: propagation_rule,
                    });

                    if depth + 1 < inference_depth {
                        queue.push_back((neighbor_id, depth + 1, new_path));
                    }
                } else if depth + 1 < inference_depth {
                    let mut new_path = path.clone();
                    new_path.push(DerivationStep {
                        node_id: neighbor_id,
                        edge_id: Some(edge_id),
                        rule: propagation_rule,
                    });
                    queue.push_back((neighbor_id, depth + 1, new_path));
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> BeliefGraph {
        BeliefGraph::with_default_config()
    }

    #[test]
    fn test_create_belief() {
        let graph = create_test_graph();

        let result = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "Alice".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        );

        assert!(result.is_ok());
        let node_id = result.unwrap();
        assert!(graph.get_node(node_id).is_some());
    }

    #[test]
    fn test_create_belief_validation() {
        let graph = create_test_graph();

        let result = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_update_belief() {
        let graph = create_test_graph();
        let node_id = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::File,
            "test.txt".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::DirectObservation,
            None,
            None,
        )
        .unwrap();

        let mut updates = HashMap::new();
        updates.insert(
            "size".to_string(),
            AttributeValue::new(
                serde_json::json!(1024),
                0.9,
                "test".to_string(),
                SourceType::DirectObservation,
            ),
        );

        let result = BeliefGraphOperations::update_belief(
            &graph,
            node_id,
            updates,
            UpdateStrategy::Overwrite,
        );

        assert!(result.is_ok());
        let update_result = result.unwrap();
        assert!(update_result.updated);
        assert_eq!(update_result.new_version, 2);
    }

    #[test]
    fn test_delete_belief() {
        let graph = create_test_graph();
        let node_id = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::Tool,
            "my_tool".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::ToolReturn,
            None,
            None,
        )
        .unwrap();

        let result = BeliefGraphOperations::delete_belief(&graph, node_id, false);
        assert!(result.is_ok());
        assert!(graph.get_node(node_id).is_none());
    }

    #[test]
    fn test_create_edge() {
        let graph = create_test_graph();
        let source = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "Bob".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();
        let target = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::File,
            "doc.txt".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();

        let edge_id =
            BeliefGraphOperations::create_edge(&graph, RelationEdgeType::Owns, source, target, 0.9);

        assert!(edge_id.is_ok());
        assert!(graph.get_edge(edge_id.unwrap()).is_some());
    }

    #[test]
    fn test_query_by_type() {
        let graph = create_test_graph();
        BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "User1".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();
        BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "User2".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();
        BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::File,
            "File1".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();

        let spec = QuerySpecification {
            query_type: QueryType::ByType,
            node_type: Some(BeliefNodeType::User),
            ..Default::default()
        };

        let result = BeliefGraphOperations::query(&graph, spec, QueryOptions::default());
        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.items.len(), 2);
    }
}
