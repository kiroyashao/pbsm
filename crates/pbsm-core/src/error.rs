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
    #[error("Prediction not found: {message}")]
    NotFound { message: String, code: String },

    #[error("Context incomplete: {message}")]
    ContextIncomplete { message: String, code: String },

    #[error("Invalid action: {message}")]
    InvalidAction { message: String, code: String },

    #[error("Target not found: {message}")]
    TargetNotFound { message: String, code: String },

    #[error("Prediction already verified")]
    AlreadyVerified { code: String },

    #[error("Prediction expired")]
    Expired { code: String },

    #[error("State transition error: {reason}")]
    StateTransition { reason: String, code: String },

    #[error("Internal error: {context}")]
    Internal { context: String, code: String },

    #[error("Invalid observation: {message}")]
    InvalidObservation { message: String, code: String },

    #[error("Cancellation error: {message}")]
    Cancellation { message: String, code: String },
}

impl PredictionError {
    pub fn error_code(&self) -> &str {
        match self {
            PredictionError::NotFound { code, .. } => code,
            PredictionError::ContextIncomplete { code, .. } => code,
            PredictionError::InvalidAction { code, .. } => code,
            PredictionError::TargetNotFound { code, .. } => code,
            PredictionError::AlreadyVerified { code } => code,
            PredictionError::Expired { code } => code,
            PredictionError::StateTransition { code, .. } => code,
            PredictionError::Internal { code, .. } => code,
            PredictionError::InvalidObservation { code, .. } => code,
            PredictionError::Cancellation { code, .. } => code,
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            PredictionError::NotFound { .. } => 404,
            PredictionError::ContextIncomplete { .. } => 400,
            PredictionError::InvalidAction { .. } => 400,
            PredictionError::TargetNotFound { .. } => 404,
            PredictionError::AlreadyVerified { .. } => 409,
            PredictionError::Expired { .. } => 410,
            PredictionError::StateTransition { .. } => 409,
            PredictionError::Internal { .. } => 500,
            PredictionError::InvalidObservation { .. } => 400,
            PredictionError::Cancellation { .. } => 409,
        }
    }
}

impl PartialEq for PredictionError {
    fn eq(&self, other: &Self) -> bool {
        let result = matches!(
            (self, other),
            (
                PredictionError::NotFound { .. }
                    | PredictionError::AlreadyVerified { .. }
                    | PredictionError::Expired { .. },
                PredictionError::NotFound { .. }
                    | PredictionError::AlreadyVerified { .. }
                    | PredictionError::Expired { .. }
            ) | (
                PredictionError::ContextIncomplete { .. }
                    | PredictionError::InvalidAction { .. }
                    | PredictionError::TargetNotFound { .. }
                    | PredictionError::StateTransition { .. }
                    | PredictionError::Internal { .. }
                    | PredictionError::InvalidObservation { .. }
                    | PredictionError::Cancellation { .. },
                PredictionError::ContextIncomplete { .. }
                    | PredictionError::InvalidAction { .. }
                    | PredictionError::TargetNotFound { .. }
                    | PredictionError::StateTransition { .. }
                    | PredictionError::Internal { .. }
                    | PredictionError::InvalidObservation { .. }
                    | PredictionError::Cancellation { .. },
            )
        );
        result
    }
}
