//! 预测引擎错误类型定义
//!
//! 本模块定义了预测引擎（M2）所有操作可能产生的错误类型。
//! 错误类型采用 thiserror 库实现，支持结构化错误信息和错误链。
//!
//! # 错误分类
//!
//! - **查找错误**：PredictionNotFound、TargetNotFound
//! - **状态错误**：AlreadyVerified、Expired、StateTransition
//! - **输入错误**：ContextIncomplete、InvalidAction、InvalidObservation
//! - **系统错误**：Internal、Cancellation
//!
//! # 使用方式
//!
//! 大多数错误会直接返回给调用者，由上层逻辑决定如何处理。
//! 状态相关的错误（如 AlreadyVerified）通常表示操作流程异常。

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum PredictionError {
    #[error("Prediction not found: {0}")]
    NotFound(String),

    #[error("Context incomplete: {0}")]
    ContextIncomplete(String),

    #[error("Invalid action: {0}")]
    InvalidAction(String),

    #[error("Target not found: {0}")]
    TargetNotFound(String),

    #[error("Prediction already verified")]
    AlreadyVerified,

    #[error("Prediction expired")]
    Expired,

    #[error("State transition error: {reason}")]
    StateTransition { reason: String },

    #[error("Internal error: {context}")]
    Internal { context: String },

    #[error("Invalid observation: {0}")]
    InvalidObservation(String),

    #[error("Cancellation error: {0}")]
    Cancellation(String),
}

impl PartialEq for PredictionError {
    fn eq(&self, other: &Self) -> bool {
        let result = matches!(
            (self, other),
            (
                PredictionError::NotFound(_)
                    | PredictionError::AlreadyVerified
                    | PredictionError::Expired,
                PredictionError::NotFound(_)
                    | PredictionError::AlreadyVerified
                    | PredictionError::Expired
            ) | (
                PredictionError::ContextIncomplete(_)
                    | PredictionError::InvalidAction(_)
                    | PredictionError::TargetNotFound(_)
                    | PredictionError::StateTransition { .. }
                    | PredictionError::Internal { .. }
                    | PredictionError::InvalidObservation(_)
                    | PredictionError::Cancellation(_),
                PredictionError::ContextIncomplete(_)
                    | PredictionError::InvalidAction(_)
                    | PredictionError::TargetNotFound(_)
                    | PredictionError::StateTransition { .. }
                    | PredictionError::Internal { .. }
                    | PredictionError::InvalidObservation(_)
                    | PredictionError::Cancellation(_),
            )
        );
        result
    }
}
