use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::*;

#[derive(Clone, Debug, PartialEq)]
pub enum DelegationState {
    Pending,
    InProgress,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub enum DelegationType {
    Task,
    Query,
    Monitoring,
}

#[derive(Clone, Debug)]
pub struct TaskSpecification {
    pub task_id: String,
    pub task_type: DelegationType,
    pub description: String,
    pub parameters: HashMap<String, ParameterValue>,
    pub required_capabilities: Vec<String>,
    pub quality_criteria: Vec<QualityCriterion>,
    pub deadline: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub enum ParameterValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Json(serde_json::Value),
}

#[derive(Clone, Debug)]
pub struct QualityCriterion {
    pub criterion: String,
    pub threshold: f64,
    pub weight: f64,
}

#[derive(Clone, Debug)]
pub struct DelegationOptions {
    pub delegatee: Option<String>,
    pub fallback_strategy: Option<FallbackStrategy>,
    pub max_retries: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub priority: Option<Priority>,
}

#[derive(Clone, Debug)]
pub struct DelegationResult {
    pub delegation_id: String,
    pub state: DelegationState,
    pub delegatee: String,
}

#[derive(Clone, Debug)]
pub struct DelegationStatus {
    pub delegation_id: String,
    pub task: TaskSpecification,
    pub state: DelegationState,
    pub delegatee: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub progress: Option<DelegationProgress>,
    pub result: Option<FinalResult>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum StateTransition {
    Initiate,
    Start,
    Pause,
    Resume,
    Complete,
    Fail,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct DelegationProgress {
    pub percentage: f64,
    pub stage: String,
    pub resource_usage: ResourceUsage,
}

#[derive(Clone, Debug)]
pub struct ResourceUsage {
    pub cpu_time_ms: u64,
    pub memory_bytes: u64,
    pub api_calls: u32,
}

#[derive(Clone, Debug)]
pub struct IntermediateResult {
    pub result_id: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct FinalResult {
    pub data: serde_json::Value,
    pub quality_score: Option<f64>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct CancellationResult {
    pub delegation_id: String,
    pub cancelled: bool,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub enum DelegationEvent {
    DelegationInitiated {
        delegation_id: String,
        delegatee: String,
    },
    DelegationProgress {
        delegation_id: String,
        progress: DelegationProgress,
    },
    DelegationCompleted {
        delegation_id: String,
        result: FinalResult,
    },
    DelegationFailed {
        delegation_id: String,
        error: String,
    },
    DelegationCancelled {
        delegation_id: String,
        reason: String,
    },
    DelegationPaused {
        delegation_id: String,
    },
    DelegationResumed {
        delegation_id: String,
    },
}

pub struct DelegationManager {
    delegations: RwLock<HashMap<String, DelegationStatus>>,
    event_bus: broadcast::Sender<DelegationEvent>,
}

impl Default for DelegationManager {
    fn default() -> Self {
        let (event_bus, _) = broadcast::channel(100);
        Self {
            delegations: RwLock::new(HashMap::new()),
            event_bus,
        }
    }
}

impl DelegationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn delegate(
        &self,
        task: TaskSpecification,
        delegatee: Option<&str>,
        options: Option<DelegationOptions>,
    ) -> Result<DelegationResult, CommunicationError> {
        let delegation_id = Uuid::new_v4().to_string();
        let delegatee_id = delegatee
            .map(|s| s.to_string())
            .or_else(|| options.as_ref().and_then(|o| o.delegatee.clone()))
            .unwrap_or_else(|| "default-delegatee".to_string());

        let now = Utc::now();
        let status = DelegationStatus {
            delegation_id: delegation_id.clone(),
            task: task.clone(),
            state: DelegationState::Pending,
            delegatee: Some(delegatee_id.clone()),
            created_at: now,
            updated_at: now,
            progress: None,
            result: None,
            errors: Vec::new(),
        };

        self.delegations
            .write()
            .insert(delegation_id.clone(), status);

        let _ = self.event_bus.send(DelegationEvent::DelegationInitiated {
            delegation_id: delegation_id.clone(),
            delegatee: delegatee_id.clone(),
        });

        Ok(DelegationResult {
            delegation_id,
            state: DelegationState::Pending,
            delegatee: delegatee_id,
        })
    }

    pub fn delegate_with_matching(
        &self,
        task: TaskSpecification,
        required_capabilities: Vec<String>,
    ) -> Result<DelegationResult, CommunicationError> {
        let _ = required_capabilities;
        self.delegate(task, None, None)
    }

    pub fn get_delegation_status(
        &self,
        delegation_id: &str,
    ) -> Result<DelegationStatus, CommunicationError> {
        self.delegations
            .read()
            .get(delegation_id)
            .cloned()
            .ok_or_else(|| {
                CommunicationError::DelegationFailed(format!(
                    "Delegation {} not found",
                    delegation_id
                ))
            })
    }

    pub fn get_delegation_history(
        &self,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<DelegationStatus>, CommunicationError> {
        let delegations = self.delegations.read();
        let mut results: Vec<DelegationStatus> = delegations
            .values()
            .filter(|d| {
                agent_id
                    .map(|aid| d.delegatee.as_deref() == Some(aid))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        results.sort_by_key(|d| std::cmp::Reverse(d.updated_at));
        results.truncate(limit);
        Ok(results)
    }

    pub fn cancel_delegation(
        &self,
        delegation_id: &str,
        reason: &str,
    ) -> Result<CancellationResult, CommunicationError> {
        let mut delegations = self.delegations.write();
        let status = delegations.get_mut(delegation_id).ok_or_else(|| {
            CommunicationError::DelegationFailed(format!("Delegation {} not found", delegation_id))
        })?;

        if status.state == DelegationState::Completed || status.state == DelegationState::Cancelled
        {
            return Ok(CancellationResult {
                delegation_id: delegation_id.to_string(),
                cancelled: false,
                reason: format!("Cannot cancel delegation in state {:?}", status.state),
            });
        }

        status.state = DelegationState::Cancelled;
        status.updated_at = Utc::now();
        status.errors.push(format!("Cancelled: {}", reason));

        let _ = self.event_bus.send(DelegationEvent::DelegationCancelled {
            delegation_id: delegation_id.to_string(),
            reason: reason.to_string(),
        });

        Ok(CancellationResult {
            delegation_id: delegation_id.to_string(),
            cancelled: true,
            reason: reason.to_string(),
        })
    }

    pub fn pause_delegation(
        &self,
        delegation_id: &str,
    ) -> Result<DelegationStatus, CommunicationError> {
        let mut delegations = self.delegations.write();
        let status = delegations.get_mut(delegation_id).ok_or_else(|| {
            CommunicationError::DelegationFailed(format!("Delegation {} not found", delegation_id))
        })?;

        if status.state != DelegationState::InProgress {
            return Err(CommunicationError::DelegationFailed(format!(
                "Cannot pause delegation in state {:?}",
                status.state
            )));
        }

        status.state = DelegationState::Paused;
        status.updated_at = Utc::now();

        let _ = self.event_bus.send(DelegationEvent::DelegationPaused {
            delegation_id: delegation_id.to_string(),
        });

        Ok(status.clone())
    }

    pub fn resume_delegation(
        &self,
        delegation_id: &str,
    ) -> Result<DelegationStatus, CommunicationError> {
        let mut delegations = self.delegations.write();
        let status = delegations.get_mut(delegation_id).ok_or_else(|| {
            CommunicationError::DelegationFailed(format!("Delegation {} not found", delegation_id))
        })?;

        if status.state != DelegationState::Paused {
            return Err(CommunicationError::DelegationFailed(format!(
                "Cannot resume delegation in state {:?}",
                status.state
            )));
        }

        status.state = DelegationState::InProgress;
        status.updated_at = Utc::now();

        let _ = self.event_bus.send(DelegationEvent::DelegationResumed {
            delegation_id: delegation_id.to_string(),
        });

        Ok(status.clone())
    }

    pub fn subscribe_to_delegations(&self) -> broadcast::Receiver<DelegationEvent> {
        self.event_bus.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_task() -> TaskSpecification {
        TaskSpecification {
            task_id: "task-1".to_string(),
            task_type: DelegationType::Task,
            description: "Test task".to_string(),
            parameters: HashMap::new(),
            required_capabilities: vec!["compute".to_string()],
            quality_criteria: Vec::new(),
            deadline: None,
        }
    }

    #[test]
    fn test_delegate() {
        let manager = DelegationManager::new();
        let task = make_test_task();
        let result = manager.delegate(task, Some("agent-2"), None).unwrap();

        assert!(!result.delegation_id.is_empty());
        assert_eq!(result.state, DelegationState::Pending);
        assert_eq!(result.delegatee, "agent-2");

        let status = manager
            .get_delegation_status(&result.delegation_id)
            .unwrap();
        assert_eq!(status.state, DelegationState::Pending);
        assert_eq!(status.delegatee, Some("agent-2".to_string()));
    }

    #[test]
    fn test_get_status_not_found() {
        let manager = DelegationManager::new();
        let result = manager.get_delegation_status("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_delegation() {
        let manager = DelegationManager::new();
        let task = make_test_task();
        let result = manager.delegate(task, Some("agent-2"), None).unwrap();

        let cancel_result = manager
            .cancel_delegation(&result.delegation_id, "no longer needed")
            .unwrap();

        assert!(cancel_result.cancelled);
        assert_eq!(cancel_result.reason, "no longer needed");

        let status = manager
            .get_delegation_status(&result.delegation_id)
            .unwrap();
        assert_eq!(status.state, DelegationState::Cancelled);
    }

    #[test]
    fn test_pause_resume() {
        let manager = DelegationManager::new();
        let task = make_test_task();
        let result = manager.delegate(task, Some("agent-2"), None).unwrap();

        let mut delegations = manager.delegations.write();
        let status = delegations.get_mut(&result.delegation_id).unwrap();
        status.state = DelegationState::InProgress;
        drop(delegations);

        let paused = manager.pause_delegation(&result.delegation_id).unwrap();
        assert_eq!(paused.state, DelegationState::Paused);

        let resumed = manager.resume_delegation(&result.delegation_id).unwrap();
        assert_eq!(resumed.state, DelegationState::InProgress);
    }

    #[test]
    fn test_pause_not_in_progress() {
        let manager = DelegationManager::new();
        let task = make_test_task();
        let result = manager.delegate(task, Some("agent-2"), None).unwrap();

        let pause_result = manager.pause_delegation(&result.delegation_id);
        assert!(pause_result.is_err());
    }

    #[test]
    fn test_delegation_history() {
        let manager = DelegationManager::new();

        for i in 0..3 {
            let task = TaskSpecification {
                task_id: format!("task-{}", i),
                task_type: DelegationType::Task,
                description: format!("Task {}", i),
                parameters: HashMap::new(),
                required_capabilities: Vec::new(),
                quality_criteria: Vec::new(),
                deadline: None,
            };
            manager.delegate(task, Some("agent-2"), None).unwrap();
        }

        let task = TaskSpecification {
            task_id: "task-other".to_string(),
            task_type: DelegationType::Task,
            description: "Other task".to_string(),
            parameters: HashMap::new(),
            required_capabilities: Vec::new(),
            quality_criteria: Vec::new(),
            deadline: None,
        };
        manager.delegate(task, Some("agent-3"), None).unwrap();

        let history = manager.get_delegation_history(Some("agent-2"), 10).unwrap();
        assert_eq!(history.len(), 3);

        let all_history = manager.get_delegation_history(None, 10).unwrap();
        assert_eq!(all_history.len(), 4);

        let limited = manager.get_delegation_history(None, 2).unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_delegate_with_matching() {
        let manager = DelegationManager::new();
        let task = make_test_task();
        let result = manager
            .delegate_with_matching(task, vec!["compute".to_string()])
            .unwrap();

        assert!(!result.delegation_id.is_empty());
        assert_eq!(result.state, DelegationState::Pending);
    }

    #[test]
    fn test_subscribe_receives_events() {
        let manager = DelegationManager::new();
        let mut receiver = manager.subscribe_to_delegations();

        let task = make_test_task();
        let result = manager.delegate(task, Some("agent-2"), None).unwrap();

        let event = receiver.try_recv().unwrap();
        match event {
            DelegationEvent::DelegationInitiated {
                delegation_id,
                delegatee,
            } => {
                assert_eq!(delegation_id, result.delegation_id);
                assert_eq!(delegatee, "agent-2");
            }
            _ => panic!("Expected DelegationInitiated event"),
        }
    }
}
