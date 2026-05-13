use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::*;

#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub enum AgentRole {
    Coordinator,
    Collaborator,
    Observer,
    Worker,
}

#[derive(Clone, Debug)]
pub struct AccessPermission {
    pub resource_type: ResourceType,
    pub actions: Vec<AccessAction>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResourceType {
    Snapshot,
    Belief,
    Relation,
    Intent,
    Prediction,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AccessAction {
    Read,
    Write,
    Delete,
    Share,
    Delegate,
}

#[derive(Clone, Debug)]
pub struct AuthorizationRequest {
    pub requester: AgentAuthInfo,
    pub resource: ResourceDescriptor,
    pub action: AccessAction,
    pub context: AuthorizationContext,
}

#[derive(Clone, Debug)]
pub struct AgentAuthInfo {
    pub agent_id: String,
    pub role: AgentRole,
    pub session_id: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ResourceDescriptor {
    pub resource_type: ResourceType,
    pub resource_id: Option<String>,
    pub scope: Option<CommSnapshotScope>,
}

#[derive(Clone, Debug)]
pub struct AuthorizationContext {
    pub purpose: Option<SnapshotPurpose>,
    pub delegation_chain: Vec<String>,
    pub time_constraints: Option<TimeConstraints>,
}

#[derive(Clone, Debug)]
pub struct TimeConstraints {
    pub not_before: Option<DateTime<Utc>>,
    pub not_after: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct AuthorizationDecision {
    pub outcome: DecisionOutcome,
    pub conditions: Vec<AuthorizationCondition>,
    pub obligations: Vec<Obligation>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DecisionOutcome {
    Permitted,
    Denied,
    Conditional,
}

#[derive(Clone, Debug)]
pub struct AuthorizationCondition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    GreaterThan,
    LessThan,
}

#[derive(Clone, Debug)]
pub struct Obligation {
    pub obligation_type: ObligationType,
    pub description: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ObligationType {
    Log,
    Audit,
    Notify,
    Encrypt,
    Anonymize,
}

pub struct AccessController {
    role_permissions: HashMap<AgentRole, Vec<AccessPermission>>,
}

impl Default for AccessController {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessController {
    pub fn new() -> Self {
        let mut role_permissions = HashMap::new();

        let all_actions = vec![
            AccessAction::Read,
            AccessAction::Write,
            AccessAction::Delete,
            AccessAction::Share,
            AccessAction::Delegate,
        ];

        let all_resource_types = [
            ResourceType::Snapshot,
            ResourceType::Belief,
            ResourceType::Relation,
            ResourceType::Intent,
            ResourceType::Prediction,
        ];

        let coordinator_perms: Vec<AccessPermission> = all_resource_types
            .iter()
            .map(|rt| AccessPermission {
                resource_type: rt.clone(),
                actions: all_actions.clone(),
            })
            .collect();
        role_permissions.insert(AgentRole::Coordinator, coordinator_perms);

        let collaborator_actions =
            vec![AccessAction::Read, AccessAction::Write, AccessAction::Share];
        let collaborator_perms: Vec<AccessPermission> = all_resource_types
            .iter()
            .map(|rt| AccessPermission {
                resource_type: rt.clone(),
                actions: collaborator_actions.clone(),
            })
            .collect();
        role_permissions.insert(AgentRole::Collaborator, collaborator_perms);

        let observer_actions = vec![AccessAction::Read];
        let observer_perms: Vec<AccessPermission> = all_resource_types
            .iter()
            .map(|rt| AccessPermission {
                resource_type: rt.clone(),
                actions: observer_actions.clone(),
            })
            .collect();
        role_permissions.insert(AgentRole::Observer, observer_perms);

        let worker_actions = vec![AccessAction::Read, AccessAction::Write];
        let worker_perms: Vec<AccessPermission> = all_resource_types
            .iter()
            .map(|rt| AccessPermission {
                resource_type: rt.clone(),
                actions: worker_actions.clone(),
            })
            .collect();
        role_permissions.insert(AgentRole::Worker, worker_perms);

        Self { role_permissions }
    }

    pub fn check_authorization(
        &self,
        request: AuthorizationRequest,
    ) -> Result<AuthorizationDecision, CommunicationError> {
        let permissions = self
            .role_permissions
            .get(&request.requester.role)
            .ok_or_else(|| {
                CommunicationError::AccessDenied(format!(
                    "Unknown role: {:?}",
                    request.requester.role
                ))
            })?;

        let resource_perm = permissions
            .iter()
            .find(|p| p.resource_type == request.resource.resource_type);

        match resource_perm {
            Some(perm) if perm.actions.contains(&request.action) => {
                if request.requester.role == AgentRole::Worker {
                    Ok(AuthorizationDecision {
                        outcome: DecisionOutcome::Conditional,
                        conditions: vec![AuthorizationCondition {
                            field: "delegated_task".to_string(),
                            operator: ConditionOperator::Equals,
                            value: serde_json::json!(true),
                        }],
                        obligations: vec![Obligation {
                            obligation_type: ObligationType::Audit,
                            description: "Worker access must be for delegated tasks only"
                                .to_string(),
                            parameters: HashMap::new(),
                        }],
                        reason: Some("Worker access permitted for delegated tasks".to_string()),
                    })
                } else {
                    Ok(AuthorizationDecision {
                        outcome: DecisionOutcome::Permitted,
                        conditions: Vec::new(),
                        obligations: Vec::new(),
                        reason: None,
                    })
                }
            }
            _ => Ok(AuthorizationDecision {
                outcome: DecisionOutcome::Denied,
                conditions: Vec::new(),
                obligations: Vec::new(),
                reason: Some(format!(
                    "Role {:?} does not have {:?} access to {:?}",
                    request.requester.role, request.action, request.resource.resource_type
                )),
            }),
        }
    }

    pub fn authorize_snapshot_access(
        &self,
        requester: &AgentAuthInfo,
        resource_type: ResourceType,
        scope: &CommSnapshotScope,
        action: AccessAction,
    ) -> Result<AuthorizationDecision, CommunicationError> {
        let mut decision = self.check_authorization(AuthorizationRequest {
            requester: requester.clone(),
            resource: ResourceDescriptor {
                resource_type,
                resource_id: None,
                scope: Some(scope.clone()),
            },
            action,
            context: AuthorizationContext {
                purpose: None,
                delegation_chain: Vec::new(),
                time_constraints: None,
            },
        })?;

        let is_restrictive = !scope.entity_types.is_empty() || !scope.topics.is_empty();

        if is_restrictive {
            if !scope.entity_types.is_empty() {
                decision.conditions.push(AuthorizationCondition {
                    field: "entity_type".to_string(),
                    operator: ConditionOperator::Contains,
                    value: serde_json::json!(scope
                        .entity_types
                        .iter()
                        .map(|t| format!("{:?}", t))
                        .collect::<Vec<_>>()),
                });
            }
            if !scope.topics.is_empty() {
                decision.conditions.push(AuthorizationCondition {
                    field: "topic".to_string(),
                    operator: ConditionOperator::Contains,
                    value: serde_json::json!(scope.topics),
                });
            }

            if requester.role == AgentRole::Observer && decision.outcome == DecisionOutcome::Permitted
            {
                decision.outcome = DecisionOutcome::Conditional;
                decision.obligations.push(Obligation {
                    obligation_type: ObligationType::Audit,
                    description: "Observer access restricted to scoped resources".to_string(),
                    parameters: HashMap::new(),
                });
            }
        }

        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_auth_info(role: AgentRole) -> AgentAuthInfo {
        AgentAuthInfo {
            agent_id: "agent-1".to_string(),
            role,
            session_id: Some("session-1".to_string()),
            capabilities: Vec::new(),
        }
    }

    #[test]
    fn test_coordinator_has_full_access() {
        let controller = AccessController::new();
        let requester = make_auth_info(AgentRole::Coordinator);

        for action in &[
            AccessAction::Read,
            AccessAction::Write,
            AccessAction::Delete,
            AccessAction::Share,
            AccessAction::Delegate,
        ] {
            let decision = controller
                .authorize_snapshot_access(
                    &requester,
                    ResourceType::Snapshot,
                    &CommSnapshotScope::default(),
                    action.clone(),
                )
                .unwrap();
            assert!(
                decision.outcome == DecisionOutcome::Permitted,
                "Coordinator should have {:?} access",
                action
            );
        }
    }

    #[test]
    fn test_observer_read_only() {
        let controller = AccessController::new();
        let requester = make_auth_info(AgentRole::Observer);

        let read_decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Snapshot,
                &CommSnapshotScope::default(),
                AccessAction::Read,
            )
            .unwrap();
        assert_eq!(read_decision.outcome, DecisionOutcome::Permitted);

        let write_decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Snapshot,
                &CommSnapshotScope::default(),
                AccessAction::Write,
            )
            .unwrap();
        assert_eq!(write_decision.outcome, DecisionOutcome::Denied);

        let delete_decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Belief,
                &CommSnapshotScope::default(),
                AccessAction::Delete,
            )
            .unwrap();
        assert_eq!(delete_decision.outcome, DecisionOutcome::Denied);
    }

    #[test]
    fn test_denied_access() {
        let controller = AccessController::new();
        let requester = make_auth_info(AgentRole::Observer);

        let decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Snapshot,
                &CommSnapshotScope::default(),
                AccessAction::Delete,
            )
            .unwrap();

        assert_eq!(decision.outcome, DecisionOutcome::Denied);
        assert!(decision.reason.is_some());
    }

    #[test]
    fn test_worker_conditional_permit() {
        let controller = AccessController::new();
        let requester = make_auth_info(AgentRole::Worker);

        let decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Snapshot,
                &CommSnapshotScope::default(),
                AccessAction::Read,
            )
            .unwrap();

        assert_eq!(decision.outcome, DecisionOutcome::Conditional);
        assert!(!decision.conditions.is_empty());
        assert!(!decision.obligations.is_empty());
    }

    #[test]
    fn test_collaborator_permissions() {
        let controller = AccessController::new();
        let requester = make_auth_info(AgentRole::Collaborator);

        let read_decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Belief,
                &CommSnapshotScope::default(),
                AccessAction::Read,
            )
            .unwrap();
        assert_eq!(read_decision.outcome, DecisionOutcome::Permitted);

        let write_decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Belief,
                &CommSnapshotScope::default(),
                AccessAction::Write,
            )
            .unwrap();
        assert_eq!(write_decision.outcome, DecisionOutcome::Permitted);

        let share_decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Belief,
                &CommSnapshotScope::default(),
                AccessAction::Share,
            )
            .unwrap();
        assert_eq!(share_decision.outcome, DecisionOutcome::Permitted);

        let delete_decision = controller
            .authorize_snapshot_access(
                &requester,
                ResourceType::Belief,
                &CommSnapshotScope::default(),
                AccessAction::Delete,
            )
            .unwrap();
        assert_eq!(delete_decision.outcome, DecisionOutcome::Denied);
    }

    #[test]
    fn test_check_authorization_with_request() {
        let controller = AccessController::new();
        let request = AuthorizationRequest {
            requester: make_auth_info(AgentRole::Coordinator),
            resource: ResourceDescriptor {
                resource_type: ResourceType::Intent,
                resource_id: Some("intent-1".to_string()),
                scope: None,
            },
            action: AccessAction::Delegate,
            context: AuthorizationContext {
                purpose: Some(SnapshotPurpose::Delegate),
                delegation_chain: Vec::new(),
                time_constraints: None,
            },
        };

        let decision = controller.check_authorization(request).unwrap();
        assert_eq!(decision.outcome, DecisionOutcome::Permitted);
    }
}
