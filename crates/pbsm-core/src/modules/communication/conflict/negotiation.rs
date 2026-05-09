use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::*;

use super::detector::{Conflict, ConflictType};

#[derive(Clone, Debug, PartialEq)]
pub enum NegotiationType {
    ValueResolution,
    IntentAlignment,
    PriorityNegotiation,
    ResourceAllocation,
}

#[derive(Clone, Debug)]
pub struct AgentInfo {
    pub agent_id: String,
    pub role: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ConflictReference {
    pub conflict_id: String,
    pub conflict_type: ConflictType,
}

#[derive(Clone, Debug)]
pub struct Proposal {
    pub proposal_id: String,
    pub proposed_value: serde_json::Value,
    pub justification: ProposalJustification,
    pub confidence: f64,
}

#[derive(Clone, Debug)]
pub struct ProposalJustification {
    pub reasoning: String,
    pub evidence: Vec<String>,
    pub authority: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NegotiationState {
    Initiated,
    AwaitingResponse,
    CounterProposed,
    Accepted,
    Rejected,
    Expired,
}

#[derive(Clone, Debug)]
pub struct NegotiationContext {
    pub scope: String,
    pub priority: Priority,
    pub deadline: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct NegotiationMetadata {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub round: u32,
    pub max_rounds: u32,
}

#[derive(Clone, Debug)]
pub struct NegotiationOptions {
    pub max_rounds: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub auto_accept_threshold: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct NegotiationResponse {
    pub response_type: ResponseType,
    pub data: ResponseData,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseType {
    Accept,
    Reject,
    CounterProposal,
}

#[derive(Clone, Debug)]
pub struct ResponseData {
    pub justification: Option<String>,
    pub counter_proposal: Option<CounterProposal>,
}

#[derive(Clone, Debug)]
pub struct CounterProposal {
    pub proposal: Proposal,
    pub rationale: String,
}

#[derive(Clone, Debug)]
pub struct NegotiationResult {
    pub negotiation_id: String,
    pub outcome: NegotiationOutcome,
    pub resolution: Option<Resolution>,
    pub session: NegotiationSession,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NegotiationOutcome {
    Accepted,
    Rejected,
    CounterProposed,
    Expired,
    Pending,
}

#[derive(Clone, Debug)]
pub struct Resolution {
    pub resolved_value: serde_json::Value,
    pub affected_beliefs: Vec<AffectedBelief>,
    pub commitments: Vec<Commitment>,
}

#[derive(Clone, Debug)]
pub struct AffectedBelief {
    pub node_id: String,
    pub action: BeliefAction,
    pub original_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BeliefAction {
    Update,
    Remove,
    Add,
}

#[derive(Clone, Debug)]
pub struct Commitment {
    pub agent_id: String,
    pub commitment_type: String,
    pub description: String,
    pub deadline: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug)]
pub struct NegotiationSession {
    pub negotiation_id: String,
    pub negotiation_type: NegotiationType,
    pub initiator: AgentInfo,
    pub respondent: String,
    pub conflict_reference: ConflictReference,
    pub proposals: Vec<Proposal>,
    pub state: NegotiationState,
    pub context: NegotiationContext,
    pub metadata: NegotiationMetadata,
    pub expires_at: Option<DateTime<Utc>>,
}

pub struct NegotiationHandler {
    sessions: RwLock<HashMap<String, NegotiationSession>>,
}

impl Default for NegotiationHandler {
    fn default() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

impl NegotiationHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initiate_negotiation(
        &self,
        conflict: Conflict,
        proposals: Vec<Proposal>,
        options: Option<NegotiationOptions>,
    ) -> Result<NegotiationSession, CommunicationError> {
        let negotiation_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let opts = options.unwrap_or(NegotiationOptions {
            max_rounds: Some(5),
            timeout_ms: None,
            auto_accept_threshold: None,
        });

        let negotiation_type = match conflict.conflict_type {
            ConflictType::AttributeMismatch => NegotiationType::ValueResolution,
            ConflictType::RelationMismatch => NegotiationType::ValueResolution,
            ConflictType::IntentMismatch => NegotiationType::IntentAlignment,
            ConflictType::ValueConfidenceConflict => NegotiationType::PriorityNegotiation,
        };

        let initiator = if let Some(entity) = conflict.affected_entities.first() {
            AgentInfo {
                agent_id: entity.local_belief.node_id.clone(),
                role: None,
                capabilities: Vec::new(),
            }
        } else {
            AgentInfo {
                agent_id: "unknown".to_string(),
                role: None,
                capabilities: Vec::new(),
            }
        };

        let respondent = conflict
            .affected_entities
            .first()
            .map(|e| e.remote_belief.node_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let session = NegotiationSession {
            negotiation_id: negotiation_id.clone(),
            negotiation_type,
            initiator,
            respondent,
            conflict_reference: ConflictReference {
                conflict_id: conflict.conflict_id.clone(),
                conflict_type: conflict.conflict_type,
            },
            proposals,
            state: NegotiationState::Initiated,
            context: NegotiationContext {
                scope: conflict.context.scope.clone(),
                priority: Priority::Normal,
                deadline: None,
            },
            metadata: NegotiationMetadata {
                created_at: now,
                updated_at: now,
                round: 1,
                max_rounds: opts.max_rounds.unwrap_or(5),
            },
            expires_at: opts
                .timeout_ms
                .map(|ms| now + chrono::Duration::milliseconds(ms as i64)),
        };

        self.sessions
            .write()
            .insert(negotiation_id, session.clone());

        Ok(session)
    }

    pub fn respond_to_negotiation(
        &self,
        negotiation_id: &str,
        response: NegotiationResponse,
    ) -> Result<NegotiationResult, CommunicationError> {
        let mut sessions = self.sessions.write();
        let session =
            sessions
                .get_mut(negotiation_id)
                .ok_or_else(|| CommunicationError::InternalError {
                    context: format!("Negotiation {} not found", negotiation_id),
                })?;

        if session.state != NegotiationState::Initiated
            && session.state != NegotiationState::AwaitingResponse
            && session.state != NegotiationState::CounterProposed
        {
            return Err(CommunicationError::InternalError {
                context: format!("Cannot respond to negotiation in state {:?}", session.state),
            });
        }

        let (outcome, resolution) = match response.response_type {
            ResponseType::Accept => {
                session.state = NegotiationState::Accepted;
                let best_proposal = session.proposals.last().cloned();
                let resolved_value = best_proposal
                    .map(|p| p.proposed_value.clone())
                    .unwrap_or(serde_json::Value::Null);
                (
                    NegotiationOutcome::Accepted,
                    Some(Resolution {
                        resolved_value,
                        affected_beliefs: Vec::new(),
                        commitments: Vec::new(),
                    }),
                )
            }
            ResponseType::Reject => {
                session.state = NegotiationState::Rejected;
                (NegotiationOutcome::Rejected, None)
            }
            ResponseType::CounterProposal => {
                session.state = NegotiationState::CounterProposed;
                session.metadata.round += 1;
                if let Some(cp) = &response.data.counter_proposal {
                    session.proposals.push(cp.proposal.clone());
                }
                (NegotiationOutcome::CounterProposed, None)
            }
        };

        session.metadata.updated_at = Utc::now();

        let result = NegotiationResult {
            negotiation_id: negotiation_id.to_string(),
            outcome,
            resolution,
            session: session.clone(),
        };

        Ok(result)
    }

    pub fn submit_counter_proposal(
        &self,
        negotiation_id: &str,
        counter_proposal: Proposal,
    ) -> Result<NegotiationResult, CommunicationError> {
        let mut sessions = self.sessions.write();
        let session =
            sessions
                .get_mut(negotiation_id)
                .ok_or_else(|| CommunicationError::InternalError {
                    context: format!("Negotiation {} not found", negotiation_id),
                })?;

        if session.state != NegotiationState::Initiated
            && session.state != NegotiationState::AwaitingResponse
            && session.state != NegotiationState::CounterProposed
        {
            return Err(CommunicationError::InternalError {
                context: format!(
                    "Cannot submit counter proposal in state {:?}",
                    session.state
                ),
            });
        }

        session.proposals.push(counter_proposal);
        session.state = NegotiationState::CounterProposed;
        session.metadata.round += 1;
        session.metadata.updated_at = Utc::now();

        Ok(NegotiationResult {
            negotiation_id: negotiation_id.to_string(),
            outcome: NegotiationOutcome::CounterProposed,
            resolution: None,
            session: session.clone(),
        })
    }

    pub fn get_negotiation_status(
        &self,
        negotiation_id: &str,
    ) -> Result<NegotiationSession, CommunicationError> {
        self.sessions
            .read()
            .get(negotiation_id)
            .cloned()
            .ok_or_else(|| CommunicationError::InternalError {
                context: format!("Negotiation {} not found", negotiation_id),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::communication::conflict::detector::{
        AffectedEntity, BeliefState, ConflictContext, Divergence, ImpactAssessment,
    };

    fn make_test_conflict() -> Conflict {
        Conflict {
            conflict_id: "conflict-1".to_string(),
            conflict_type: ConflictType::AttributeMismatch,
            affected_entities: vec![AffectedEntity {
                local_belief: BeliefState {
                    node_id: "agent-1".to_string(),
                    attributes: HashMap::new(),
                    confidence: 0.9,
                    source: None,
                    last_updated: None,
                },
                remote_belief: BeliefState {
                    node_id: "agent-2".to_string(),
                    attributes: HashMap::new(),
                    confidence: 0.7,
                    source: None,
                    last_updated: None,
                },
            }],
            divergence: Divergence {
                attribute_name: "role".to_string(),
                local_value: serde_json::json!("admin"),
                remote_value: serde_json::json!("viewer"),
                deviation_metric: 0.2,
            },
            context: ConflictContext {
                scope: "entity:node-1".to_string(),
                intent_relevance: 0.8,
                impact_assessment: ImpactAssessment::Medium,
            },
            detected_at: Utc::now(),
        }
    }

    fn make_test_proposal(value: &str) -> Proposal {
        Proposal {
            proposal_id: Uuid::new_v4().to_string(),
            proposed_value: serde_json::json!(value),
            justification: ProposalJustification {
                reasoning: "test reasoning".to_string(),
                evidence: Vec::new(),
                authority: None,
            },
            confidence: 0.8,
        }
    }

    #[test]
    fn test_initiate_negotiation() {
        let handler = NegotiationHandler::new();
        let conflict = make_test_conflict();
        let proposals = vec![make_test_proposal("editor")];

        let session = handler
            .initiate_negotiation(conflict, proposals, None)
            .unwrap();

        assert_eq!(session.state, NegotiationState::Initiated);
        assert_eq!(session.negotiation_type, NegotiationType::ValueResolution);
        assert_eq!(session.initiator.agent_id, "agent-1");
        assert_eq!(session.respondent, "agent-2");
        assert_eq!(session.proposals.len(), 1);
        assert_eq!(session.metadata.round, 1);
    }

    #[test]
    fn test_respond_accept() {
        let handler = NegotiationHandler::new();
        let conflict = make_test_conflict();
        let proposals = vec![make_test_proposal("editor")];

        let session = handler
            .initiate_negotiation(conflict, proposals, None)
            .unwrap();
        let neg_id = session.negotiation_id.clone();

        let result = handler
            .respond_to_negotiation(
                &neg_id,
                NegotiationResponse {
                    response_type: ResponseType::Accept,
                    data: ResponseData {
                        justification: None,
                        counter_proposal: None,
                    },
                },
            )
            .unwrap();

        assert_eq!(result.outcome, NegotiationOutcome::Accepted);
        assert!(result.resolution.is_some());
        assert_eq!(result.session.state, NegotiationState::Accepted);
    }

    #[test]
    fn test_respond_reject() {
        let handler = NegotiationHandler::new();
        let conflict = make_test_conflict();
        let proposals = vec![make_test_proposal("editor")];

        let session = handler
            .initiate_negotiation(conflict, proposals, None)
            .unwrap();
        let neg_id = session.negotiation_id.clone();

        let result = handler
            .respond_to_negotiation(
                &neg_id,
                NegotiationResponse {
                    response_type: ResponseType::Reject,
                    data: ResponseData {
                        justification: Some("Not acceptable".to_string()),
                        counter_proposal: None,
                    },
                },
            )
            .unwrap();

        assert_eq!(result.outcome, NegotiationOutcome::Rejected);
        assert!(result.resolution.is_none());
        assert_eq!(result.session.state, NegotiationState::Rejected);
    }

    #[test]
    fn test_counter_proposal() {
        let handler = NegotiationHandler::new();
        let conflict = make_test_conflict();
        let proposals = vec![make_test_proposal("editor")];

        let session = handler
            .initiate_negotiation(conflict, proposals, None)
            .unwrap();
        let neg_id = session.negotiation_id.clone();

        let counter = make_test_proposal("moderator");
        let result = handler.submit_counter_proposal(&neg_id, counter).unwrap();

        assert_eq!(result.outcome, NegotiationOutcome::CounterProposed);
        assert_eq!(result.session.proposals.len(), 2);
        assert_eq!(result.session.metadata.round, 2);
    }

    #[test]
    fn test_get_negotiation_status() {
        let handler = NegotiationHandler::new();
        let conflict = make_test_conflict();
        let proposals = vec![make_test_proposal("editor")];

        let session = handler
            .initiate_negotiation(conflict, proposals, None)
            .unwrap();
        let neg_id = session.negotiation_id.clone();

        let status = handler.get_negotiation_status(&neg_id).unwrap();
        assert_eq!(status.negotiation_id, neg_id);
        assert_eq!(status.state, NegotiationState::Initiated);

        let not_found = handler.get_negotiation_status("nonexistent");
        assert!(not_found.is_err());
    }
}
