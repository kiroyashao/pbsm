use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::*;

use super::state_machine::*;

#[derive(Clone, Debug)]
pub enum SyncEvent {
    SyncRequestInitiated {
        sync_id: String,
        source_agent: String,
        target_agent: String,
        scope: CommSnapshotScope,
        priority: Priority,
    },
    SyncSnapshotConstructed {
        sync_id: String,
        snapshot_id: String,
        node_count: usize,
        relation_count: usize,
        construction_time_ms: u64,
        compression_ratio: Option<f64>,
    },
    SyncSnapshotTransmitted {
        sync_id: String,
        snapshot_id: String,
        transmission_time_ms: u64,
        bytes_transmitted: u64,
    },
    SyncSnapshotVerified {
        sync_id: String,
        snapshot_id: String,
        verification_passed: bool,
        verification_time_ms: u64,
    },
    SyncCompleted {
        sync_id: String,
        direction: SyncDirection,
        nodes_synced: usize,
        relations_synced: usize,
        conflicts_detected: usize,
        conflicts_resolved: usize,
        total_time_ms: u64,
    },
    SyncFailed {
        sync_id: String,
        failure_stage: SyncFailureStage,
        error_code: String,
        error_message: String,
        retryable: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyncFailureStage {
    Construction,
    Transmission,
    Verification,
    Fusion,
}

#[derive(Clone, Debug)]
pub struct SyncRequest {
    pub request_id: String,
    pub request_type: SyncRequestType,
    pub source_agent: SourceAgentInfo,
    pub target_agent: String,
    pub scope: CommSnapshotScope,
    pub priority: Priority,
    pub preference: SyncPreference,
    pub correlation_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub ttl: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyncRequestType {
    SyncRequest,
    PushRequest,
    NegotiationRequest,
}

#[derive(Clone, Debug)]
pub struct SourceAgentInfo {
    pub agent_id: String,
    pub session_id: String,
}

#[derive(Clone, Debug)]
pub struct SyncPreference {
    pub compression: bool,
    pub encrypted: bool,
    pub max_snapshot_size: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct SyncRequestResult {
    pub sync_id: String,
    pub request: SyncRequest,
    pub status: SyncStatus,
}

pub struct SyncManager {
    state_machine: RwLock<SyncStateMachine>,
    event_bus: broadcast::Sender<SyncEvent>,
    agent_id: String,
    session_id: String,
}

impl SyncManager {
    pub fn new(agent_id: String, session_id: String) -> Self {
        let (event_bus, _) = broadcast::channel(100);
        Self {
            state_machine: RwLock::new(SyncStateMachine::new()),
            event_bus,
            agent_id,
            session_id,
        }
    }

    pub fn request_sync(
        &self,
        target_agent: &str,
        scope: CommSnapshotScope,
        priority: Option<Priority>,
    ) -> Result<SyncRequestResult, CommunicationError> {
        let sync_id = Uuid::new_v4().to_string();
        let priority = priority.unwrap_or(Priority::Normal);

        let request = SyncRequest {
            request_id: Uuid::new_v4().to_string(),
            request_type: SyncRequestType::SyncRequest,
            source_agent: SourceAgentInfo {
                agent_id: self.agent_id.clone(),
                session_id: self.session_id.clone(),
            },
            target_agent: target_agent.to_string(),
            scope: scope.clone(),
            priority,
            preference: SyncPreference {
                compression: true,
                encrypted: false,
                max_snapshot_size: None,
            },
            correlation_id: None,
            timestamp: Utc::now(),
            ttl: None,
        };

        self.state_machine
            .write()
            .transition(&sync_id, SyncStateTransition::RequestInitiated)?;

        let _ = self.event_bus.send(SyncEvent::SyncRequestInitiated {
            sync_id: sync_id.clone(),
            source_agent: self.agent_id.clone(),
            target_agent: target_agent.to_string(),
            scope,
            priority,
        });

        Ok(SyncRequestResult {
            sync_id,
            status: SyncStatus::Initiated,
            request,
        })
    }

    pub fn push_sync(
        &self,
        target_agent: &str,
        scope: CommSnapshotScope,
    ) -> Result<SyncRequestResult, CommunicationError> {
        let sync_id = Uuid::new_v4().to_string();

        let request = SyncRequest {
            request_id: Uuid::new_v4().to_string(),
            request_type: SyncRequestType::PushRequest,
            source_agent: SourceAgentInfo {
                agent_id: self.agent_id.clone(),
                session_id: self.session_id.clone(),
            },
            target_agent: target_agent.to_string(),
            scope: scope.clone(),
            priority: Priority::Normal,
            preference: SyncPreference {
                compression: true,
                encrypted: false,
                max_snapshot_size: None,
            },
            correlation_id: None,
            timestamp: Utc::now(),
            ttl: None,
        };

        self.state_machine
            .write()
            .transition(&sync_id, SyncStateTransition::RequestInitiated)?;

        let _ = self.event_bus.send(SyncEvent::SyncRequestInitiated {
            sync_id: sync_id.clone(),
            source_agent: self.agent_id.clone(),
            target_agent: target_agent.to_string(),
            scope,
            priority: Priority::Normal,
        });

        Ok(SyncRequestResult {
            sync_id,
            status: SyncStatus::Initiated,
            request,
        })
    }

    pub fn get_sync_status(&self, sync_id: &str) -> Result<SyncStatusInfo, CommunicationError> {
        self.state_machine
            .read()
            .get_status(sync_id)
            .ok_or_else(|| CommunicationError::InternalError {
                context: format!("Sync {} not found", sync_id),
            })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
        self.event_bus.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_sync_creates_correct_state() {
        let manager = SyncManager::new("agent-1".to_string(), "session-1".to_string());
        let scope = CommSnapshotScope::default();
        let result = manager
            .request_sync("agent-2", scope.clone(), Some(Priority::High))
            .unwrap();

        assert!(!result.sync_id.is_empty());
        assert_eq!(result.status, SyncStatus::Initiated);
        assert_eq!(result.request.request_type, SyncRequestType::SyncRequest);
        assert_eq!(result.request.target_agent, "agent-2");
        assert_eq!(result.request.priority, Priority::High);

        let status = manager.get_sync_status(&result.sync_id).unwrap();
        assert_eq!(status.status, SyncStatus::Initiated);
    }

    #[test]
    fn test_push_sync() {
        let manager = SyncManager::new("agent-1".to_string(), "session-1".to_string());
        let scope = CommSnapshotScope::default();
        let result = manager.push_sync("agent-3", scope).unwrap();

        assert!(!result.sync_id.is_empty());
        assert_eq!(result.status, SyncStatus::Initiated);
        assert_eq!(result.request.request_type, SyncRequestType::PushRequest);
        assert_eq!(result.request.target_agent, "agent-3");
        assert_eq!(result.request.priority, Priority::Normal);
    }

    #[test]
    fn test_get_sync_status_not_found() {
        let manager = SyncManager::new("agent-1".to_string(), "session-1".to_string());
        let result = manager.get_sync_status("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_subscribe_receives_events() {
        let manager = SyncManager::new("agent-1".to_string(), "session-1".to_string());
        let mut receiver = manager.subscribe();

        let scope = CommSnapshotScope::default();
        let result = manager.request_sync("agent-2", scope, None).unwrap();

        let event = receiver.try_recv().unwrap();
        match event {
            SyncEvent::SyncRequestInitiated {
                sync_id,
                source_agent,
                target_agent,
                ..
            } => {
                assert_eq!(sync_id, result.sync_id);
                assert_eq!(source_agent, "agent-1");
                assert_eq!(target_agent, "agent-2");
            }
            _ => panic!("Expected SyncRequestInitiated event"),
        }
    }
}
