//! 预测状态机实现
//!
//! 本模块实现了预测状态机的核心逻辑。
//! 预测状态机定义了预测对象从创建到终结的完整生命周期。
//!
//! # 状态转换规则
//!
//! - Pending 是唯一允许转换到其他状态的起始状态
//! - Verified、Falsified、Expired、Cancelled 均为终态
//! - 只有 Pending 状态可以转换到其他状态

use crate::types::prediction::{PredictionState, StatusHistoryEntry};

/// 状态转换错误结构体
#[derive(Debug, Clone)]
pub struct StateTransitionError {
    pub from: PredictionState,
    pub to: PredictionState,
    pub reason: String,
}

impl std::fmt::Display for StateTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid state transition from {:?} to {:?}: {}",
            self.from, self.to, self.reason
        )
    }
}

impl std::error::Error for StateTransitionError {}

/// 预测状态机结构体，管理预测的状态转换和历史记录
///
/// # 设计说明
///
/// 状态机封装了预测状态转换的所有逻辑，包括：
/// - 状态转换验证
/// - 状态历史记录
/// - 终态判定
pub struct PredictionStateMachine {
    state: PredictionState,
    status_history: Vec<StatusHistoryEntry>,
}

impl PredictionStateMachine {
    /// 创建新的状态机实例，初始状态为 Pending
    pub fn new() -> Self {
        Self {
            state: PredictionState::Pending,
            status_history: Vec::new(),
        }
    }

    /// 使用指定初始状态创建状态机
    ///
    /// # 参数
    /// * `state` - 初始状态
    ///
    /// # 说明
    ///
    /// 如果指定的状态不是 Pending，会自动创建一条初始状态的历史记录
    pub fn with_state(state: PredictionState) -> Self {
        let history = if state != PredictionState::Pending {
            vec![StatusHistoryEntry::new(
                state,
                "Initial state",
                Some("M2".to_string()),
            )]
        } else {
            Vec::new()
        };
        Self {
            state,
            status_history: history,
        }
    }

    /// 执行状态转换
    ///
    /// # 参数
    /// * `new_state` - 目标状态
    /// * `reason` - 转换原因描述
    ///
    /// # 返回
    /// * `Ok(())` - 转换成功
    /// * `Err(StateTransitionError)` - 转换失败（状态不合法）
    pub fn transition(
        &mut self,
        new_state: PredictionState,
        reason: &str,
    ) -> Result<(), StateTransitionError> {
        if !self.state.can_transition_to(new_state) {
            return Err(StateTransitionError {
                from: self.state,
                to: new_state,
                reason: "Invalid state transition".to_string(),
            });
        }

        let entry = StatusHistoryEntry::new(new_state, reason, Some("M2".to_string()));
        self.status_history.push(entry);
        self.state = new_state;
        Ok(())
    }

    /// 获取当前状态
    pub fn current_state(&self) -> PredictionState {
        self.state
    }

    /// 获取状态历史记录
    pub fn status_history(&self) -> &[StatusHistoryEntry] {
        &self.status_history
    }

    /// 判断是否可以转换到目标状态
    pub fn can_transition_to(&self, target: PredictionState) -> bool {
        self.state.can_transition_to(target)
    }

    /// 判断当前状态是否为终态
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }
}

impl Default for PredictionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_machine_creation() {
        let sm = PredictionStateMachine::new();
        assert_eq!(sm.current_state(), PredictionState::Pending);
        assert!(!sm.is_terminal());
    }

    #[test]
    fn test_valid_transitions() {
        let mut sm = PredictionStateMachine::new();

        assert!(sm.transition(PredictionState::Verified, "Test").is_ok());
        assert_eq!(sm.current_state(), PredictionState::Verified);
        assert!(sm.is_terminal());

        assert!(sm.status_history().len() == 1);
    }

    #[test]
    fn test_invalid_transitions() {
        let mut sm = PredictionStateMachine::new();

        sm.transition(PredictionState::Verified, "First transition")
            .unwrap();

        let result = sm.transition(PredictionState::Pending, "Should fail");
        assert!(result.is_err());
        assert_eq!(sm.current_state(), PredictionState::Verified);
    }

    #[test]
    fn test_all_pending_transitions() {
        let mut sm = PredictionStateMachine::new();

        assert!(sm.can_transition_to(PredictionState::Verified));
        assert!(sm.can_transition_to(PredictionState::Falsified));
        assert!(sm.can_transition_to(PredictionState::Expired));
        assert!(sm.can_transition_to(PredictionState::Cancelled));

        assert!(sm.transition(PredictionState::Verified, "Test").is_ok());
        assert!(sm
            .transition(PredictionState::Falsified, "Should fail")
            .is_err());
    }

    #[test]
    fn test_status_history() {
        let mut sm = PredictionStateMachine::new();
        assert!(sm.status_history().is_empty());

        sm.transition(PredictionState::Verified, "Verified")
            .unwrap();
        assert_eq!(sm.status_history().len(), 1);
        assert_eq!(sm.status_history()[0].status, PredictionState::Verified);
    }
}
