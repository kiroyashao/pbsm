use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Pending,
    Ready,
    InProgress,
    WaitingFeedback,
    Suspended,
    Completed,
    PartiallyCompleted,
    Abandoned,
    Failed,
    RolledBack,
}

impl ExecutionState {
    pub fn all_states() -> &'static [ExecutionState] {
        &[
            Self::Pending,
            Self::Ready,
            Self::InProgress,
            Self::WaitingFeedback,
            Self::Suspended,
            Self::Completed,
            Self::PartiallyCompleted,
            Self::Abandoned,
            Self::Failed,
            Self::RolledBack,
        ]
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }

    pub fn allowed_transitions(&self) -> Vec<ExecutionState> {
        match self {
            Self::Pending => vec![Self::Ready, Self::Suspended, Self::Abandoned],
            Self::Ready => vec![Self::InProgress, Self::Suspended, Self::Abandoned],
            Self::InProgress => vec![
                Self::WaitingFeedback,
                Self::Completed,
                Self::PartiallyCompleted,
                Self::Failed,
                Self::RolledBack,
            ],
            Self::WaitingFeedback => vec![
                Self::InProgress,
                Self::Failed,
                Self::Suspended,
                Self::RolledBack,
            ],
            Self::Suspended => vec![Self::Ready, Self::Abandoned],
            Self::Completed | Self::Abandoned => vec![],
            Self::PartiallyCompleted => vec![Self::Completed, Self::Abandoned],
            Self::Failed => vec![Self::Ready, Self::RolledBack, Self::Abandoned],
            Self::RolledBack => vec![Self::Ready, Self::Abandoned],
        }
    }

    pub fn can_transition_to(&self, target: &ExecutionState) -> bool {
        self.allowed_transitions().contains(target)
    }
}

impl std::fmt::Display for ExecutionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Ready => write!(f, "ready"),
            Self::InProgress => write!(f, "in_progress"),
            Self::WaitingFeedback => write!(f, "waiting_feedback"),
            Self::Suspended => write!(f, "suspended"),
            Self::Completed => write!(f, "completed"),
            Self::PartiallyCompleted => write!(f, "partially_completed"),
            Self::Abandoned => write!(f, "abandoned"),
            Self::Failed => write!(f, "failed"),
            Self::RolledBack => write!(f, "rolled_back"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    None,
    Minor,
    Moderate,
    Severe,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum GoalPriority {
    Critical,
    High,
    #[default]
    Medium,
    Low,
}

impl GoalPriority {
    pub fn weight(&self) -> f32 {
        match self {
            Self::Critical => 1.0,
            Self::High => 0.75,
            Self::Medium => 0.5,
            Self::Low => 0.25,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_transitions() {
        let state = ExecutionState::Pending;
        assert!(state.can_transition_to(&ExecutionState::Ready));
        assert!(state.can_transition_to(&ExecutionState::Suspended));
        assert!(state.can_transition_to(&ExecutionState::Abandoned));
        assert!(!state.can_transition_to(&ExecutionState::InProgress));
        assert!(!state.can_transition_to(&ExecutionState::Completed));
    }

    #[test]
    fn test_ready_transitions() {
        let state = ExecutionState::Ready;
        assert!(state.can_transition_to(&ExecutionState::InProgress));
        assert!(state.can_transition_to(&ExecutionState::Suspended));
        assert!(state.can_transition_to(&ExecutionState::Abandoned));
        assert!(!state.can_transition_to(&ExecutionState::Pending));
    }

    #[test]
    fn test_in_progress_transitions() {
        let state = ExecutionState::InProgress;
        assert!(state.can_transition_to(&ExecutionState::WaitingFeedback));
        assert!(state.can_transition_to(&ExecutionState::Completed));
        assert!(state.can_transition_to(&ExecutionState::PartiallyCompleted));
        assert!(state.can_transition_to(&ExecutionState::Failed));
        assert!(state.can_transition_to(&ExecutionState::RolledBack));
        assert!(!state.can_transition_to(&ExecutionState::Pending));
    }

    #[test]
    fn test_terminal_states() {
        assert!(ExecutionState::Completed.is_terminal());
        assert!(ExecutionState::Abandoned.is_terminal());
        assert!(!ExecutionState::Pending.is_terminal());
        assert!(!ExecutionState::Failed.is_terminal());
    }

    #[test]
    fn test_completed_no_transitions() {
        let state = ExecutionState::Completed;
        assert!(state.allowed_transitions().is_empty());
    }

    #[test]
    fn test_failed_transitions() {
        let state = ExecutionState::Failed;
        assert!(state.can_transition_to(&ExecutionState::Ready));
        assert!(state.can_transition_to(&ExecutionState::RolledBack));
        assert!(state.can_transition_to(&ExecutionState::Abandoned));
        assert!(!state.can_transition_to(&ExecutionState::InProgress));
    }

    #[test]
    fn test_rolled_back_transitions() {
        let state = ExecutionState::RolledBack;
        assert!(state.can_transition_to(&ExecutionState::Ready));
        assert!(state.can_transition_to(&ExecutionState::Abandoned));
        assert!(!state.can_transition_to(&ExecutionState::InProgress));
    }

    #[test]
    fn test_display() {
        assert_eq!(ExecutionState::Pending.to_string(), "pending");
        assert_eq!(ExecutionState::InProgress.to_string(), "in_progress");
        assert_eq!(
            ExecutionState::WaitingFeedback.to_string(),
            "waiting_feedback"
        );
        assert_eq!(ExecutionState::RolledBack.to_string(), "rolled_back");
    }

    #[test]
    fn test_goal_priority_weight() {
        assert!((GoalPriority::Critical.weight() - 1.0).abs() < f32::EPSILON);
        assert!((GoalPriority::High.weight() - 0.75).abs() < f32::EPSILON);
        assert!((GoalPriority::Medium.weight() - 0.5).abs() < f32::EPSILON);
        assert!((GoalPriority::Low.weight() - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_goal_priority_default() {
        assert_eq!(GoalPriority::default(), GoalPriority::Medium);
    }

    #[test]
    fn test_all_states_count() {
        assert_eq!(ExecutionState::all_states().len(), 10);
    }
}
