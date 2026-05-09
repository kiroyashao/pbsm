//! 信念图管理器模块
//!
//! 本模块是预测性信念状态机（PBSM）中 M1 信念图管理器的实现。
//!
//! # 核心职责
//!
//! - 信念状态管理：维护 Agent 对当前环境和任务的内部理解
//! - 图结构维护：管理信念节点和关系边的创建、删除、修改操作
//! - 信念查询与检索：提供高效的信念查询接口
//! - 版本控制与回溯：维护信念状态的历史版本快照
//! - 信念融合：实现外部信念快照的融合算法
//!
//! # 性能约束
//!
//! - 信念查询响应时间 ≤10ms
//! - 信念更新响应时间 ≤20ms
//! - 支持活跃信念数量 ≥1000条
//! - 最大上下文信念节点数 ≤500个
//!
//! # 使用示例
//!
//! ```ignore
//! use pbsm_core::modules::belief_graph::{
//!     BeliefGraph, BeliefGraphHandle, BeliefGraphOperations,
//!     BeliefNodeType, SourceType, UpdateStrategy,
//! };
//!
//! let graph = BeliefGraph::with_default_config();
//! let node_id = BeliefGraphOperations::create_belief(
//!     &graph,
//!     BeliefNodeType::User,
//!     "Alice".to_string(),
//!     HashMap::new(),
//!     "test".to_string(),
//!     SourceType::UserInput,
//!     None,
//!     None,
//! ).unwrap();
//! ```

pub mod error;
pub mod graph;
pub mod types;
pub mod operations;
pub mod operations_async;
pub mod snapshot;
pub mod fusion;

pub use error::BeliefGraphError;
pub use graph::BeliefGraph;
pub use types::*;
pub use operations::BeliefGraphOperations;
pub use operations_async::BeliefGraphHandle;
pub use snapshot::SnapshotOperations;
pub use fusion::FusionOperations;
