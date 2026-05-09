use thiserror::Error;

use super::state::ExecutionState;

#[derive(Error, Debug, Clone)]
pub enum IntentionStackError {
    #[error("Stack capacity exceeded: max {max}, current {current}")]
    StackCapacityExceeded { max: usize, current: usize },

    #[error("Maximum depth exceeded: max {max}, attempted {attempted}")]
    MaxDepthExceeded { max: usize, attempted: usize },

    #[error("Child limit exceeded at level {level}: max {max}, current {current}")]
    ChildLimitExceeded {
        level: usize,
        max: usize,
        current: usize,
    },

    #[error("Invalid layer index: {index}, valid range [0, {max})")]
    InvalidLayerIndex { index: usize, max: usize },

    #[error("Invalid parent level: {level}")]
    InvalidParentLevel { level: usize },

    #[error("Layer not found: {layer_id}")]
    LayerNotFound { layer_id: String },

    #[error("Invalid state transition: {from} -> {to}")]
    InvalidStateTransition {
        from: ExecutionState,
        to: ExecutionState,
    },

    #[error("Transition blocked: {reason}")]
    TransitionBlocked { reason: String },

    #[error("Push failed: {0}")]
    PushFailed(String),

    #[error("Pop failed: {0}")]
    PopFailed(String),

    #[error("Revert failed: {0}")]
    RevertFailed(String),

    #[error("Checkpoint not found: {checkpoint_id}")]
    CheckpointNotFound { checkpoint_id: String },

    #[error("Invalid goal: {reason}")]
    InvalidGoal { reason: String },

    #[error("Stack is empty")]
    StackEmpty,

    #[error("No active intention")]
    NoActiveIntention,

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, IntentionStackError>;
