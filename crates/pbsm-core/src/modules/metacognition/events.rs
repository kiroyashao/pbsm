use serde::{Deserialize, Serialize};

use super::types::{AdjustmentTrigger, AnomalySeverity, AnomalyType, AttentionMode, ValueFactors};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetacognitiveEvent {
    AttentionAdjusted {
        previous_value: f64,
        new_value: f64,
        trigger: AdjustmentTrigger,
        mode: AttentionMode,
        adjustment_magnitude: f64,
    },
    AttentionModeChanged {
        previous_mode: AttentionMode,
        new_mode: AttentionMode,
        reason: String,
    },
    AttentionAnomalyDetected {
        anomaly_type: AnomalyType,
        severity: AnomalySeverity,
        evidence: serde_json::Value,
    },
    AttentionAnomalyResolved {
        anomaly_type: AnomalyType,
        resolution: String,
    },
    MemoryValueCalculated {
        node_id: String,
        value_score: f64,
        factors: ValueFactors,
    },
    ForgetTriggered {
        node_ids: Vec<String>,
        reason: String,
        count: usize,
    },
    ForgetCompleted {
        archived_count: usize,
        archived_ids: Vec<String>,
        deferred_count: usize,
        deferred_ids: Vec<String>,
        protection_violations: Vec<String>,
        archive_location: String,
    },
    FocusIdentified {
        foci: Vec<super::types::FocusItem>,
        overall_level: f64,
    },
    FocusReset {
        reason: String,
        affected_residuals: Vec<String>,
    },
    ConfigChanged {
        parameter: String,
        previous_value: f64,
        new_value: f64,
    },
}

impl MetacognitiveEvent {
    pub fn event_type_name(&self) -> &'static str {
        match self {
            MetacognitiveEvent::AttentionAdjusted { .. } => "attention.adjusted",
            MetacognitiveEvent::AttentionModeChanged { .. } => "attention.modeChanged",
            MetacognitiveEvent::AttentionAnomalyDetected { .. } => "attention.anomalyDetected",
            MetacognitiveEvent::AttentionAnomalyResolved { .. } => "attention.anomalyResolved",
            MetacognitiveEvent::MemoryValueCalculated { .. } => "memory.valueCalculated",
            MetacognitiveEvent::ForgetTriggered { .. } => "memory.forgetTriggered",
            MetacognitiveEvent::ForgetCompleted { .. } => "memory.forgetCompleted",
            MetacognitiveEvent::FocusIdentified { .. } => "focus.identified",
            MetacognitiveEvent::FocusReset { .. } => "focus.reset",
            MetacognitiveEvent::ConfigChanged { .. } => "controller.configChanged",
        }
    }
}

pub trait MetacognitiveEventPublisher: Send + Sync {
    fn publish(&self, event: MetacognitiveEvent) -> Result<(), String>;
}

pub struct NullMetacognitiveEventPublisher;

impl MetacognitiveEventPublisher for NullMetacognitiveEventPublisher {
    fn publish(&self, _event: MetacognitiveEvent) -> Result<(), String> {
        Ok(())
    }
}
