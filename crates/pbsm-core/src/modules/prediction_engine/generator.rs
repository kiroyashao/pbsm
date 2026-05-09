//! 预测生成器实现
//!
//! 本模块实现了预测生成的核心逻辑。
//! 预测生成采用四阶段流水线架构：动作解析 → 上下文检索 → 影响分析 → 输出。
//!
//! # 核心职责
//!
//! - 在动作执行前生成结构化的预测记录
//! - 基于当前信念状态推导预期变化
//! - 生成预期观测格式用于后续验证
//!
//! # 流水线说明
//!
//! 1. **动作解析阶段**：提取动作类型、参数和目标
//! 2. **上下文检索阶段**：从信念图获取相关信念节点
//! 3. **影响分析阶段**：推导动作执行后的预期状态变化
//! 4. **输出阶段**：组装完整的预测结构体

use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::PredictionError;
use crate::modules::common::{BeliefGraphReader, BeliefNode, NullBeliefGraphReader};
use crate::types::prediction::{
    ActionRequest, AssociatedAction, ChangeType, ContextSnapshot, ExpectedChange,
    ExpectedObservation, ExtractionHints, FieldMapping, Prediction, PredictionState,
    ValidityWindow,
};
use crate::types::residual::{DimensionWeights, SeverityThreshold};

/// 上下文检索提示结构体，控制上下文检索的行为
#[derive(Debug, Clone)]
pub struct ContextHint {
    /// 最大检索深度
    pub max_depth: Option<u32>,
    /// 是否包含历史信念节点
    pub include_historical: Option<bool>,
}

impl Default for ContextHint {
    fn default() -> Self {
        Self {
            max_depth: Some(2),
            include_historical: Some(false),
        }
    }
}

/// 预测生成器结构体，负责创建预测实例
///
/// # 设计说明
///
/// 预测生成器封装了预测创建的所有逻辑，包括：
/// - 依赖信念图管理器获取上下文
/// - 使用配置参数控制生成行为
/// - 四阶段流水线处理
#[allow(dead_code)]
pub struct PredictionGenerator {
    belief_graph: Arc<dyn BeliefGraphReader>,
    default_validity_window: ValidityWindow,
    default_weights: DimensionWeights,
    default_thresholds: SeverityThreshold,
}

impl PredictionGenerator {
    /// 创建新的预测生成器实例
    ///
    /// # 参数
    /// * `belief_graph` - 信念图读取接口
    ///
    /// # 返回
    /// * 配置好的 PredictionGenerator 实例，使用默认参数
    pub fn new(belief_graph: Arc<dyn BeliefGraphReader>) -> Self {
        Self {
            belief_graph,
            default_validity_window: ValidityWindow::default(),
            default_weights: DimensionWeights::default(),
            default_thresholds: SeverityThreshold::default(),
        }
    }

    /// 使用默认参数创建预测生成器
    ///
    /// # 参数
    /// * `belief_graph` - 信念图读取接口
    ///
    /// # 返回
    /// * 配置好的 PredictionGenerator 实例
    pub fn with_defaults(belief_graph: Arc<dyn BeliefGraphReader>) -> Self {
        Self {
            belief_graph,
            default_validity_window: ValidityWindow::new_steps_window(10),
            default_weights: DimensionWeights::default(),
            default_thresholds: SeverityThreshold::default(),
        }
    }

    /// 创建预测（主入口）
    ///
    /// # 参数
    /// * `action_request` - 动作请求
    /// * `context_hint` - 上下文检索提示（可选）
    ///
    /// # 返回
    /// * `Ok(Prediction)` - 创建的预测实例
    /// * `Err(PredictionError)` - 创建失败
    ///
    /// # 流水线处理
    ///
    /// 1. 调用 `retrieve_context` 检索相关信念节点
    /// 2. 调用 `analyze_effects` 分析预期变化
    /// 3. 调用 `build_prediction` 组装预测结构体
    pub async fn create_prediction(
        &self,
        action_request: ActionRequest,
        context_hint: Option<ContextHint>,
    ) -> Result<Prediction, PredictionError> {
        let hint = context_hint.unwrap_or_default();

        let context = self
            .retrieve_context(&action_request, hint)
            .await
            .map_err(|e| PredictionError::ContextIncomplete(e.to_string()))?;

        let expected_changes = self
            .analyze_effects(&action_request, &context)
            .map_err(|e| PredictionError::InvalidAction(e.to_string()))?;

        let prediction = self
            .build_prediction(action_request, expected_changes, context)
            .map_err(|e| PredictionError::InvalidAction(e.to_string()))?;

        Ok(prediction)
    }

    /// 阶段1：上下文检索
    ///
    /// 从信念图中检索与动作相关的信念节点
    ///
    /// # 参数
    /// * `action_request` - 动作请求
    /// * `hint` - 检索提示
    ///
    /// # 返回
    /// * `Ok(ContextRetrievalResult)` - 检索结果
    async fn retrieve_context(
        &self,
        action_request: &ActionRequest,
        hint: ContextHint,
    ) -> Result<ContextRetrievalResult, PredictionError> {
        let mut relevant_nodes = Vec::new();

        if let Some(ref target_id) = action_request.target_id {
            if let Ok(Some(node)) = self.belief_graph.query_belief_by_id(target_id).await {
                relevant_nodes.push(node);
            }

            if let Ok(edges) = self.belief_graph.get_outgoing_edges(target_id).await {
                for edge in edges {
                    if let Ok(Some(node)) = self
                        .belief_graph
                        .query_belief_by_id(&edge.target_node)
                        .await
                    {
                        if hint.max_depth.unwrap_or(2) >= 1 {
                            relevant_nodes.push(node);
                        }
                    }
                }
            }
        }

        Ok(ContextRetrievalResult {
            target_beliefs: relevant_nodes,
            belief_state_hash: format!("hash_{}", Uuid::new_v4()),
        })
    }

    /// 阶段2：影响分析
    ///
    /// 基于动作类型和上下文分析预期发生的变化
    ///
    /// # 参数
    /// * `action_request` - 动作请求
    /// * `context` - 上下文检索结果
    ///
    /// # 返回
    /// * `Ok(Vec<ExpectedChange>)` - 预期变化列表
    fn analyze_effects(
        &self,
        action_request: &ActionRequest,
        context: &ContextRetrievalResult,
    ) -> Result<Vec<ExpectedChange>, PredictionError> {
        let mut expected_changes = Vec::new();

        if let Some(ref target_id) = action_request.target_id {
            let target_node = context
                .target_beliefs
                .iter()
                .find(|n| n.node_id == *target_id);

            let previous_value = target_node
                .map(|n| n.attributes.clone())
                .unwrap_or(serde_json::Value::Null);

            let change_type = ChangeType::Modify;

            let expected_value = self.derive_expected_value(action_request, &previous_value);

            expected_changes.push(ExpectedChange {
                change_id: Uuid::new_v4(),
                node_id: target_id.clone(),
                attribute: Some("state".to_string()),
                expected_value,
                previous_value,
                change_type,
                expected_confidence: 0.8,
                derivation_path: vec!["generator".to_string()],
            });
        }

        Ok(expected_changes)
    }

    /// 推导预期值
    ///
    /// 根据动作参数和前值推导预期的结果值
    fn derive_expected_value(
        &self,
        action_request: &ActionRequest,
        previous_value: &serde_json::Value,
    ) -> serde_json::Value {
        {
            if let Some(params) = action_request.parameters.as_object() {
                if let Some(expected) = params.get("expected_value") {
                    return expected.clone();
                }
            }
            previous_value.clone()
        }
    }

    /// 阶段3和4：组装预测结构体
    ///
    /// 将分析结果组装成完整的预测实例
    ///
    /// # 参数
    /// * `action_request` - 原始动作请求
    /// * `expected_changes` - 预期变化列表
    /// * `context` - 上下文检索结果
    ///
    /// # 返回
    /// * `Ok(Prediction)` - 完整的预测实例
    fn build_prediction(
        &self,
        action_request: ActionRequest,
        expected_changes: Vec<ExpectedChange>,
        context: ContextRetrievalResult,
    ) -> Result<Prediction, PredictionError> {
        let prediction_id = Uuid::new_v4();

        let associated_action = AssociatedAction {
            action_id: Uuid::new_v4(),
            action_type: action_request.action_type,
            action_name: action_request.action_name.clone(),
            parameters: action_request.parameters.clone(),
            target_node: action_request.target_id.clone(),
            affected_nodes: expected_changes.iter().map(|c| c.node_id.clone()).collect(),
        };

        let field_mappings: Vec<FieldMapping> = expected_changes
            .iter()
            .filter_map(|c| {
                c.attribute.as_ref().map(|attr| FieldMapping {
                    output_field: attr.clone(),
                    maps_to_node: c.node_id.clone(),
                    maps_to_attribute: attr.clone(),
                })
            })
            .collect();

        let expected_observation = ExpectedObservation {
            format: "json".to_string(),
            sample_value: Some(serde_json::json!({"success": true})),
            field_mappings,
            extraction_hints: ExtractionHints::default(),
        };

        let context_snapshot = ContextSnapshot {
            belief_state_hash: context.belief_state_hash,
            relevant_nodes: context
                .target_beliefs
                .iter()
                .map(|n| n.node_id.clone())
                .collect(),
            intention_level: 0,
        };

        let now = Utc::now();

        Ok(Prediction {
            prediction_id,
            version: 1,
            associated_action,
            expected_changes,
            expected_observation,
            validity_window: self.default_validity_window.clone(),
            status: PredictionState::Pending,
            status_history: vec![],
            residuals: None,
            context_snapshot,
            metadata: crate::types::prediction::PredictionMetadata {
                created_at: now,
                created_by: "M2".to_string(),
                updated_at: now,
                verified_at: None,
                confidence: 0.5,
                tags: Vec::new(),
            },
        })
    }
}

impl Default for PredictionGenerator {
    fn default() -> Self {
        Self::with_defaults(Arc::new(NullBeliefGraphReader))
    }
}

/// 上下文检索结果结构体
#[derive(Debug)]
pub struct ContextRetrievalResult {
    /// 检索到的相关信念节点列表
    pub target_beliefs: Vec<BeliefNode>,
    /// 信念状态哈希值
    pub belief_state_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_prediction_basic() {
        let generator = PredictionGenerator::default();

        let action = ActionRequest {
            action_type: crate::types::prediction::ActionType::ToolCall,
            action_name: "test_action".to_string(),
            parameters: serde_json::json!({"expected_value": "success"}),
            target_id: Some("node-123".to_string()),
        };

        let result = generator.create_prediction(action, None).await;
        assert!(result.is_ok());

        let prediction = result.unwrap();
        assert_eq!(prediction.status, PredictionState::Pending);
        assert!(prediction.prediction_id != Uuid::nil());
    }

    #[tokio::test]
    async fn test_prediction_with_changes() {
        let generator = PredictionGenerator::default();

        let action = ActionRequest {
            action_type: crate::types::prediction::ActionType::ToolCall,
            action_name: "update_status".to_string(),
            parameters: serde_json::json!({"expected_value": "completed"}),
            target_id: Some("file-456".to_string()),
        };

        let result = generator.create_prediction(action, None).await;
        assert!(result.is_ok());

        let prediction = result.unwrap();
        assert!(!prediction.expected_changes.is_empty());
        assert_eq!(prediction.expected_changes[0].node_id, "file-456");
    }

    #[test]
    fn test_context_hint_default() {
        let hint = ContextHint::default();
        assert_eq!(hint.max_depth, Some(2));
        assert_eq!(hint.include_historical, Some(false));
    }
}
