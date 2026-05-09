//! PBSM 核心库
//!
//! 本库是 Predictive Belief State Machine (PBSM) 的核心实现，
//! 提供了预测引擎的完整功能，包括预测生成、验证和残差计算。
//!
//! # 模块结构
//!
//! - **error**：错误类型定义
//! - **types**：数据类型定义（预测、残差、过滤等）
//! - **modules**：核心模块（预测引擎、状态机、生成器、验证器等）
//!
//! # 使用示例
//!
//! ```ignore
//! use pbsm_core::{PredictionEngine, ActionRequest, ActionType};
//!
//! let engine = PredictionEngine::new();
//! let action = ActionRequest {
//!     action_type: ActionType::ToolCall,
//!     action_name: "unlock_file".to_string(),
//!     parameters: serde_json::json!({"file_id": "file-123"}),
//!     target_id: Some("file-123".to_string()),
//! };
//! let prediction = engine.create_prediction(action, None).await.unwrap();
//! ```
//!
//! # 架构说明
//!
//! 预测引擎采用以下架构：
//! - **M2 层**：核心预测引擎模块
//! - **预测生成**：基于信念图上下文生成预测
//! - **预测验证**：计算残差并更新预测状态
//! - **残差计算**：多维度（数值、语义、时间、结构）残差分析

pub mod error;
pub mod event_bus;
pub mod modules;
pub mod orchestrator;
pub mod types;

pub use error::PredictionError;
pub use event_bus::{
    IntentionStackEventAdapter, MemoryEventAdapter, MetacognitiveEventAdapter,
    PredictionEventAdapter, SystemEvent, SystemEventBus,
};
pub use modules::metacognition::*;
pub use modules::prediction_engine::*;
pub use orchestrator::{ExecuteCycleResult, HandleErrorResult, PbsmConfig, PbsmOrchestrator};
pub use types::*;

#[cfg(test)]
mod tests {
    use crate::modules::prediction_engine::PredictionEngine;
    use crate::types::prediction::{ActionRequest, ActionType};
    use crate::types::prediction::{Prediction, PredictionState};
    use crate::types::residual::MatchLevel;

    #[tokio::test]
    async fn test_end_to_end_prediction_flow() {
        let engine = PredictionEngine::new();

        let action = ActionRequest {
            action_type: ActionType::ToolCall,
            action_name: "unlock_file".to_string(),
            parameters: serde_json::json!({"file_id": "file-123", "expected_value": "unlocked"}),
            target_id: Some("file-123".to_string()),
        };

        let prediction = engine.create_prediction(action, None).await.unwrap();
        assert_eq!(prediction.status, PredictionState::Pending);

        let observation = crate::types::prediction::Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "unlocked", "file_id": "file-123"}),
            timestamp: chrono::Utc::now(),
            source: "tool_response".to_string(),
        };

        let result = engine
            .verify_prediction(&prediction.prediction_id.to_string(), observation)
            .await
            .unwrap();

        assert!(result.residual.overall_degree < 0.9);
    }

    #[test]
    fn test_prediction_state_machine() {
        let mut prediction = Prediction::new();
        assert_eq!(prediction.status, PredictionState::Pending);

        prediction
            .transition_to(PredictionState::Verified, "Test")
            .unwrap();
        assert_eq!(prediction.status, PredictionState::Verified);
        assert!(prediction.metadata.verified_at.is_some());
    }

    #[test]
    fn test_prediction_default_state() {
        let prediction = Prediction::new();
        assert_eq!(prediction.status, PredictionState::Pending);
        assert!(!prediction.status.is_terminal());
    }

    #[test]
    fn test_terminal_states() {
        assert!(PredictionState::Verified.is_terminal());
        assert!(PredictionState::Falsified.is_terminal());
        assert!(PredictionState::Expired.is_terminal());
        assert!(PredictionState::Cancelled.is_terminal());
        assert!(!PredictionState::Pending.is_terminal());
    }
}
