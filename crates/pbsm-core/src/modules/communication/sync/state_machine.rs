use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::modules::communication::error::CommunicationError;

#[derive(Clone, Debug, PartialEq)]
pub enum SyncStatus {
    Initiated,
    InProgress,
    Completed,
    Failed,
    Partial,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyncDirection {
    Outbound,
    Inbound,
    Bidirectional,
}

#[derive(Clone, Debug)]
pub struct SyncStatusInfo {
    pub sync_id: String,
    pub status: SyncStatus,
    pub direction: SyncDirection,
    pub progress: Option<SyncProgress>,
    pub last_activity: Option<DateTime<Utc>>,
    pub errors: Vec<SyncError>,
}

#[derive(Clone, Debug)]
pub struct SyncProgress {
    pub bytes_sent: u64,
    pub bytes_total: u64,
    pub percentage: f64,
}

#[derive(Clone, Debug)]
pub struct SyncError {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct SyncStateData {
    pub direction: SyncDirection,
    pub progress: Option<SyncProgress>,
    pub last_activity: DateTime<Utc>,
    pub errors: Vec<SyncError>,
}

impl Default for SyncStateData {
    fn default() -> Self {
        Self {
            direction: SyncDirection::Outbound,
            progress: None,
            last_activity: Utc::now(),
            errors: Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct SyncStateMachine {
    states: HashMap<String, (SyncState, SyncStateData)>,
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum SyncState {
    Idle,
    Initiated,
    AwaitingResponse,
    Constructing,
    Transmitting,
    AwaitingVerification,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy)]
pub enum SyncStateTransition {
    RequestInitiated,
    ResponseReceived,
    SnapshotConstructed,
    SnapshotTransmitted,
    VerificationCompleted,
    Completed,
    Failed,
}

impl SyncState {
    pub fn to_status(&self) -> SyncStatus {
        match self {
            SyncState::Idle => SyncStatus::Initiated,
            SyncState::Initiated => SyncStatus::Initiated,
            SyncState::AwaitingResponse => SyncStatus::Initiated,
            SyncState::Constructing => SyncStatus::InProgress,
            SyncState::Transmitting => SyncStatus::InProgress,
            SyncState::AwaitingVerification => SyncStatus::InProgress,
            SyncState::Completed => SyncStatus::Completed,
            SyncState::Failed => SyncStatus::Failed,
        }
    }
}

impl SyncStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn transition(
        &mut self,
        sync_id: &str,
        transition: SyncStateTransition,
    ) -> Result<(), CommunicationError> {
        let current = self
            .states
            .get(sync_id)
            .map(|(s, _)| *s)
            .unwrap_or(SyncState::Idle);

        let next = match (current, transition) {
            (SyncState::Idle, SyncStateTransition::RequestInitiated) => SyncState::Initiated,
            (SyncState::Initiated, SyncStateTransition::ResponseReceived) => {
                SyncState::AwaitingResponse
            }
            (SyncState::AwaitingResponse, SyncStateTransition::SnapshotConstructed) => {
                SyncState::Constructing
            }
            (SyncState::Constructing, SyncStateTransition::SnapshotTransmitted) => {
                SyncState::Transmitting
            }
            (SyncState::Transmitting, SyncStateTransition::VerificationCompleted) => {
                SyncState::AwaitingVerification
            }
            (SyncState::AwaitingVerification, SyncStateTransition::Completed) => {
                SyncState::Completed
            }
            (_, SyncStateTransition::Failed) => SyncState::Failed,
            _ => {
                return Err(CommunicationError::InternalError {
                    context: format!(
                        "Invalid transition from {:?} with {:?}",
                        current, transition
                    ),
                })
            }
        };

        if let Some(entry) = self.states.get_mut(sync_id) {
            entry.0 = next;
            entry.1.last_activity = Utc::now();
        } else {
            self.states
                .insert(sync_id.to_string(), (next, SyncStateData::default()));
        }
        Ok(())
    }

    pub fn get_status(&self, sync_id: &str) -> Option<SyncStatusInfo> {
        self.states.get(sync_id).map(|(state, data)| SyncStatusInfo {
            sync_id: sync_id.to_string(),
            status: state.to_status(),
            direction: data.direction.clone(),
            progress: data.progress.clone(),
            last_activity: Some(data.last_activity),
            errors: data.errors.clone(),
        })
    }

    pub fn update_progress(&mut self, sync_id: &str, progress: SyncProgress) {
        if let Some(entry) = self.states.get_mut(sync_id) {
            entry.1.progress = Some(progress);
            entry.1.last_activity = Utc::now();
        }
    }

    pub fn record_error(&mut self, sync_id: &str, error: SyncError) {
        if let Some(entry) = self.states.get_mut(sync_id) {
            entry.1.errors.push(error);
            entry.1.last_activity = Utc::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        let mut sm = SyncStateMachine::new();
        let sync_id = "test-sync-1";

        sm.transition(sync_id, SyncStateTransition::RequestInitiated)
            .unwrap();
        assert_eq!(sm.states.get(sync_id).map(|(s, _)| s), Some(&SyncState::Initiated));

        sm.transition(sync_id, SyncStateTransition::ResponseReceived)
            .unwrap();
        assert_eq!(sm.states.get(sync_id).map(|(s, _)| s), Some(&SyncState::AwaitingResponse));

        sm.transition(sync_id, SyncStateTransition::SnapshotConstructed)
            .unwrap();
        assert_eq!(sm.states.get(sync_id).map(|(s, _)| s), Some(&SyncState::Constructing));

        sm.transition(sync_id, SyncStateTransition::SnapshotTransmitted)
            .unwrap();
        assert_eq!(sm.states.get(sync_id).map(|(s, _)| s), Some(&SyncState::Transmitting));

        sm.transition(sync_id, SyncStateTransition::VerificationCompleted)
            .unwrap();
        assert_eq!(
            sm.states.get(sync_id).map(|(s, _)| s),
            Some(&SyncState::AwaitingVerification)
        );

        sm.transition(sync_id, SyncStateTransition::Completed)
            .unwrap();
        assert_eq!(sm.states.get(sync_id).map(|(s, _)| s), Some(&SyncState::Completed));
    }

    #[test]
    fn test_invalid_transitions() {
        let mut sm = SyncStateMachine::new();
        let sync_id = "test-sync-2";

        let result = sm.transition(sync_id, SyncStateTransition::ResponseReceived);
        assert!(result.is_err());

        sm.transition(sync_id, SyncStateTransition::RequestInitiated)
            .unwrap();
        let result = sm.transition(sync_id, SyncStateTransition::SnapshotConstructed);
        assert!(result.is_err());

        let result = sm.transition(sync_id, SyncStateTransition::Completed);
        assert!(result.is_err());
    }

    #[test]
    fn test_failed_from_any_state() {
        let mut sm = SyncStateMachine::new();
        let sync_id = "test-sync-3";

        sm.transition(sync_id, SyncStateTransition::RequestInitiated)
            .unwrap();
        sm.transition(sync_id, SyncStateTransition::Failed).unwrap();
        assert_eq!(sm.states.get(sync_id).map(|(s, _)| s), Some(&SyncState::Failed));

        let sync_id2 = "test-sync-4";
        sm.transition(sync_id2, SyncStateTransition::Failed)
            .unwrap();
        assert_eq!(sm.states.get(sync_id2).map(|(s, _)| s), Some(&SyncState::Failed));
    }

    #[test]
    fn test_state_to_status_mapping() {
        assert_eq!(SyncState::Idle.to_status(), SyncStatus::Initiated);
        assert_eq!(SyncState::Initiated.to_status(), SyncStatus::Initiated);
        assert_eq!(
            SyncState::AwaitingResponse.to_status(),
            SyncStatus::Initiated
        );
        assert_eq!(SyncState::Constructing.to_status(), SyncStatus::InProgress);
        assert_eq!(SyncState::Transmitting.to_status(), SyncStatus::InProgress);
        assert_eq!(
            SyncState::AwaitingVerification.to_status(),
            SyncStatus::InProgress
        );
        assert_eq!(SyncState::Completed.to_status(), SyncStatus::Completed);
        assert_eq!(SyncState::Failed.to_status(), SyncStatus::Failed);
    }

    #[test]
    fn test_get_status_unknown_sync() {
        let sm = SyncStateMachine::new();
        assert!(sm.get_status("nonexistent").is_none());
    }

    #[test]
    fn test_get_status_returns_correct_info() {
        let mut sm = SyncStateMachine::new();
        let sync_id = "test-sync-5";

        sm.transition(sync_id, SyncStateTransition::RequestInitiated)
            .unwrap();

        let info = sm.get_status(sync_id).unwrap();
        assert_eq!(info.sync_id, sync_id);
        assert_eq!(info.status, SyncStatus::Initiated);
        assert_eq!(info.direction, SyncDirection::Outbound);
        assert!(info.progress.is_none());
        assert!(info.errors.is_empty());
    }
}
