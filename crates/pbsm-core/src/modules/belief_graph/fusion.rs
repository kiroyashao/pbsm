//! 信念融合功能实现
//!
//! 本模块实现了信念融合算法，用于处理来自外部记忆库或跨Agent通信的信念快照的合并。
//!
//! # 核心职责
//!
//! - 实现信念快照的自动合并与冲突解决
//! - 支持多种融合策略以适应不同场景
//! - 追踪融合过程中的冲突记录
//!
//! # 融合策略说明
//!
//! |         策略        |    描述    |    适用场景       |
//! |--------------------|------------|-----------------|
//! | HighConfidenceWins | 高置信度优先 | 已知外部源更可靠   |
//! | SourcePriority     | 来源优先级   | 优先信任特定来源   |
//! | MostRecent         | 最新优先     | 时效性重要的场景   |
//! | TimeDecay          | 时间衰减     | 历史信念自然退化   |
//! | ManualReview       | 人工审核     | 需要人工介入的冲突 |
//!
//! # 使用流程
//!
//! 1. 外部Agent或记忆库发送信念快照
//! 2. 调用 `fuse_belief_snapshot` 执行融合
//! 3. 系统根据策略自动解决冲突
//! 4. 返回融合结果和未解决的冲突记录

use std::collections::HashMap;

use super::error::BeliefGraphError;
use super::graph::BeliefGraph;
use super::operations::BeliefGraphOperations;
use super::types::*;

pub type Result<T> = std::result::Result<T, BeliefGraphError>;

/// 信念融合操作提供者
///
/// # 设计说明
///
/// FusionOperations 封装了所有信念融合相关的逻辑：
/// - 外部快照与本地信念的匹配
/// - 多策略冲突解决算法
/// - 融合统计与结果报告
pub struct FusionOperations;

impl FusionOperations {
    /// 执行信念快照融合
    ///
    /// # 参数
    /// * `graph` - 目标信念图
    /// * `external_snapshot` - 外部信念快照
    /// * `fusion_config` - 融合配置（可选，使用默认配置）
    ///
    /// # 返回
    /// * `Ok(FusionResult)` - 融合结果
    /// * `Err(BeliefGraphError)` - 融合失败
    ///
    /// # 处理流程
    ///
    /// 1. 提取本地信念图中的所有节点构建映射表
    /// 2. 遍历外部快照中的每个节点：
    ///    - 若本地存在匹配节点，按策略合并属性
    ///    - 若本地不存在，创建新节点
    /// 3. 遍历外部快照中的每条边：
    ///    - 若本地不存在该边，创建新边
    /// 4. 返回融合统计和未解决的冲突
    pub fn fuse_belief_snapshot(
        graph: &BeliefGraph,
        external_snapshot: BeliefSnapshot,
        fusion_config: Option<FusionConfig>,
    ) -> Result<FusionResult> {
        let config = fusion_config.unwrap_or_default();
        let strategy = config.strategy.unwrap_or_default();

        let nodes = graph.nodes().read();
        let mut local_node_map: HashMap<BeliefId, BeliefNode> = HashMap::new();
        for (id, node) in nodes.iter() {
            local_node_map.insert(*id, node.clone());
        }
        drop(nodes);

        let mut stats = FusionStatistics {
            nodes_processed: 0,
            nodes_created: 0,
            nodes_updated: 0,
            conflicts_resolved: 0,
            conflicts_deferred: 0,
        };

        let mut deferred_conflicts: Vec<ConflictRecord> = Vec::new();

        for external_node in &external_snapshot.nodes {
            stats.nodes_processed += 1;

            if let Some(local_node) = local_node_map.get(&external_node.node_id) {
                let (updated, conflicts) =
                    Self::merge_node(local_node, external_node, strategy, &config);

                if updated {
                    stats.nodes_updated += 1;

                    let mut updates = HashMap::new();
                    for (key, attr) in &external_node.attributes {
                        updates.insert(
                            key.clone(),
                            AttributeValue {
                                value: attr.value.clone(),
                                confidence: attr.confidence,
                                last_updated: attr.last_updated,
                                source: attr.source.clone(),
                                source_type: SourceType::AgentSync,
                            },
                        );
                    }

                    let _ = BeliefGraphOperations::update_belief(
                        graph,
                        local_node.node_id,
                        updates,
                        UpdateStrategy::ConditionalReplace,
                    );
                }

                stats.conflicts_resolved += conflicts.resolved;
                stats.conflicts_deferred += conflicts.deferred;
                deferred_conflicts.extend(conflicts.deferred_records);
            } else {
                let new_attributes: HashMap<String, AttributeValue> = external_node
                    .attributes
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            AttributeValue {
                                value: v.value.clone(),
                                confidence: v.confidence * 0.8,
                                last_updated: v.last_updated,
                                source: v.source.clone(),
                                source_type: SourceType::AgentSync,
                            },
                        )
                    })
                    .collect();

                if BeliefGraphOperations::create_belief(
                    graph,
                    external_node.node_type,
                    external_node.name.clone(),
                    new_attributes,
                    "agent_sync".to_string(),
                    SourceType::AgentSync,
                    if external_node.metadata.tags.is_empty() {
                        None
                    } else {
                        Some(external_node.metadata.tags.clone())
                    },
                    None,
                )
                .is_ok()
                {
                    stats.nodes_created += 1;
                }
            }
        }

        for external_edge in &external_snapshot.edges {
            if graph.get_edge(external_edge.edge_id).is_none() {
                let _ = BeliefGraphOperations::create_edge(
                    graph,
                    external_edge.edge_type,
                    external_edge.source_node,
                    external_edge.target_node,
                    external_edge.confidence * 0.8,
                );
            }
        }

        graph.publish_event(crate::modules::common::BeliefGraphEvent::FusionCompleted {
            snapshot_id: "external".to_string(),
            merged_count: stats.nodes_updated,
            conflict_count: stats.conflicts_resolved + stats.conflicts_deferred,
        });

        Ok(FusionResult {
            success: true,
            statistics: Some(stats),
            deferred_conflicts: if deferred_conflicts.is_empty() {
                None
            } else {
                Some(deferred_conflicts)
            },
        })
    }

    /// 合并单个信念节点
    ///
    /// # 参数
    /// * `local` - 本地信念节点
    /// * `external` - 外部信念节点
    /// * `strategy` - 冲突解决策略
    /// * `config` - 融合配置
    ///
    /// # 返回
    /// * `(bool, ConflictStats)` - 是否需要更新，以及冲突统计
    ///
    /// # 策略处理逻辑
    ///
    /// - **HighConfidenceWins**: 比较置信度，外部高于本地时更新
    /// - **SourcePriority**: 比较来源优先级，外部更高时更新
    /// - **MostRecent**: 比较时间戳，外部更新时更新
    /// - **TimeDecay**: 对本地置信度施加时间衰减后比较
    /// - **ManualReview**: 记录冲突但不更新，等待人工处理
    fn merge_node(
        local: &BeliefNode,
        external: &BeliefNode,
        strategy: ResolutionStrategy,
        _config: &FusionConfig,
    ) -> (bool, ConflictStats) {
        let mut updated = false;
        let mut stats = ConflictStats::default();
        let mut deferred_records: Vec<ConflictRecord> = Vec::new();

        for (key, external_attr) in &external.attributes {
            if let Some(local_attr) = local.attributes.get(key) {
                let conflict = local_attr.value != external_attr.value;

                if conflict {
                    match strategy {
                        ResolutionStrategy::HighConfidenceWins => {
                            if external_attr.confidence > local_attr.confidence {
                                updated = true;
                            }
                            stats.resolved += 1;
                        }
                        ResolutionStrategy::SourcePriority => {
                            let local_priority = Self::get_source_priority(local_attr.source_type);
                            let external_priority =
                                Self::get_source_priority(external_attr.source_type);
                            if external_priority > local_priority {
                                updated = true;
                            }
                            stats.resolved += 1;
                        }
                        ResolutionStrategy::MostRecent => {
                            if external_attr.last_updated > local_attr.last_updated {
                                updated = true;
                            }
                            stats.resolved += 1;
                        }
                        ResolutionStrategy::TimeDecay => {
                            let age_hours =
                                (chrono::Utc::now() - local_attr.last_updated).num_hours() as f64;
                            let decay_rate = 0.01;
                            let adjusted_local =
                                local_attr.confidence * (-decay_rate * age_hours).exp().max(0.3);

                            if external_attr.confidence > adjusted_local {
                                updated = true;
                            }
                            stats.resolved += 1;
                        }
                        ResolutionStrategy::ManualReview => {
                            deferred_records.push(ConflictRecord {
                                node_id: local.node_id,
                                attribute: key.clone(),
                                local_value: local_attr.value.clone(),
                                external_value: external_attr.value.clone(),
                                local_confidence: local_attr.confidence,
                                external_confidence: external_attr.confidence,
                            });
                            stats.deferred += 1;
                        }
                    }
                }
            } else {
                updated = true;
            }
        }

        (
            updated,
            ConflictStats {
                resolved: stats.resolved,
                deferred: stats.deferred,
                deferred_records,
            },
        )
    }

    /// 获取信念来源的优先级
    ///
    /// # 参数
    /// * `source_type` - 信念来源类型
    ///
    /// # 返回
    /// * `f64` - 优先级分数（0.0-1.0）
    ///
    /// # 优先级排序
    ///
    /// 1. DirectObservation (1.0) - 直接观察，最可靠
    /// 2. ToolReturn (0.9) - 工具返回
    /// 3. UserInput (0.85) - 用户输入
    /// 4. MemoryRestore (0.7) - 记忆恢复
    /// 5. AgentSync (0.65) - Agent同步
    /// 6. Derived (0.5) - 推导得出，最低
    fn get_source_priority(source_type: SourceType) -> f64 {
        match source_type {
            SourceType::DirectObservation => 1.0,
            SourceType::ToolReturn => 0.9,
            SourceType::UserInput => 0.85,
            SourceType::MemoryRestore => 0.7,
            SourceType::AgentSync => 0.65,
            SourceType::Derived => 0.5,
        }
    }
}

/// 冲突统计信息
///
/// # 字段说明
///
/// - `resolved`: 已解决的冲突数量
/// - `deferred`: 延后处理的冲突数量
/// - `deferred_records`: 等待人工审核的冲突记录
#[derive(Default)]
struct ConflictStats {
    resolved: usize,
    deferred: usize,
    deferred_records: Vec<ConflictRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::belief_graph::operations::BeliefGraphOperations;

    fn create_test_graph() -> BeliefGraph {
        BeliefGraph::with_default_config()
    }

    #[test]
    fn test_fuse_belief_snapshot_new_nodes() {
        let graph = create_test_graph();

        let external_snapshot = BeliefSnapshot {
            nodes: vec![BeliefNode::new(
                BeliefNodeType::User,
                "ExternalUser".to_string(),
                "external".to_string(),
                SourceType::AgentSync,
            )],
            edges: vec![],
        };

        let result = FusionOperations::fuse_belief_snapshot(&graph, external_snapshot, None);

        assert!(result.is_ok());
        let fusion_result = result.unwrap();
        assert!(fusion_result.success);
        assert_eq!(fusion_result.statistics.unwrap().nodes_created, 1);
    }

    #[test]
    fn test_fuse_belief_snapshot_high_confidence_wins() {
        let graph = create_test_graph();

        let _node_id = BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::File,
            "shared_file.txt".to_string(),
            HashMap::new(),
            "local".to_string(),
            SourceType::DirectObservation,
            None,
            Some(0.7),
        )
        .unwrap();

        let external_node = BeliefNode::new(
            BeliefNodeType::File,
            "shared_file.txt".to_string(),
            "external".to_string(),
            SourceType::ToolReturn,
        );

        let external_snapshot = BeliefSnapshot {
            nodes: vec![external_node],
            edges: vec![],
        };

        let config = FusionConfig {
            strategy: Some(ResolutionStrategy::HighConfidenceWins),
            conflict_callback: None,
            preserve_local_metadata: Some(true),
        };

        let result =
            FusionOperations::fuse_belief_snapshot(&graph, external_snapshot, Some(config));

        assert!(result.is_ok());
    }

    #[test]
    fn test_fuse_with_edges() {
        let graph = create_test_graph();

        let node1 = BeliefGraphOperations::create_belief(
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

        let node2 = BeliefGraphOperations::create_belief(
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

        BeliefGraphOperations::create_edge(&graph, RelationEdgeType::Owns, node1, node2, 0.9)
            .unwrap();

        let external_snapshot = BeliefSnapshot {
            nodes: vec![BeliefNode::new(
                BeliefNodeType::User,
                "ExternalUser".to_string(),
                "external".to_string(),
                SourceType::AgentSync,
            )],
            edges: vec![RelationEdge::new(RelationEdgeType::Owns, node1, node2, 0.9)],
        };

        let result = FusionOperations::fuse_belief_snapshot(&graph, external_snapshot, None);

        assert!(result.is_ok());
    }
}
