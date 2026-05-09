//! 信念图快照管理实现
//!
//! 本模块实现了信念图的版本控制功能，包括：
//! - 快照创建
//! - 快照历史查询
//! - 回滚操作
//! - 版本差异计算
//!
//! # 设计说明
//!
//! 快照系统提供信念图的完整状态备份，支持任意时间点的回滚。
//! 每个快照包含图中所有节点和边的完整副本，以及元数据信息。
//!
//! # 快照类型说明
//!
//! | 类型 | 描述 | 触发时机 |
//! |------|------|---------|
//! | Manual | 手动创建 | 用户显式调用 |
//! | Auto | 自动创建 | 系统自动触发（如回滚前备份） |
//! | Session | 会话级快照 | 会话开始/结束 |
//! | Checkpoint | 检查点快照 | 关键操作前 |
//!
//! # 校验和说明
//!
//! 每个快照包含 SHA-256 校验和，用于验证快照完整性。
//! 校验和基于节点ID、类型、名称和边ID、类型计算。

use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::error::BeliefGraphError;
use super::graph::BeliefGraph;
use super::types::*;

pub type Result<T> = std::result::Result<T, BeliefGraphError>;

/// 快照操作提供者
///
/// # 设计说明
///
/// SnapshotOperations 封装了信念图的版本控制操作：
/// - 快照创建：捕获图的完整状态
/// - 快照查询：获取历史快照列表
/// - 回滚操作：将图恢复到指定快照状态
/// - 差异计算：比较两个快照的差异
pub struct SnapshotOperations;

impl SnapshotOperations {
    /// 创建信念图快照
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `snapshot_type` - 快照类型
    /// * `description` - 快照描述（可选）
    ///
    /// # 返回
    /// * `Ok(SnapshotId)` - 新创建的快照ID
    /// * `Err(BeliefGraphError)` - 创建失败
    ///
    /// # 实现细节
    ///
    /// 1. 获取图中所有节点和边的只读引用
    /// 2. 计算节点和边的 SHA-256 校验和
    /// 3. 获取当前版本号
    /// 4. 创建快照元数据和快照体
    /// 5. 将快照添加到图的快照列表
    pub fn create_snapshot(
        graph: &BeliefGraph,
        snapshot_type: SnapshotType,
        description: Option<String>,
    ) -> Result<SnapshotId> {
        let nodes = graph.nodes().read();
        let edges = graph.edges().read();

        let node_list: Vec<BeliefNode> = nodes.values().cloned().collect();
        let edge_list: Vec<RelationEdge> = edges.values().cloned().collect();
        let node_count = nodes.len();
        let edge_count = edges.len();

        let checksum = Self::compute_checksum(&node_list, &edge_list);

        drop(nodes);
        drop(edges);

        let mut version_guard = graph.version_mut().write();
        let version = *version_guard;
        *version_guard += 1;

        let snapshot_id = SnapshotId::new_v4();

        let metadata = SnapshotMetadata {
            snapshot_id,
            version_number: version,
            created_at: Utc::now(),
            snapshot_type,
            trigger_reason: description.unwrap_or_else(|| "Manual snapshot".to_string()),
            node_count,
            edge_count,
            checksum,
        };

        let snapshot = GraphSnapshot {
            metadata,
            nodes: node_list,
            edges: edge_list,
        };

        graph.add_snapshot(snapshot);

        Ok(snapshot_id)
    }

    /// 获取快照历史
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `filter_type` - 快照类型过滤器（可选）
    /// * `limit` - 返回数量限制（可选）
    ///
    /// # 返回
    /// * `Vec<SnapshotMetadata>` - 快照元数据列表
    ///
    /// # 说明
    ///
    /// 返回的快照列表按创建时间降序排列（最新优先）。
    pub fn get_snapshot_history(
        graph: &BeliefGraph,
        filter_type: Option<SnapshotType>,
        limit: Option<usize>,
    ) -> Vec<SnapshotMetadata> {
        let snapshots = graph.get_snapshots();

        let filtered: Vec<SnapshotMetadata> = snapshots
            .into_iter()
            .filter(|s| {
                filter_type
                    .map(|t| s.metadata.snapshot_type == t)
                    .unwrap_or(true)
            })
            .map(|s| s.metadata)
            .collect();

        if let Some(l) = limit {
            filtered.into_iter().take(l).collect()
        } else {
            filtered
        }
    }

    /// 回滚到指定快照
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `snapshot_id` - 目标快照ID
    /// * `reason` - 回滚原因
    ///
    /// # 返回
    /// * `Ok(RollbackResult)` - 回滚结果
    /// * `Err(BeliefGraphError)` - 回滚失败
    ///
    /// # 实现细节
    ///
    /// 1. 创建回滚前的自动快照（作为备份）
    /// 2. 清空图的当前状态
    /// 3. 从目标快照恢复所有节点和边
    /// 4. 重建邻接表和索引
    /// 5. 更新版本号
    /// 6. 创建回滚后的快照（记录最终状态）
    pub fn rollback_to_snapshot(
        graph: &BeliefGraph,
        snapshot_id: SnapshotId,
        reason: String,
    ) -> Result<RollbackResult> {
        let snapshot = graph
            .get_snapshot(snapshot_id)
            .ok_or_else(|| BeliefGraphError::SnapshotNotFound(snapshot_id.to_string()))?;

        let current_snapshot_id = Self::create_snapshot(
            graph,
            SnapshotType::Auto,
            Some(format!("Pre-rollback backup: {}", reason)),
        )?;

        graph.clear();

        let mut nodes = graph.nodes().write();
        let nodes_to_insert: Vec<_> = snapshot
            .nodes
            .iter()
            .map(|n| (n.node_id, n.clone()))
            .collect();
        for (id, node) in nodes_to_insert {
            nodes.insert(id, node);
        }
        drop(nodes);

        let mut edges = graph.edges().write();
        let edges_to_insert: Vec<_> = snapshot
            .edges
            .iter()
            .map(|e| (e.edge_id, e.clone()))
            .collect();
        for (id, edge) in edges_to_insert {
            edges.insert(id, edge);
        }
        drop(edges);

        let mut adjacency = graph.adjacency_mut().write();
        for edge in &snapshot.edges {
            adjacency.add_edge(edge.edge_id, edge.source_node, edge.target_node);
        }
        drop(adjacency);

        let mut indexes = graph.indexes_mut().write();
        for node in &snapshot.nodes {
            indexes.add_node(node);
        }

        let metadata = snapshot.metadata.clone();
        drop(snapshot);

        let mut version = graph.version_mut().write();
        *version = metadata.version_number + 1;
        drop(version);

        let post_rollback_snapshot_id = Self::create_snapshot(
            graph,
            SnapshotType::Manual,
            Some("Post-rollback state".to_string()),
        )?;

        Ok(RollbackResult {
            success: true,
            previous_version_id: Some(current_snapshot_id),
            current_version_id: Some(post_rollback_snapshot_id),
            nodes_created: Vec::new(),
            nodes_updated: Vec::new(),
            nodes_deleted: Vec::new(),
            invalidated_predictions: Vec::new(),
        })
    }

    /// 计算两个快照之间的差异
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `from_snapshot_id` - 起始快照ID
    /// * `to_snapshot_id` - 目标快照ID
    ///
    /// # 返回
    /// * `Ok(VersionDiff)` - 版本差异
    /// * `Err(BeliefGraphError)` - 计算失败
    ///
    /// # 差异类型说明
    ///
    /// | 类型 | 描述 |
    /// |------|------|
    /// | nodes_added | 新增的节点 |
    /// | nodes_removed | 删除的节点 |
    /// | nodes_modified | 属性变化的节点 |
    /// | edges_added | 新增的边 |
    /// | edges_removed | 删除的边 |
    pub fn compute_diff(
        graph: &BeliefGraph,
        from_snapshot_id: SnapshotId,
        to_snapshot_id: SnapshotId,
    ) -> Result<VersionDiff> {
        let from_snapshot = graph
            .get_snapshot(from_snapshot_id)
            .ok_or_else(|| BeliefGraphError::SnapshotNotFound(from_snapshot_id.to_string()))?;
        let to_snapshot = graph
            .get_snapshot(to_snapshot_id)
            .ok_or_else(|| BeliefGraphError::SnapshotNotFound(to_snapshot_id.to_string()))?;

        let from_ids: HashSet<BeliefId> = from_snapshot.nodes.iter().map(|n| n.node_id).collect();
        let to_ids: HashSet<BeliefId> = to_snapshot.nodes.iter().map(|n| n.node_id).collect();

        let nodes_added: Vec<BeliefId> = to_ids.difference(&from_ids).cloned().collect();
        let nodes_removed: Vec<BeliefId> = from_ids.difference(&to_ids).cloned().collect();

        let intersection: Vec<BeliefId> = from_ids.intersection(&to_ids).cloned().collect();

        let from_map: HashMap<BeliefId, &BeliefNode> =
            from_snapshot.nodes.iter().map(|n| (n.node_id, n)).collect();
        let to_map: HashMap<BeliefId, &BeliefNode> =
            to_snapshot.nodes.iter().map(|n| (n.node_id, n)).collect();

        let mut nodes_modified: Vec<NodeModification> = Vec::new();

        for id in &intersection {
            let from_node = from_map.get(id).unwrap();
            let to_node = to_map.get(id).unwrap();

            let mut attribute_changes: Vec<AttributeChange> = Vec::new();
            let mut confidence_increased = 0;
            let mut confidence_decreased = 0;

            for (key, to_attr) in &to_node.attributes {
                if let Some(from_attr) = from_node.attributes.get(key) {
                    if from_attr.value != to_attr.value {
                        attribute_changes.push(AttributeChange {
                            attribute: key.clone(),
                            old_value: from_attr.value.clone(),
                            new_value: to_attr.value.clone(),
                            old_confidence: from_attr.confidence,
                            new_confidence: to_attr.confidence,
                        });
                    }
                    if to_attr.confidence > from_attr.confidence {
                        confidence_increased += 1;
                    } else if to_attr.confidence < from_attr.confidence {
                        confidence_decreased += 1;
                    }
                } else {
                    attribute_changes.push(AttributeChange {
                        attribute: key.clone(),
                        old_value: serde_json::Value::Null,
                        new_value: to_attr.value.clone(),
                        old_confidence: 0.0,
                        new_confidence: to_attr.confidence,
                    });
                }
            }

            if !attribute_changes.is_empty() {
                nodes_modified.push(NodeModification {
                    node_id: *id,
                    attribute_changes,
                    confidence_changes: ConfidenceChanges {
                        increased: confidence_increased,
                        decreased: confidence_decreased,
                    },
                });
            }
        }

        let from_edge_ids: HashSet<EdgeId> =
            from_snapshot.edges.iter().map(|e| e.edge_id).collect();
        let to_edge_ids: HashSet<EdgeId> = to_snapshot.edges.iter().map(|e| e.edge_id).collect();

        let edges_added: Vec<EdgeId> = to_edge_ids.difference(&from_edge_ids).cloned().collect();
        let edges_removed: Vec<EdgeId> = from_edge_ids.difference(&to_edge_ids).cloned().collect();

        let total_changes = nodes_added.len()
            + nodes_removed.len()
            + nodes_modified.len()
            + edges_added.len()
            + edges_removed.len();

        let increased_count: usize = nodes_modified
            .iter()
            .map(|m| m.confidence_changes.increased)
            .sum();
        let decreased_count: usize = nodes_modified
            .iter()
            .map(|m| m.confidence_changes.decreased)
            .sum();

        Ok(VersionDiff {
            diff_id: Uuid::new_v4(),
            from_snapshot_id,
            to_snapshot_id,
            from_timestamp: from_snapshot.metadata.created_at,
            to_timestamp: to_snapshot.metadata.created_at,
            nodes_added,
            nodes_removed,
            nodes_modified,
            edges_added,
            edges_removed,
            edges_modified: Vec::new(),
            summary: DiffSummary {
                total_changes,
                confidence_changes: ConfidenceChanges {
                    increased: increased_count,
                    decreased: decreased_count,
                },
            },
        })
    }

    /// 计算快照校验和
    ///
    /// # 算法说明
    ///
    /// 使用 SHA-256 哈希算法：
    /// - 对每个节点的 node_id、node_type、name 进行哈希
    /// - 对每个边的 edge_id、edge_type 进行哈希
    /// - 组合所有哈希值生成最终校验和
    fn compute_checksum(nodes: &[BeliefNode], edges: &[RelationEdge]) -> String {
        let mut hasher = Sha256::new();

        for node in nodes {
            hasher.update(node.node_id.to_string().as_bytes());
            hasher.update(format!("{:?}", node.node_type).as_bytes());
            hasher.update(node.name.as_bytes());
        }

        for edge in edges {
            hasher.update(edge.edge_id.to_string().as_bytes());
            hasher.update(format!("{:?}", edge.edge_type).as_bytes());
        }

        let result = hasher.finalize();
        format!("{:x}", result)
    }
}

/// 节点修改详情
///
/// # 字段说明
///
/// | 字段 | 类型 | 描述 |
/// |------|------|------|
/// | node_id | BeliefId | 被修改的节点ID |
/// | attribute_changes | Vec<AttributeChange> | 属性变更列表 |
/// | confidence_changes | ConfidenceChanges | 置信度变化统计 |
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeModification {
    pub node_id: BeliefId,
    pub attribute_changes: Vec<AttributeChange>,
    pub confidence_changes: ConfidenceChanges,
}

/// 单个属性变更记录
///
/// # 字段说明
///
/// | 字段 | 类型 | 描述 |
/// |------|------|------|
/// | attribute | String | 属性名 |
/// | old_value | JsonValue | 变更前的值 |
/// | new_value | JsonValue | 变更后的值 |
/// | old_confidence | f64 | 变更前的置信度 |
/// | new_confidence | f64 | 变更后的置信度 |
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeChange {
    pub attribute: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
    pub old_confidence: f64,
    pub new_confidence: f64,
}

/// 置信度变化统计
///
/// # 字段说明
///
/// | 字段 | 类型 | 描述 |
/// |------|------|------|
/// | increased | usize | 置信度上升的属性数量 |
/// | decreased | usize | 置信度下降的属性数量 |
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceChanges {
    pub increased: usize,
    pub decreased: usize,
}

/// 差异汇总信息
///
/// # 字段说明
///
/// | 字段 | 类型 | 描述 |
/// |------|------|------|
/// | total_changes | usize | 变更总数 |
/// | confidence_changes | ConfidenceChanges | 置信度变化统计 |
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub total_changes: usize,
    pub confidence_changes: ConfidenceChanges,
}

/// 版本差异详情
///
/// # 字段说明
///
/// | 字段 | 类型 | 描述 |
/// |------|------|------|
/// | diff_id | Uuid | 差异ID |
/// | from_snapshot_id | SnapshotId | 起始快照ID |
/// | to_snapshot_id | SnapshotId | 目标快照ID |
/// | from_timestamp | DateTime | 起始快照时间戳 |
/// | to_timestamp | DateTime | 目标快照时间戳 |
/// | nodes_added | Vec<BeliefId> | 新增节点 |
/// | nodes_removed | Vec<BeliefId> | 删除节点 |
/// | nodes_modified | Vec<NodeModification> | 修改节点 |
/// | edges_added | Vec<EdgeId> | 新增边 |
/// | edges_removed | Vec<EdgeId> | 删除边 |
/// | edges_modified | Vec<EdgeModification> | 修改边 |
/// | summary | DiffSummary | 差异汇总 |
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDiff {
    pub diff_id: Uuid,
    pub from_snapshot_id: SnapshotId,
    pub to_snapshot_id: SnapshotId,
    pub from_timestamp: chrono::DateTime<Utc>,
    pub to_timestamp: chrono::DateTime<Utc>,
    pub nodes_added: Vec<BeliefId>,
    pub nodes_removed: Vec<BeliefId>,
    pub nodes_modified: Vec<NodeModification>,
    pub edges_added: Vec<EdgeId>,
    pub edges_removed: Vec<EdgeId>,
    pub edges_modified: Vec<EdgeModification>,
    pub summary: DiffSummary,
}

/// 边修改详情
///
/// # 字段说明
///
/// | 字段 | 类型 | 描述 |
/// |------|------|------|
/// | edge_id | EdgeId | 被修改的边ID |
/// | changes | Vec<JsonValue> | 变更记录列表 |
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeModification {
    pub edge_id: EdgeId,
    pub changes: Vec<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::belief_graph::operations::BeliefGraphOperations;

    fn create_test_graph_with_data() -> BeliefGraph {
        let graph = BeliefGraph::with_default_config();

        let node1_id = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "Alice".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();

        let node2_id = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::File,
            "test.txt".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();

        BeliefGraphOperations::create_edge(&graph, RelationEdgeType::Owns, node1_id, node2_id, 0.9)
            .unwrap();

        graph
    }

    #[test]
    fn test_create_snapshot() {
        let graph = create_test_graph_with_data();

        let result = SnapshotOperations::create_snapshot(
            &graph,
            SnapshotType::Manual,
            Some("Test snapshot".to_string()),
        );

        assert!(result.is_ok());
        let snapshot_id = result.unwrap();
        assert!(!snapshot_id.is_nil());
    }

    #[test]
    fn test_get_snapshot_history() {
        let graph = create_test_graph_with_data();

        SnapshotOperations::create_snapshot(&graph, SnapshotType::Manual, None).unwrap();
        SnapshotOperations::create_snapshot(&graph, SnapshotType::Auto, None).unwrap();
        SnapshotOperations::create_snapshot(&graph, SnapshotType::Session, None).unwrap();

        let history = SnapshotOperations::get_snapshot_history(&graph, None, None);
        assert_eq!(history.len(), 3);

        let manual_only =
            SnapshotOperations::get_snapshot_history(&graph, Some(SnapshotType::Manual), None);
        assert_eq!(manual_only.len(), 1);
    }

    #[test]
    fn test_rollback() {
        let graph = create_test_graph_with_data();

        SnapshotOperations::create_snapshot(&graph, SnapshotType::Manual, None).unwrap();

        {
            let mut nodes = graph.nodes().write();
            nodes.clear();
        }

        assert_eq!(graph.node_count(), 0);

        let snapshots = graph.get_snapshots();
        let first_snapshot_id = snapshots[0].metadata.snapshot_id;

        drop(snapshots);

        let result = SnapshotOperations::rollback_to_snapshot(
            &graph,
            first_snapshot_id,
            "Test rollback".to_string(),
        );
        println!("xxx\n");
        assert!(result.is_ok());
        assert!(graph.node_count() >= 2);
    }

    #[test]
    fn test_compute_diff() {
        let graph = create_test_graph_with_data();

        SnapshotOperations::create_snapshot(&graph, SnapshotType::Manual, None).unwrap();

        BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::Tool,
            "new_tool".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::ToolReturn,
            None,
            None,
        )
        .unwrap();

        SnapshotOperations::create_snapshot(&graph, SnapshotType::Manual, None).unwrap();

        let snapshots = graph.get_snapshots();
        let from_id = snapshots[0].metadata.snapshot_id;
        let to_id = snapshots[1].metadata.snapshot_id;

        let diff = SnapshotOperations::compute_diff(&graph, from_id, to_id);

        assert!(diff.is_ok());
        let diff = diff.unwrap();
        assert_eq!(diff.nodes_added.len(), 1);
    }
}
