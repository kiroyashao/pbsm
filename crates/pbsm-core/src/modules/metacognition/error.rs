use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum MetacognitiveError {
    #[error("Invalid parameter: {field} (code: MCA-E001)")]
    InvalidParameter { field: String },

    #[error(
        "Attention parameter out of bounds: {value} (valid range: [{min}, {max}]) (code: MCA-E002)"
    )]
    AttentionOutOfBounds { value: f64, min: f64, max: f64 },

    #[error("Weight validation failed: {reason} (code: MCA-E003)")]
    WeightValidationFailed { reason: String },

    #[error("Belief not found: {node_id} (code: MCA-E004)")]
    BeliefNotFound { node_id: String },

    #[error("Protected belief cannot be forgotten: {node_id} (code: MCA-E005)")]
    ForgetProtectedBelief { node_id: String },

    #[error("External memory error: {0} (code: MCA-E006)")]
    ExternalMemoryError(String),

    #[error("Anomaly detection failed: {0} (code: MCA-E007)")]
    AnomalyDetectionFailed(String),

    #[error("Configuration error: {0} (code: MCA-E008)")]
    ConfigurationError(String),

    #[error("Internal error: {0} (code: MCA-E999)")]
    InternalError(String),
}

impl PartialEq for MetacognitiveError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                MetacognitiveError::InvalidParameter { field: a },
                MetacognitiveError::InvalidParameter { field: b },
            ) => a == b,
            (
                MetacognitiveError::AttentionOutOfBounds {
                    value: v1,
                    min: m1,
                    max: x1,
                },
                MetacognitiveError::AttentionOutOfBounds {
                    value: v2,
                    min: m2,
                    max: x2,
                },
            ) => v1 == v2 && m1 == m2 && x1 == x2,
            (
                MetacognitiveError::WeightValidationFailed { reason: a },
                MetacognitiveError::WeightValidationFailed { reason: b },
            ) => a == b,
            (
                MetacognitiveError::BeliefNotFound { node_id: a },
                MetacognitiveError::BeliefNotFound { node_id: b },
            ) => a == b,
            (
                MetacognitiveError::ForgetProtectedBelief { node_id: a },
                MetacognitiveError::ForgetProtectedBelief { node_id: b },
            ) => a == b,
            (
                MetacognitiveError::ExternalMemoryError(a),
                MetacognitiveError::ExternalMemoryError(b),
            ) => a == b,
            (
                MetacognitiveError::AnomalyDetectionFailed(a),
                MetacognitiveError::AnomalyDetectionFailed(b),
            ) => a == b,
            (
                MetacognitiveError::ConfigurationError(a),
                MetacognitiveError::ConfigurationError(b),
            ) => a == b,
            (MetacognitiveError::InternalError(a), MetacognitiveError::InternalError(b)) => a == b,
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, MetacognitiveError>;
