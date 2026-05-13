use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum CommunicationError {
    #[error("Snapshot construction failed: {0}")]
    SnapshotConstructionFailed(String),

    #[error("Snapshot parsing failed: {0}")]
    SnapshotParsingFailed(String),

    #[error("Snapshot verification failed: {0}")]
    SnapshotVerificationFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Sync timeout: {0}")]
    SyncTimeout(String),

    #[error("Sync rejected: {0}")]
    SyncRejected(String),

    #[error("Version mismatch: expected {expected}, actual {actual}")]
    VersionMismatch { expected: String, actual: String },

    #[error("Expired snapshot: {0}")]
    ExpiredSnapshot(String),

    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("Conflict detected: {0}")]
    ConflictDetected(String),

    #[error("Negotiation timeout: {0}")]
    NegotiationTimeout(String),

    #[error("Delegation failed: {0}")]
    DelegationFailed(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Internal error: {context}")]
    InternalError { context: String },
}

impl From<serde_json::Error> for CommunicationError {
    fn from(err: serde_json::Error) -> Self {
        CommunicationError::SerializationError(err.to_string())
    }
}

impl PartialEq for CommunicationError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (
                CommunicationError::SnapshotConstructionFailed(_),
                CommunicationError::SnapshotConstructionFailed(_)
            ) | (
                CommunicationError::SnapshotParsingFailed(_),
                CommunicationError::SnapshotParsingFailed(_)
            ) | (
                CommunicationError::SnapshotVerificationFailed(_),
                CommunicationError::SnapshotVerificationFailed(_)
            ) | (
                CommunicationError::SerializationError(_),
                CommunicationError::SerializationError(_)
            ) | (
                CommunicationError::SyncTimeout(_),
                CommunicationError::SyncTimeout(_)
            ) | (
                CommunicationError::SyncRejected(_),
                CommunicationError::SyncRejected(_)
            ) | (
                CommunicationError::VersionMismatch { .. },
                CommunicationError::VersionMismatch { .. }
            ) | (
                CommunicationError::ExpiredSnapshot(_),
                CommunicationError::ExpiredSnapshot(_)
            ) | (
                CommunicationError::SignatureVerificationFailed(_),
                CommunicationError::SignatureVerificationFailed(_)
            ) | (
                CommunicationError::AccessDenied(_),
                CommunicationError::AccessDenied(_)
            ) | (
                CommunicationError::SecurityViolation(_),
                CommunicationError::SecurityViolation(_)
            ) | (
                CommunicationError::ConflictDetected(_),
                CommunicationError::ConflictDetected(_)
            ) | (
                CommunicationError::NegotiationTimeout(_),
                CommunicationError::NegotiationTimeout(_)
            ) | (
                CommunicationError::DelegationFailed(_),
                CommunicationError::DelegationFailed(_)
            ) | (
                CommunicationError::NetworkError(_),
                CommunicationError::NetworkError(_)
            ) | (
                CommunicationError::InternalError { .. },
                CommunicationError::InternalError { .. }
            )
        )
    }
}
