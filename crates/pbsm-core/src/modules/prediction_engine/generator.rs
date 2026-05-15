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
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::PredictionError;
use crate::modules::common::{
    BeliefGraphReader, BeliefNode, ContextIntegrityWarningPayload, EventPublisher,
    NullBeliefGraphReader, NullEventPublisher, PBSMEvent, PredictionEvent, RelationEdge,
};
use crate::types::prediction::{
    ActionRequest, ActionType, AssociatedAction, ChangeType, ContextSnapshot, ExpectedChange,
    ExpectedObservation, ExtractionHints, FieldMapping, Prediction, PredictionState,
    StatusHistoryEntry, ValidityWindow,
};
use crate::types::residual::{DimensionWeights, SeverityThreshold};

/// 上下文检索提示结构体，控制上下文检索的行为
const MIN_CONFIDENCE_THRESHOLD: f64 = 0.3;

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
struct ParsedAction {
    action_type: ActionType,
    target_node_id: String,
    semantic_effects: Vec<ActionSemanticEffect>,
    expected_observation_format: ExpectedObservationFormat,
    validity_window: ValidityWindow,
}

#[allow(dead_code)]
struct ActionSemanticEffect {
    field: String,
    change_type: ChangeType,
    expected_value: serde_json::Value,
    confidence: f64,
}

#[allow(dead_code)]
enum ExpectedObservationFormat {
    ExactMatch,
    PartialMatch { required_fields: Vec<String> },
    PatternMatch { pattern: String },
}

pub struct PredictionGenerator {
    belief_graph: Arc<dyn BeliefGraphReader>,
    event_publisher: Arc<dyn EventPublisher>,
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
            event_publisher: Arc::new(NullEventPublisher),
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
            event_publisher: Arc::new(NullEventPublisher),
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
    pub fn with_event_publisher(mut self, publisher: Arc<dyn EventPublisher>) -> Self {
        self.event_publisher = publisher;
        self
    }

    pub async fn create_prediction(
        &self,
        action_request: ActionRequest,
        context_hint: Option<ContextHint>,
    ) -> Result<Prediction, PredictionError> {
        let hint = context_hint.unwrap_or_default();

        let parsed_action = self.parse_action(&action_request, &hint)?;

        let context = self
            .retrieve_context(&action_request, &parsed_action, hint)
            .await
            .map_err(|e| PredictionError::ContextIncomplete {
                message: e.to_string(),
                code: "PEV-E002".to_string(),
            })?;

        let expected_changes = self
            .analyze_effects(&action_request, &parsed_action, &context)
            .map_err(|e| PredictionError::InvalidAction {
                message: e.to_string(),
                code: "PEV-E003".to_string(),
            })?;

        let prediction = self
            .build_prediction(action_request, expected_changes, context)
            .map_err(|e| PredictionError::InvalidAction {
                message: e.to_string(),
                code: "PEV-E003".to_string(),
            })?;

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
    fn parse_action(
        &self,
        action_request: &ActionRequest,
        hint: &ContextHint,
    ) -> Result<ParsedAction, PredictionError> {
        let target_node_id = action_request.target_id.clone().unwrap_or_default();

        let semantic_effects = match action_request.action_type {
            ActionType::BeliefUpdate => {
                vec![ActionSemanticEffect {
                    field: action_request
                        .parameters
                        .get("attribute")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "state".to_string()),
                    change_type: ChangeType::Modify,
                    expected_value: action_request
                        .parameters
                        .get("expected_value")
                        .or_else(|| action_request.parameters.get("new_value"))
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                    confidence: 0.8,
                }]
            }
            ActionType::StateTransition => {
                vec![ActionSemanticEffect {
                    field: "state".to_string(),
                    change_type: ChangeType::Modify,
                    expected_value: action_request
                        .parameters
                        .get("expected_value")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                    confidence: 0.7,
                }]
            }
            ActionType::InformationQuery => {
                vec![ActionSemanticEffect {
                    field: "data".to_string(),
                    change_type: ChangeType::Preserve,
                    expected_value: serde_json::Value::Null,
                    confidence: 0.9,
                }]
            }
            _ => {
                vec![ActionSemanticEffect {
                    field: "state".to_string(),
                    change_type: ChangeType::Modify,
                    expected_value: action_request
                        .parameters
                        .get("expected_value")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                    confidence: 0.6,
                }]
            }
        };

        let expected_observation_format = match action_request.action_type {
            ActionType::InformationQuery => ExpectedObservationFormat::ExactMatch,
            ActionType::BeliefUpdate | ActionType::StateTransition => {
                let required_fields: Vec<String> =
                    semantic_effects.iter().map(|e| e.field.clone()).collect();
                ExpectedObservationFormat::PartialMatch { required_fields }
            }
            _ => ExpectedObservationFormat::PatternMatch {
                pattern: ".*".to_string(),
            },
        };

        let validity_window = match action_request.action_type {
            ActionType::StateTransition => ValidityWindow::new_steps_window(5),
            ActionType::InformationQuery => ValidityWindow::new_steps_window(20),
            _ => {
                let steps = hint.max_depth.unwrap_or(2) as i64 * 5;
                ValidityWindow::new_steps_window(steps.max(10))
            }
        };

        Ok(ParsedAction {
            action_type: action_request.action_type,
            target_node_id,
            semantic_effects,
            expected_observation_format,
            validity_window,
        })
    }

    async fn retrieve_context(
        &self,
        _action_request: &ActionRequest,
        parsed_action: &ParsedAction,
        hint: ContextHint,
    ) -> Result<ContextRetrievalResult, PredictionError> {
        let mut relevant_nodes = Vec::new();
        let mut visited_ids = HashSet::new();
        let mut all_edges = Vec::new();
        let mut precondition_node_ids = Vec::new();
        let mut missing_node_ids = Vec::new();

        let target_id = &parsed_action.target_node_id;

        if !target_id.is_empty() {
            if let Ok(Some(node)) = self.belief_graph.query_belief_by_id(target_id).await {
                relevant_nodes.push(node);
                visited_ids.insert(target_id.clone());
            }

            if let Ok(edges) = self.belief_graph.get_incoming_edges(target_id).await {
                for edge in &edges {
                    all_edges.push(edge.clone());
                    if edge.edge_type == "precondition" || edge.edge_type == "causal" {
                        precondition_node_ids.push(edge.source_node.clone());
                    }
                }
            }

            if let Ok(edges) = self.belief_graph.get_outgoing_edges(target_id).await {
                for edge in edges {
                    all_edges.push(edge.clone());
                    if !visited_ids.contains(&edge.target_node) {
                        if let Ok(Some(node)) = self
                            .belief_graph
                            .query_belief_by_id(&edge.target_node)
                            .await
                        {
                            if hint.max_depth.unwrap_or(2) >= 1 {
                                relevant_nodes.push(node);
                                visited_ids.insert(edge.target_node.clone());
                            }
                        }
                    }

                    if hint.max_depth.unwrap_or(2) >= 2 {
                        if let Ok(two_hop_edges) =
                            self.belief_graph.get_outgoing_edges(&edge.target_node).await
                        {
                            for two_hop_edge in two_hop_edges {
                                all_edges.push(two_hop_edge.clone());
                                if !visited_ids.contains(&two_hop_edge.target_node) {
                                    if let Ok(Some(node)) = self
                                        .belief_graph
                                        .query_belief_by_id(&two_hop_edge.target_node)
                                        .await
                                    {
                                        relevant_nodes.push(node);
                                        visited_ids.insert(two_hop_edge.target_node.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            for precond_id in &precondition_node_ids {
                if !visited_ids.contains(precond_id) {
                    if let Ok(Some(node)) =
                        self.belief_graph.query_belief_by_id(precond_id).await
                    {
                        relevant_nodes.push(node);
                        visited_ids.insert(precond_id.clone());
                    } else {
                        missing_node_ids.push(precond_id.clone());
                    }
                }
            }
        }

        let found_nodes = relevant_nodes.len() as f64;
        let missing_nodes = missing_node_ids.len() as f64;
        let completeness_score = if found_nodes + missing_nodes > 0.0 {
            found_nodes / (found_nodes + missing_nodes)
        } else {
            1.0
        };

        let mut sorted_nodes = relevant_nodes.clone();
        sorted_nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

        let mut hasher = DefaultHasher::new();
        for node in &sorted_nodes {
            node.node_id.hash(&mut hasher);
            node.node_type.hash(&mut hasher);
            node.confidence.to_bits().hash(&mut hasher);
            if let Some(obj) = node.attributes.as_object() {
                let mut keys: Vec<&String> = obj.keys().collect();
                keys.sort();
                for key in keys {
                    key.hash(&mut hasher);
                    obj[key].to_string().hash(&mut hasher);
                }
            }
        }
        let belief_state_hash = format!("{:016x}", hasher.finish());

        if completeness_score < 0.5 {
            let _ = self.event_publisher.publish_event(
                PBSMEvent::new(PredictionEvent::ContextIntegrityWarning(
                    ContextIntegrityWarningPayload {
                        prediction_id: Uuid::nil(),
                        missing_fields: missing_node_ids.clone(),
                        completeness_score,
                    },
                )),
            );
        }

        Ok(ContextRetrievalResult {
            target_beliefs: relevant_nodes,
            belief_state_hash,
            completeness_score,
            missing_node_ids,
            relation_edges: all_edges,
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
        parsed_action: &ParsedAction,
        context: &ContextRetrievalResult,
    ) -> Result<Vec<ExpectedChange>, PredictionError> {
        let mut expected_changes = Vec::new();

        let change_type = match action_request.action_type {
            ActionType::BeliefUpdate | ActionType::StateTransition => ChangeType::Modify,
            ActionType::InformationQuery => ChangeType::Preserve,
            _ => ChangeType::Modify,
        };

        if !parsed_action.target_node_id.is_empty() {
            let target_node = context
                .target_beliefs
                .iter()
                .find(|n| n.node_id == parsed_action.target_node_id);

            let previous_value = target_node
                .map(|n| n.attributes.clone())
                .unwrap_or(serde_json::Value::Null);

            let attribute = action_request
                .parameters
                .get("attribute")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    target_node
                        .and_then(|n| n.attributes.as_object())
                        .and_then(|obj| obj.keys().next().cloned())
                })
                .unwrap_or_else(|| "state".to_string());

            let target_edge_confidence = context
                .relation_edges
                .iter()
                .filter(|e| {
                    e.source_node == parsed_action.target_node_id
                        || e.target_node == parsed_action.target_node_id
                })
                .map(|e| e.confidence)
                .fold(0.0f64, |acc, c| acc.max(c));

            let edge_decay = if target_edge_confidence > 0.0 {
                target_edge_confidence
            } else {
                0.9
            };

            let expected_confidence = target_node
                .map(|n| n.confidence * edge_decay)
                .unwrap_or(0.5);

            let expected_value = self.derive_expected_value(action_request, &previous_value);

            expected_changes.push(ExpectedChange {
                change_id: Uuid::new_v4(),
                node_id: parsed_action.target_node_id.clone(),
                attribute: Some(attribute),
                expected_value,
                previous_value,
                change_type,
                expected_confidence,
                derivation_path: vec!["generator".to_string()],
            });

            for related_node in context
                .target_beliefs
                .iter()
                .filter(|n| n.node_id != parsed_action.target_node_id)
            {
                let related_previous = related_node.attributes.clone();
                let related_attribute = related_previous
                    .as_object()
                    .and_then(|obj| obj.keys().next().cloned())
                    .unwrap_or_else(|| "state".to_string());

                let related_edge_confidence = context
                    .relation_edges
                    .iter()
                    .filter(|e| {
                        (e.source_node == parsed_action.target_node_id
                            && e.target_node == related_node.node_id)
                            || (e.target_node == parsed_action.target_node_id
                                && e.source_node == related_node.node_id)
                    })
                    .map(|e| e.confidence)
                    .fold(0.0f64, |acc, c| acc.max(c));

                let related_decay = if related_edge_confidence > 0.0 {
                    related_edge_confidence
                } else {
                    0.7
                };

                let related_confidence = related_node.confidence * edge_decay * related_decay;

                expected_changes.push(ExpectedChange {
                    change_id: Uuid::new_v4(),
                    node_id: related_node.node_id.clone(),
                    attribute: Some(related_attribute),
                    expected_value: related_previous.clone(),
                    previous_value: related_previous,
                    change_type,
                    expected_confidence: related_confidence,
                    derivation_path: vec!["generator".to_string(), "related".to_string()],
                });
            }

            let cascade_edges: Vec<&RelationEdge> = context
                .relation_edges
                .iter()
                .filter(|e| e.edge_type == "causal" || e.edge_type == "correlational")
                .collect();

            for edge in &cascade_edges {
                let cascade_target_id = if edge.source_node == parsed_action.target_node_id {
                    &edge.target_node
                } else if edge.target_node == parsed_action.target_node_id {
                    &edge.source_node
                } else if context
                    .target_beliefs
                    .iter()
                    .any(|n| n.node_id == edge.source_node)
                {
                    &edge.target_node
                } else {
                    continue;
                };

                if expected_changes
                    .iter()
                    .any(|c| c.node_id == *cascade_target_id)
                {
                    continue;
                }

                if let Some(cascade_node) = context
                    .target_beliefs
                    .iter()
                    .find(|n| n.node_id == *cascade_target_id)
                {
                    let cascade_previous = cascade_node.attributes.clone();
                    let cascade_attribute = cascade_previous
                        .as_object()
                        .and_then(|obj| obj.keys().next().cloned())
                        .unwrap_or_else(|| "state".to_string());
                    let cascade_confidence = cascade_node.confidence * edge.confidence;

                    expected_changes.push(ExpectedChange {
                        change_id: Uuid::new_v4(),
                        node_id: cascade_node.node_id.clone(),
                        attribute: Some(cascade_attribute),
                        expected_value: cascade_previous.clone(),
                        previous_value: cascade_previous,
                        change_type: ChangeType::Modify,
                        expected_confidence: cascade_confidence,
                        derivation_path: vec![
                            "generator".to_string(),
                            "cascade".to_string(),
                            edge.edge_type.clone(),
                        ],
                    });
                }
            }

            match action_request.action_type {
                ActionType::StateTransition => {
                    for node in context
                        .target_beliefs
                        .iter()
                        .filter(|n| n.node_id != parsed_action.target_node_id)
                    {
                        if expected_changes.iter().any(|c| c.node_id == node.node_id) {
                            continue;
                        }
                        let side_previous = node.attributes.clone();
                        let side_attribute = side_previous
                            .as_object()
                            .and_then(|obj| obj.keys().next().cloned())
                            .unwrap_or_else(|| "state".to_string());
                        let side_confidence = node.confidence * 0.3;

                        expected_changes.push(ExpectedChange {
                            change_id: Uuid::new_v4(),
                            node_id: node.node_id.clone(),
                            attribute: Some(side_attribute),
                            expected_value: side_previous.clone(),
                            previous_value: side_previous,
                            change_type: ChangeType::Modify,
                            expected_confidence: side_confidence,
                            derivation_path: vec![
                                "generator".to_string(),
                                "side_effect".to_string(),
                            ],
                        });
                    }
                }
                ActionType::BeliefUpdate => {
                    for node in context
                        .target_beliefs
                        .iter()
                        .filter(|n| n.node_id != parsed_action.target_node_id)
                    {
                        if expected_changes.iter().any(|c| c.node_id == node.node_id) {
                            continue;
                        }
                        let side_previous = node.attributes.clone();
                        let side_attribute = side_previous
                            .as_object()
                            .and_then(|obj| obj.keys().next().cloned())
                            .unwrap_or_else(|| "state".to_string());
                        let side_confidence = node.confidence * 0.4;

                        expected_changes.push(ExpectedChange {
                            change_id: Uuid::new_v4(),
                            node_id: node.node_id.clone(),
                            attribute: Some(side_attribute),
                            expected_value: side_previous.clone(),
                            previous_value: side_previous,
                            change_type: ChangeType::Modify,
                            expected_confidence: side_confidence,
                            derivation_path: vec![
                                "generator".to_string(),
                                "side_effect".to_string(),
                            ],
                        });
                    }
                }
                _ => {}
            }
        }

        expected_changes.retain(|c| c.expected_confidence >= MIN_CONFIDENCE_THRESHOLD);

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
        if let Some(params) = action_request.parameters.as_object() {
            if let Some(expected) = params.get("expected_value") {
                return expected.clone();
            }
            if let Some(new_value) = params.get("new_value") {
                return new_value.clone();
            }
            if let Some(value) = params.get("value") {
                return value.clone();
            }
        }
        match action_request.action_type {
            ActionType::BeliefUpdate => {
                if previous_value.is_boolean() {
                    serde_json::json!(!previous_value.as_bool().unwrap_or(true))
                } else if previous_value.is_number() {
                    serde_json::json!(previous_value.as_f64().unwrap_or(0.0) * 1.1)
                } else {
                    previous_value.clone()
                }
            }
            ActionType::StateTransition => previous_value.clone(),
            ActionType::InformationQuery => previous_value.clone(),
            _ => previous_value.clone(),
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
            completeness_score: context.completeness_score,
        };

        let now = Utc::now();

        let initial_entry = StatusHistoryEntry {
            status: PredictionState::Pending,
            timestamp: now,
            reason: "Prediction created".to_string(),
            triggered_by: Some("M2".to_string()),
        };

        let prediction = Prediction {
            prediction_id,
            version: 1,
            associated_action,
            expected_changes,
            expected_observation,
            validity_window: self.default_validity_window.clone(),
            status: PredictionState::Pending,
            status_history: vec![initial_entry],
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
        };

        prediction.validate().map_err(|e| PredictionError::InvalidAction {
            message: e,
            code: "PEV-E003".to_string(),
        })?;

        Ok(prediction)
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
    pub completeness_score: f64,
    pub missing_node_ids: Vec<String>,
    pub relation_edges: Vec<RelationEdge>,
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

    #[test]
    fn test_parse_action_belief_update() {
        let generator = PredictionGenerator::default();
        let action = ActionRequest {
            action_type: ActionType::BeliefUpdate,
            action_name: "update_belief".to_string(),
            parameters: serde_json::json!({"attribute": "status", "expected_value": "active"}),
            target_id: Some("node-1".to_string()),
        };
        let hint = ContextHint::default();

        let result = generator.parse_action(&action, &hint);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.target_node_id, "node-1");
        assert_eq!(parsed.semantic_effects.len(), 1);
        assert_eq!(parsed.semantic_effects[0].field, "status");
    }

    #[test]
    fn test_parse_action_state_transition() {
        let generator = PredictionGenerator::default();
        let action = ActionRequest {
            action_type: ActionType::StateTransition,
            action_name: "transition".to_string(),
            parameters: serde_json::json!({"expected_value": "done"}),
            target_id: Some("node-2".to_string()),
        };
        let hint = ContextHint::default();

        let result = generator.parse_action(&action, &hint);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.semantic_effects[0].field, "state");
        assert_eq!(parsed.semantic_effects[0].change_type, ChangeType::Modify);
    }

    #[test]
    fn test_parse_action_information_query() {
        let generator = PredictionGenerator::default();
        let action = ActionRequest {
            action_type: ActionType::InformationQuery,
            action_name: "query".to_string(),
            parameters: serde_json::json!({}),
            target_id: Some("node-3".to_string()),
        };
        let hint = ContextHint::default();

        let result = generator.parse_action(&action, &hint);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.semantic_effects[0].change_type, ChangeType::Preserve);
    }

    #[tokio::test]
    async fn test_prediction_has_initial_pending_history() {
        let generator = PredictionGenerator::default();

        let action = ActionRequest {
            action_type: crate::types::prediction::ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({"expected_value": "ok"}),
            target_id: Some("node-1".to_string()),
        };

        let result = generator.create_prediction(action, None).await;
        assert!(result.is_ok());

        let prediction = result.unwrap();
        assert!(!prediction.status_history.is_empty());
        assert_eq!(prediction.status_history[0].status, PredictionState::Pending);
    }

    #[tokio::test]
    async fn test_completeness_score_in_context_snapshot() {
        let generator = PredictionGenerator::default();

        let action = ActionRequest {
            action_type: crate::types::prediction::ActionType::ToolCall,
            action_name: "test".to_string(),
            parameters: serde_json::json!({"expected_value": "ok"}),
            target_id: Some("node-1".to_string()),
        };

        let result = generator.create_prediction(action, None).await;
        assert!(result.is_ok());

        let prediction = result.unwrap();
        assert_eq!(prediction.context_snapshot.completeness_score, 1.0);
    }
}
