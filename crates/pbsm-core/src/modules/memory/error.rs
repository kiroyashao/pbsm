//! 外部记忆库错误类型定义
//!
//! 本模块定义了外部记忆库（M4）所有操作可能产生的错误类型。
//! 错误类型采用 thiserror 库实现，支持结构化错误信息和错误链。
//!
//! # 错误分类
//!
//! - **存储错误**：StorageOpen、StorageConfig、WriteFailed、ReadFailed、FlushFailed、StorageFull
//! - **事务错误**：TransactionBegin、TransactionCommit、TransactionConflict、DatabaseLocked
//! - **数据完整性错误**：ChecksumMismatch、InvalidFormat、SerializationError
//! - **资源不存在错误**：SnapshotNotFound、ExperienceNotFound、SessionNotFound
//! - **操作错误**：SchemaInit、TreeOpen、CleanupInProgress、CompressionFailed、InvalidQuery

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Storage open failed: {0} (code: STG_001)")]
    StorageOpen(String),

    #[error("Storage config error: {0} (code: STG_002)")]
    StorageConfig(String),

    #[error("Schema initialization failed: {0} (code: STG_003)")]
    SchemaInit(String),

    #[error("Tree open failed: {0} (code: STG_004)")]
    TreeOpen(String),

    #[error("Write failed: {0} (code: STG_005)")]
    WriteFailed(String),

    #[error("Read failed: {0} (code: STG_006)")]
    ReadFailed(String),

    #[error("Flush failed: {0} (code: STG_007)")]
    FlushFailed(String),

    #[error("Transaction begin failed: {0} (code: TXN_001)")]
    TransactionBegin(String),

    #[error("Transaction commit failed: {0} (code: TXN_002)")]
    TransactionCommit(String),

    #[error("Transaction conflict: {0} (code: TXN_003)")]
    TransactionConflict(String),

    #[error("Database locked: {0} (code: TXN_004)")]
    DatabaseLocked(String),

    #[error("Snapshot not found: {0} (code: NFD_001)")]
    SnapshotNotFound(String),

    #[error("Checksum mismatch: expected {expected}, actual {actual} (code: INT_001)")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Invalid format: {0} (code: VAL_002)")]
    InvalidFormat(String),

    #[error("Serialization error: {0} (code: VAL_003)")]
    SerializationError(#[from] serde_json::Error),

    #[error("Cleanup in progress (code: OPR_001)")]
    CleanupInProgress,

    #[error("Compression failed: {0} (code: OPR_002)")]
    CompressionFailed(String),

    #[error("Blocking task failed: {0} (code: OPR_E003)")]
    BlockingTaskFailed(String),

    #[error("Experience not found: {0} (code: NFD_004)")]
    ExperienceNotFound(String),

    #[error("Session not found: {0} (code: NFD_002)")]
    SessionNotFound(String),

    #[error("Invalid query: {0} (code: VAL_001)")]
    InvalidQuery(String),

    #[error("Storage full: {0} (code: STG_008)")]
    StorageFull(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_open_error() {
        let error = MemoryError::StorageOpen("path/to/db".to_string());
        assert!(error.to_string().contains("Storage open failed"));
        assert!(error.to_string().contains("path/to/db"));
        assert!(error.to_string().contains("STG_001"));
    }

    #[test]
    fn test_checksum_mismatch_error() {
        let error = MemoryError::ChecksumMismatch {
            expected: "abc123".to_string(),
            actual: "def456".to_string(),
        };
        assert!(error.to_string().contains("abc123"));
        assert!(error.to_string().contains("def456"));
        assert!(error.to_string().contains("INT_001"));
    }

    #[test]
    fn test_cleanup_in_progress_error() {
        let error = MemoryError::CleanupInProgress;
        assert!(error.to_string().contains("Cleanup in progress"));
        assert!(error.to_string().contains("OPR_001"));
    }

    #[test]
    fn test_serialization_error_from_serde() {
        let serde_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let error = MemoryError::from(serde_err);
        assert!(matches!(error, MemoryError::SerializationError(_)));
        assert!(error.to_string().contains("VAL_003"));
    }

    #[test]
    fn test_result_alias_ok() {
        let result: Result<i32> = Ok(42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_alias_err() {
        let result: Result<i32> = Err(MemoryError::SnapshotNotFound("snap-001".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_conflict_error() {
        let error = MemoryError::TransactionConflict("concurrent write".to_string());
        assert!(error.to_string().contains("concurrent write"));
        assert!(error.to_string().contains("TXN_003"));
    }

    #[test]
    fn test_storage_full_error() {
        let error = MemoryError::StorageFull("100GB limit reached".to_string());
        assert!(error.to_string().contains("100GB limit reached"));
        assert!(error.to_string().contains("STG_008"));
    }
}
