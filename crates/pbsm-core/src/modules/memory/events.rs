//! M4 外部记忆存储 - 事件模块
//!
//! 定义记忆系统的事件类型、事件发布接口及辅助事件构造函数。
//! 事件编码遵循 HLD-M4 Section 8 规范。

use std::fmt;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for EventSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventSeverity::Info => write!(f, "info"),
            EventSeverity::Warning => write!(f, "warning"),
            EventSeverity::Error => write!(f, "error"),
            EventSeverity::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvent {
    pub event_id: String,
    pub event_code: String,
    pub event_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_module: String,
    pub correlation_id: Option<String>,
    pub event_data: serde_json::Value,
    pub severity: EventSeverity,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Error)]
pub enum EventPublishError {
    #[error("事件发布失败: {0}")]
    PublishFailed(String),
    #[error("未知事件类型")]
    EventTypeUnknown,
}

pub trait MemoryEventPublisher: Send + Sync {
    fn publish(&self, event: MemoryEvent) -> Result<(), EventPublishError>;
}

pub struct NullMemoryEventPublisher;

impl MemoryEventPublisher for NullMemoryEventPublisher {
    fn publish(&self, _event: MemoryEvent) -> Result<(), EventPublishError> {
        Ok(())
    }
}

fn new_event(
    event_code: &str,
    event_type: &str,
    severity: EventSeverity,
    event_data: serde_json::Value,
) -> MemoryEvent {
    MemoryEvent {
        event_id: Uuid::new_v4().to_string(),
        event_code: event_code.to_string(),
        event_type: event_type.to_string(),
        timestamp: Utc::now(),
        source_module: "M4".to_string(),
        correlation_id: None,
        event_data,
        severity,
        metadata: None,
    }
}

pub fn create_snapshot_created_event(
    snapshot_id: &str,
    snapshot_type: &str,
    size_bytes: u64,
) -> MemoryEvent {
    new_event(
        "MEM_EVT_001",
        "memory.snapshotCreated",
        EventSeverity::Info,
        serde_json::json!({
            "snapshot_id": snapshot_id,
            "snapshot_type": snapshot_type,
            "size_bytes": size_bytes,
        }),
    )
}

pub fn create_snapshot_restored_event(snapshot_id: &str, duration_ms: u64) -> MemoryEvent {
    new_event(
        "MEM_EVT_002",
        "memory.snapshotRestored",
        EventSeverity::Info,
        serde_json::json!({
            "snapshot_id": snapshot_id,
            "duration_ms": duration_ms,
        }),
    )
}

pub fn create_retrieval_completed_event(
    query_type: &str,
    result_count: usize,
    duration_ms: u64,
    cache_hit: bool,
) -> MemoryEvent {
    new_event(
        "MEM_EVT_003",
        "memory.retrievalCompleted",
        EventSeverity::Info,
        serde_json::json!({
            "query_type": query_type,
            "result_count": result_count,
            "duration_ms": duration_ms,
            "cache_hit": cache_hit,
        }),
    )
}

pub fn create_cleanup_completed_event(
    cleanup_type: &str,
    deleted: usize,
    archived: usize,
    freed_bytes: u64,
) -> MemoryEvent {
    new_event(
        "MEM_EVT_004",
        "memory.cleanupCompleted",
        EventSeverity::Info,
        serde_json::json!({
            "cleanup_type": cleanup_type,
            "deleted": deleted,
            "archived": archived,
            "freed_bytes": freed_bytes,
        }),
    )
}

pub fn create_storage_warning_event(
    usage_pct: f64,
    threshold_pct: f64,
    layer: &str,
) -> MemoryEvent {
    new_event(
        "MEM_EVT_005",
        "memory.storageWarning",
        EventSeverity::Warning,
        serde_json::json!({
            "usage_pct": usage_pct,
            "threshold_pct": threshold_pct,
            "layer": layer,
        }),
    )
}

pub fn create_experience_created_event(
    experience_id: &str,
    pattern_type: &str,
    confidence: f64,
) -> MemoryEvent {
    new_event(
        "MEM_EVT_006",
        "memory.experienceCreated",
        EventSeverity::Info,
        serde_json::json!({
            "experience_id": experience_id,
            "pattern_type": pattern_type,
            "confidence": confidence,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_severity_display() {
        assert_eq!(EventSeverity::Info.to_string(), "info");
        assert_eq!(EventSeverity::Warning.to_string(), "warning");
        assert_eq!(EventSeverity::Error.to_string(), "error");
        assert_eq!(EventSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn test_snapshot_created_event() {
        let event = create_snapshot_created_event("snap-001", "full", 4096);
        assert_eq!(event.event_code, "MEM_EVT_001");
        assert_eq!(event.event_type, "memory.snapshotCreated");
        assert_eq!(event.source_module, "M4");
        assert_eq!(event.severity, EventSeverity::Info);
        assert!(event.correlation_id.is_none());
        assert!(event.metadata.is_none());
        assert!(!event.event_id.is_empty());

        let data = &event.event_data;
        assert_eq!(data["snapshot_id"], "snap-001");
        assert_eq!(data["snapshot_type"], "full");
        assert_eq!(data["size_bytes"], 4096);
    }

    #[test]
    fn test_snapshot_restored_event() {
        let event = create_snapshot_restored_event("snap-002", 150);
        assert_eq!(event.event_code, "MEM_EVT_002");
        assert_eq!(event.event_type, "memory.snapshotRestored");
        assert_eq!(event.severity, EventSeverity::Info);

        let data = &event.event_data;
        assert_eq!(data["snapshot_id"], "snap-002");
        assert_eq!(data["duration_ms"], 150);
    }

    #[test]
    fn test_retrieval_completed_event() {
        let event = create_retrieval_completed_event("semantic", 5, 42, true);
        assert_eq!(event.event_code, "MEM_EVT_003");
        assert_eq!(event.event_type, "memory.retrievalCompleted");

        let data = &event.event_data;
        assert_eq!(data["query_type"], "semantic");
        assert_eq!(data["result_count"], 5);
        assert_eq!(data["duration_ms"], 42);
        assert_eq!(data["cache_hit"], true);
    }

    #[test]
    fn test_cleanup_completed_event() {
        let event = create_cleanup_completed_event("ttl", 10, 3, 8192);
        assert_eq!(event.event_code, "MEM_EVT_004");
        assert_eq!(event.event_type, "memory.cleanupCompleted");

        let data = &event.event_data;
        assert_eq!(data["cleanup_type"], "ttl");
        assert_eq!(data["deleted"], 10);
        assert_eq!(data["archived"], 3);
        assert_eq!(data["freed_bytes"], 8192);
    }

    #[test]
    fn test_storage_warning_event() {
        let event = create_storage_warning_event(85.5, 80.0, "hot");
        assert_eq!(event.event_code, "MEM_EVT_005");
        assert_eq!(event.event_type, "memory.storageWarning");
        assert_eq!(event.severity, EventSeverity::Warning);

        let data = &event.event_data;
        assert!((data["usage_pct"].as_f64().unwrap() - 85.5).abs() < f64::EPSILON);
        assert!((data["threshold_pct"].as_f64().unwrap() - 80.0).abs() < f64::EPSILON);
        assert_eq!(data["layer"], "hot");
    }

    #[test]
    fn test_experience_created_event() {
        let event = create_experience_created_event("exp-001", "causal", 0.92);
        assert_eq!(event.event_code, "MEM_EVT_006");
        assert_eq!(event.event_type, "memory.experienceCreated");

        let data = &event.event_data;
        assert_eq!(data["experience_id"], "exp-001");
        assert_eq!(data["pattern_type"], "causal");
        assert!((data["confidence"].as_f64().unwrap() - 0.92).abs() < f64::EPSILON);
    }

    #[test]
    fn test_null_publisher_discards_events() {
        let publisher = NullMemoryEventPublisher;
        let event = create_snapshot_created_event("snap-999", "incremental", 1024);
        let result = publisher.publish(event);
        assert!(result.is_ok());
    }

    #[test]
    fn test_event_publish_error_display() {
        let err = EventPublishError::PublishFailed("connection lost".to_string());
        assert!(err.to_string().contains("connection lost"));

        let err = EventPublishError::EventTypeUnknown;
        assert!(err.to_string().contains("未知事件类型"));
    }

    #[test]
    fn test_event_has_unique_ids() {
        let e1 = create_snapshot_created_event("a", "full", 0);
        let e2 = create_snapshot_created_event("a", "full", 0);
        assert_ne!(e1.event_id, e2.event_id);
    }

    #[test]
    fn test_event_timestamp_is_recent() {
        let before = Utc::now();
        let event = create_snapshot_created_event("snap-ts", "full", 0);
        let after = Utc::now();
        assert!(event.timestamp >= before);
        assert!(event.timestamp <= after);
    }
}
