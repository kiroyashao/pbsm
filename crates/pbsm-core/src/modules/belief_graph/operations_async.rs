//! 信念图异步操作实现
//!
//! 本模块提供了信念图的异步接口封装，用于异步环境下的信念图操作。
//!
//! # 设计说明
//!
//! BeliefGraphHandle 基于 Arc<BeliefGraph> 提供线程安全的共享访问。
//! 所有修改操作通过 task::spawn_blocking 调度到阻塞线程池执行，
//! 以避免阻塞异步运行时。
//!
//! # 异步封装策略
//!
//! | 操作类型 | 封装方式 |
//! |---------|----------|
//! | 信念 CRUD | spawn_blocking → BeliefGraphOperations |
//! | 查询操作 | spawn_blocking → BeliefGraphOperations |
//! | 同步访问 | 直接调用 Arc 方法 |

use std::collections::HashMap;
use std::sync::Arc;

use tokio::task;

use super::error::BeliefGraphError;
use super::graph::BeliefGraph;
use super::types::*;

pub type Result<T> = std::result::Result<T, BeliefGraphError>;

/// 信念图异步句柄
///
/// # 设计说明
///
/// BeliefGraphHandle 提供信念图的异步接口封装：
/// - 内部持有 Arc<BeliefGraph>，支持多线程共享
/// - 所有修改操作通过 spawn_blocking 调度到阻塞线程执行
/// - 提供同步便捷方法用于只读或清理操作
///
/// # 用法示例
///
/// ```ignore
/// let handle = BeliefGraphHandle::new(BeliefGraph::with_default_config());
/// let node_id = handle.create_belief(...).await?;
/// ```
pub struct BeliefGraphHandle {
    inner: Arc<BeliefGraph>,
}

impl BeliefGraphHandle {
    /// 创建新的异步句柄
    ///
    /// # 参数
    /// * `graph` - 要包装的信念图
    ///
    /// # 返回
    /// * `BeliefGraphHandle` - 异步句柄
    pub fn new(graph: BeliefGraph) -> Self {
        Self {
            inner: Arc::new(graph),
        }
    }

    /// 从已有的 Arc 创建句柄
    ///
    /// # 参数
    /// * `arc` - 已有的 Arc<BeliefGraph>
    ///
    /// # 返回
    /// * `BeliefGraphHandle` - 异步句柄
    pub fn from_arc(arc: Arc<BeliefGraph>) -> Self {
        Self { inner: arc }
    }

    /// 获取内部图的 Arc 引用
    pub fn graph(&self) -> Arc<BeliefGraph> {
        Arc::clone(&self.inner)
    }

    /// 异步创建信念节点
    ///
    /// # 参数
    /// * `node_type` - 节点类型
    /// * `name` - 节点名称
    /// * `attributes` - 初始属性
    /// * `source` - 来源标识
    /// * `source_type` - 来源类型
    /// * `tags` - 标签列表（可选）
    /// * `initial_confidence` - 初始置信度（可选）
    ///
    /// # 返回
    /// * `Ok(BeliefId)` - 新创建的节点ID
    /// * `Err(BeliefGraphError)` - 创建失败
    #[allow(clippy::too_many_arguments)]
    pub async fn create_belief(
        &self,
        node_type: BeliefNodeType,
        name: String,
        attributes: HashMap<String, AttributeValue>,
        source: String,
        source_type: SourceType,
        tags: Option<Vec<String>>,
        initial_confidence: Option<f64>,
    ) -> Result<BeliefId> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::operations::BeliefGraphOperations::create_belief(
                &graph,
                node_type,
                name,
                attributes,
                source,
                source_type,
                tags,
                initial_confidence,
            )
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步更新信念节点
    ///
    /// # 参数
    /// * `node_id` - 要更新的节点ID
    /// * `updates` - 属性更新
    /// * `strategy` - 更新策略
    ///
    /// # 返回
    /// * `Ok(UpdateResult)` - 更新结果
    /// * `Err(BeliefGraphError)` - 更新失败
    pub async fn update_belief(
        &self,
        node_id: BeliefId,
        updates: HashMap<String, AttributeValue>,
        strategy: UpdateStrategy,
    ) -> Result<UpdateResult> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::operations::BeliefGraphOperations::update_belief(
                &graph, node_id, updates, strategy,
            )
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步删除信念节点
    ///
    /// # 参数
    /// * `node_id` - 要删除的节点ID
    /// * `cascade` - 是否级联删除关联边
    ///
    /// # 返回
    /// * `Ok(DeleteResult)` - 删除结果
    /// * `Err(BeliefGraphError)` - 删除失败
    pub async fn delete_belief(&self, node_id: BeliefId, cascade: bool) -> Result<DeleteResult> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::operations::BeliefGraphOperations::delete_belief(&graph, node_id, cascade)
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步查询信念
    ///
    /// # 参数
    /// * `spec` - 查询规格
    /// * `options` - 查询选项
    ///
    /// # 返回
    /// * `Ok(QueryResult)` - 查询结果
    /// * `Err(BeliefGraphError)` - 查询失败
    pub async fn query(
        &self,
        spec: QuerySpecification,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        let graph = self.inner.clone();
        let start = std::time::Instant::now();
        task::spawn_blocking(move || {
            super::operations::BeliefGraphOperations::query(&graph, spec, options)
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
        .map(|mut result| {
            result.execution_time_ms = start.elapsed().as_secs_f64() * 1000.0;
            result
        })
    }

    /// 异步创建关系边
    ///
    /// # 参数
    /// * `edge_type` - 边类型
    /// * `source_node` - 源节点ID
    /// * `target_node` - 目标节点ID
    /// * `confidence` - 置信度
    ///
    /// # 返回
    /// * `Ok(EdgeId)` - 新创建的边ID
    /// * `Err(BeliefGraphError)` - 创建失败
    pub async fn create_edge(
        &self,
        edge_type: RelationEdgeType,
        source_node: BeliefId,
        target_node: BeliefId,
        confidence: f64,
    ) -> Result<EdgeId> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::operations::BeliefGraphOperations::create_edge(
                &graph,
                edge_type,
                source_node,
                target_node,
                confidence,
            )
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步删除关系边
    ///
    /// # 参数
    /// * `edge_id` - 要删除的边ID
    ///
    /// # 返回
    /// * `Ok(EdgeId)` - 已删除的边ID
    /// * `Err(BeliefGraphError)` - 删除失败
    pub async fn delete_edge(&self, edge_id: EdgeId) -> Result<EdgeId> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::operations::BeliefGraphOperations::delete_edge(&graph, edge_id)
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步图遍历
    ///
    /// # 参数
    /// * `start_node_id` - 起始节点ID
    /// * `max_depth` - 最大遍历深度
    /// * `edge_types` - 边类型过滤器（可选）
    /// * `direction` - 遍历方向
    ///
    /// # 返回
    /// * `Ok(TraversalResult)` - 遍历结果
    /// * `Err(BeliefGraphError)` - 遍历失败
    pub async fn traverse_graph(
        &self,
        start_node_id: BeliefId,
        max_depth: u32,
        edge_types: Option<Vec<RelationEdgeType>>,
        direction: EdgeDirection,
    ) -> Result<TraversalResult> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::operations::BeliefGraphOperations::traverse_graph(
                &graph,
                start_node_id,
                max_depth,
                edge_types,
                direction,
                true,
            )
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步信念推导
    ///
    /// # 参数
    /// * `start_node_id` - 起始节点ID
    /// * `target_attributes` - 目标属性列表
    /// * `inference_depth` - 推理深度
    /// * `confidence_threshold` - 置信度阈值
    ///
    /// # 返回
    /// * `Ok(Vec<DerivationResult>)` - 推导结果列表
    /// * `Err(BeliefGraphError)` - 推导失败
    pub async fn derive_beliefs(
        &self,
        start_node_id: BeliefId,
        target_attributes: Vec<String>,
        inference_depth: u32,
        confidence_threshold: f64,
    ) -> Result<Vec<DerivationResult>> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::operations::BeliefGraphOperations::derive_beliefs(
                &graph,
                start_node_id,
                target_attributes,
                inference_depth,
                confidence_threshold,
            )
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步创建快照
    ///
    /// # 参数
    /// * `snapshot_type` - 快照类型
    /// * `description` - 快照描述（可选）
    ///
    /// # 返回
    /// * `Ok(SnapshotId)` - 新创建的快照ID
    /// * `Err(BeliefGraphError)` - 创建失败
    pub async fn create_snapshot(
        &self,
        snapshot_type: SnapshotType,
        description: Option<String>,
    ) -> Result<SnapshotId> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::snapshot::SnapshotOperations::create_snapshot(&graph, snapshot_type, description)
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步回滚到指定快照
    ///
    /// # 参数
    /// * `snapshot_id` - 目标快照ID
    /// * `reason` - 回滚原因
    ///
    /// # 返回
    /// * `Ok(RollbackResult)` - 回滚结果
    /// * `Err(BeliefGraphError)` - 回滚失败
    pub async fn rollback_to_snapshot(
        &self,
        snapshot_id: SnapshotId,
        reason: String,
    ) -> Result<RollbackResult> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::snapshot::SnapshotOperations::rollback_to_snapshot(&graph, snapshot_id, reason)
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 异步融合信念快照
    ///
    /// # 参数
    /// * `external_snapshot` - 外部信念快照
    /// * `fusion_config` - 融合配置（可选）
    ///
    /// # 返回
    /// * `Ok(FusionResult)` - 融合结果
    /// * `Err(BeliefGraphError)` - 融合失败
    pub async fn fuse_belief_snapshot(
        &self,
        external_snapshot: BeliefSnapshot,
        fusion_config: Option<FusionConfig>,
    ) -> Result<FusionResult> {
        let graph = self.inner.clone();
        task::spawn_blocking(move || {
            super::fusion::FusionOperations::fuse_belief_snapshot(
                &graph,
                external_snapshot,
                fusion_config,
            )
        })
        .await
        .map_err(|_| BeliefGraphError::InternalError("Task join error".into()))?
    }

    /// 同步获取节点
    ///
    /// # 参数
    /// * `node_id` - 节点ID
    ///
    /// # 返回
    /// * `Option<BeliefNode>` - 节点（如果存在）
    pub fn get_node_sync(&self, node_id: BeliefId) -> Option<BeliefNode> {
        self.inner.get_node(node_id)
    }

    /// 同步获取节点数量
    pub fn get_node_count_sync(&self) -> usize {
        self.inner.node_count()
    }

    /// 同步获取边数量
    pub fn get_edge_count_sync(&self) -> usize {
        self.inner.edge_count()
    }

    /// 同步获取统计信息
    pub fn get_statistics_sync(&self) -> GraphStatistics {
        self.inner.get_statistics()
    }

    /// 同步清空图
    pub fn clear_sync(&self) {
        self.inner.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_handle() -> BeliefGraphHandle {
        BeliefGraphHandle::new(BeliefGraph::with_default_config())
    }

    #[tokio::test]
    async fn test_async_create_belief() {
        let handle = create_test_handle();

        let result = handle
            .create_belief(
                BeliefNodeType::User,
                "TestUser".to_string(),
                HashMap::new(),
                "test".to_string(),
                SourceType::UserInput,
                None,
                None,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_update_belief() {
        let handle = create_test_handle();

        let node_id = handle
            .create_belief(
                BeliefNodeType::File,
                "test.txt".to_string(),
                HashMap::new(),
                "test".to_string(),
                SourceType::DirectObservation,
                None,
                None,
            )
            .await
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

        let result = handle
            .update_belief(node_id, updates, UpdateStrategy::Overwrite)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_delete_belief() {
        let handle = create_test_handle();

        let node_id = handle
            .create_belief(
                BeliefNodeType::Tool,
                "my_tool".to_string(),
                HashMap::new(),
                "test".to_string(),
                SourceType::ToolReturn,
                None,
                None,
            )
            .await
            .unwrap();

        let result = handle.delete_belief(node_id, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_create_edge() {
        let handle = create_test_handle();

        let source = handle
            .create_belief(
                BeliefNodeType::User,
                "Bob".to_string(),
                HashMap::new(),
                "test".to_string(),
                SourceType::UserInput,
                None,
                None,
            )
            .await
            .unwrap();

        let target = handle
            .create_belief(
                BeliefNodeType::File,
                "doc.txt".to_string(),
                HashMap::new(),
                "test".to_string(),
                SourceType::UserInput,
                None,
                None,
            )
            .await
            .unwrap();

        let edge_id = handle
            .create_edge(RelationEdgeType::Owns, source, target, 0.9)
            .await;

        assert!(edge_id.is_ok());
    }

    #[tokio::test]
    async fn test_async_query() {
        let handle = create_test_handle();

        handle
            .create_belief(
                BeliefNodeType::User,
                "User1".to_string(),
                HashMap::new(),
                "test".to_string(),
                SourceType::UserInput,
                None,
                None,
            )
            .await
            .unwrap();

        let spec = QuerySpecification {
            query_type: QueryType::ByType,
            node_type: Some(BeliefNodeType::User),
            ..Default::default()
        };

        let result = handle.query(spec, QueryOptions::default()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().items.len(), 1);
    }

    #[tokio::test]
    async fn test_async_snapshot() {
        let handle = create_test_handle();

        handle
            .create_belief(
                BeliefNodeType::User,
                "SnapUser".to_string(),
                HashMap::new(),
                "test".to_string(),
                SourceType::UserInput,
                None,
                None,
            )
            .await
            .unwrap();

        let snapshot_id = handle
            .create_snapshot(SnapshotType::Manual, Some("Test".to_string()))
            .await;

        assert!(snapshot_id.is_ok());
    }
}
