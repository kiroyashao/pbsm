use std::sync::Arc;

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
    DeferredForgetWarning {
        node_id: String,
        residual_association: f64,
        defer_steps: usize,
        max_defer_steps: usize,
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
    AuditAttentionAdjusted {
        node_ids: Vec<String>,
        adjustment_summary: String,
    },
    AuditForgetExecuted {
        forgotten_ids: Vec<String>,
        reason: String,
        operator: String,
    },
    AuditConfigModified {
        parameter: String,
        old_value: String,
        new_value: String,
        modifier: String,
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
            MetacognitiveEvent::DeferredForgetWarning { .. } => "memory.deferredForgetWarning",
            MetacognitiveEvent::FocusIdentified { .. } => "focus.identified",
            MetacognitiveEvent::FocusReset { .. } => "focus.reset",
            MetacognitiveEvent::ConfigChanged { .. } => "controller.configChanged",
            MetacognitiveEvent::AuditAttentionAdjusted { .. } => "controller.audit.attentionAdjusted",
            MetacognitiveEvent::AuditForgetExecuted { .. } => "controller.audit.forgetExecuted",
            MetacognitiveEvent::AuditConfigModified { .. } => "controller.audit.configModified",
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

pub struct LoggingMetacognitiveEventPublisher {
    inner: Arc<dyn MetacognitiveEventPublisher>,
}

impl LoggingMetacognitiveEventPublisher {
    pub fn new(inner: Arc<dyn MetacognitiveEventPublisher>) -> Self {
        Self { inner }
    }
}

impl MetacognitiveEventPublisher for LoggingMetacognitiveEventPublisher {
    fn publish(&self, event: MetacognitiveEvent) -> Result<(), String> {
        if let Err(e) = self.inner.publish(event) {
            eprintln!("WARN metacognitive event publish failed: {}", e);
        }
        Ok(())
    }
}
