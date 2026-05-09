//! 信念图管理器错误类型定义
//!
//! 本模块定义了信念图管理器（M1）所有操作可能产生的错误类型。
//! 错误类型采用 thiserror 库实现，支持结构化错误信息和错误链。

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BeliefGraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Node already exists: {0}")]
    NodeExists(String),

    #[error("Edge already exists: {0}")]
    EdgeExists(String),

    #[error("Graph capacity exceeded: nodes={nodes}, edges={edges}")]
    CapacityExceeded { nodes: usize, edges: usize },

    #[error("Cyclic dependency detected")]
    CyclicDependency,

    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(String),

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    #[error("Fusion failed: {0}")]
    FusionFailed(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Lock acquisition timeout")]
    LockTimeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_not_found_error() {
        let error = BeliefGraphError::NodeNotFound("node-123".to_string());
        assert_eq!(error.to_string(), "Node not found: node-123");
    }

    #[test]
    fn test_capacity_exceeded_error() {
        let error = BeliefGraphError::CapacityExceeded {
            nodes: 500,
            edges: 2000,
        };
        assert_eq!(
            error.to_string(),
            "Graph capacity exceeded: nodes=500, edges=2000"
        );
    }
}
