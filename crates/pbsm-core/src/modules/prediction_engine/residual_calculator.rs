//! 残差计算器实现
//!
//! 本模块实现了预测残差的核心计算逻辑。
//! 残差是预测与实际观测之间的偏差，是驱动信念修正的核心机制。
//!
//! # 核心职责
//!
//! - 计算预测值与实际观测值之间的多维度残差
//! - 支持数值、语义、时间、结构和组件五个维度的残差计算
//! - 根据残差结果进行匹配级别判定和严重程度评估
//!
//! # 残差维度说明
//!
//! 1. **数值维度**：适用于数值型属性，计算相对误差
//! 2. **语义维度**：适用于字符串型属性，计算语义距离
//! 3. **时间维度**：记录观测相对于预测的时间偏差
//! 4. **结构维度**：比较预期字段与实际字段的差异
//! 5. **组件维度**：逐个属性计算残差，用于精确问题定位

use std::collections::HashMap;
use uuid::Uuid;

use crate::types::prediction::{ChangeType, ExpectedChange, Observation};
use crate::types::residual::{
    ComponentResidual, DimensionWeights, MatchLevel, NumericalDimension, Residual,
    ResidualDimensions, SemanticDimension, SeverityAssessment, SeverityThreshold,
    StructuralDimension, TemporalDimension,
};

/// 残差计算器结构体，负责计算预测残差
///
/// # 设计说明
///
/// 残差计算器封装了所有残差相关的计算逻辑：
/// - 使用配置化的阈值和权重
/// - 维护知识图谱用于语义距离计算
/// - 支持多种数据类型的残差计算
pub struct ResidualCalculator {
    thresholds: SeverityThreshold,
    weights: DimensionWeights,
    knowledge_graph: HashMap<String, Vec<String>>,
}

impl ResidualCalculator {
    pub fn new() -> Self {
        Self {
            thresholds: SeverityThreshold::default(),
            weights: DimensionWeights::default(),
            knowledge_graph: HashMap::new(),
        }
    }

    /// 使用指定的阈值和权重创建残差计算器
    ///
    /// # 参数
    /// * `thresholds` - 严重程度阈值配置
    /// * `weights` - 各维度权重配置
    ///
    /// # 返回
    /// * 配置好的 ResidualCalculator 实例
    pub fn with_config(thresholds: SeverityThreshold, weights: DimensionWeights) -> Self {
        Self {
            thresholds,
            weights,
            knowledge_graph: HashMap::new(),
        }
    }

    /// 设置知识图谱，用于语义距离计算
    ///
    /// # 参数
    /// * `graph` - 知识图谱，键为词汇，值为相关词汇列表
    pub fn set_knowledge_graph(&mut self, graph: HashMap<String, Vec<String>>) {
        self.knowledge_graph = graph;
    }

    /// 计算预测残差（主入口）
    ///
    /// # 参数
    /// * `prediction_id` - 预测ID
    /// * `expected_changes` - 预期变化列表
    /// * `observation` - 实际观测结果
    ///
    /// # 返回
    /// * Residual 结构体，包含所有维度的残差计算结果
    ///
    /// # 计算流程
    ///
    /// 1. 对每个预期变化计算组件残差
    /// 2. 计算数值、语义、结构三个维度的残差
    /// 3. 计算时间维度残差
    /// 4. 综合各维度计算总体残差程度
    /// 5. 根据阈值判定匹配级别
    pub fn compute_residual(
        &self,
        prediction_id: Uuid,
        expected_changes: &[ExpectedChange],
        observation: &Observation,
    ) -> Residual {
        let mut residual = Residual::new(prediction_id, observation.timestamp);
        let mut component_residuals = Vec::new();

        for change in expected_changes {
            let actual_value = self.extract_value(&observation.data, &change.attribute);
            let component = self.compute_component_residual(change, actual_value);
            component_residuals.push(component);
        }

        let (numerical, semantic, structural) =
            self.compute_value_residuals(expected_changes, &observation.data);

        let temporal = TemporalDimension::compute(
            observation.timestamp - chrono::Duration::seconds(1),
            observation.timestamp,
        );

        residual.dimensions = ResidualDimensions {
            numerical,
            semantic,
            temporal,
            structural,
        };

        residual.component_residuals = component_residuals;
        residual.overall_degree = residual.dimensions.compute_overall(&self.weights);
        residual.compute_match_level(&self.thresholds);

        residual
    }

    /// 从观测数据中提取指定属性的值
    ///
    /// # 参数
    /// * `data` - 观测数据（JSON格式）
    /// * `attribute` - 属性名称（可选）
    ///
    /// # 返回
    /// * 提取到的值，如果属性不存在则返回 Null
    fn extract_value<'a>(
        &self,
        data: &'a serde_json::Value,
        attribute: &Option<String>,
    ) -> &'a serde_json::Value {
        match attribute {
            Some(attr) => data.get(attr).unwrap_or(&serde_json::Value::Null),
            _ => data,
        }
    }

    /// 计算单个预期变化的组件残差
    ///
    /// # 参数
    /// * `change` - 预期变化
    /// * `actual_value` - 实际观测值
    ///
    /// # 返回
    /// * ComponentResidual 结构体
    fn compute_component_residual(
        &self,
        change: &ExpectedChange,
        actual_value: &serde_json::Value,
    ) -> ComponentResidual {
        let diff_value = self.compute_json_diff(&change.expected_value, actual_value);
        let match_level = self.determine_match_level_single(diff_value);

        ComponentResidual {
            change_id: change.change_id,
            node_id: change.node_id.clone(),
            attribute: change.attribute.clone().unwrap_or_default(),
            match_level,
            diff_value,
            diff_details: serde_json::json!({
                "expected": change.expected_value,
                "actual": actual_value,
                "change_type": change.change_type,
            }),
        }
    }

    /// 计算两个JSON值之间的差异
    ///
    /// # 参数
    /// * `expected` - 预期值
    /// * `actual` - 实际值
    ///
    /// # 返回
    /// * 差异值，范围 [0.0, 1.0]，0表示完全匹配
    ///
    /// # 计算规则
    ///
    /// - 数值类型：计算相对误差
    /// - 字符串类型：计算语义距离
    /// - 布尔类型：相等为0，否则为1
    /// - Null值：根据变更类型判定
    fn compute_json_diff(&self, expected: &serde_json::Value, actual: &serde_json::Value) -> f64 {
        match (expected, actual) {
            (serde_json::Value::Number(e), serde_json::Value::Number(a)) => {
                let e_f = e.as_f64().unwrap_or(0.0);
                let a_f = a.as_f64().unwrap_or(0.0);
                if e_f == 0.0 {
                    (a_f - e_f).abs().min(1.0)
                } else {
                    ((a_f - e_f) / e_f).abs().min(1.0)
                }
            }
            (serde_json::Value::String(e), serde_json::Value::String(a)) => {
                if e == a {
                    0.0
                } else {
                    self.compute_semantic_distance(e, a).min(1.0)
                }
            }
            (serde_json::Value::Bool(e), serde_json::Value::Bool(a)) => {
                if e == a {
                    0.0
                } else {
                    1.0
                }
            }
            (serde_json::Value::Null, serde_json::Value::Null) => 0.0,
            (serde_json::Value::Null, _) => 1.0,
            (_, serde_json::Value::Null) => match self.determine_change_type(expected) {
                ChangeType::Add => 0.0,
                ChangeType::Remove => 1.0,
                _ => 1.0,
            },
            _ => {
                if expected == actual {
                    0.0
                } else {
                    1.0
                }
            }
        }
    }

    /// 根据值的内容判断变更类型
    ///
    /// # 参数
    /// * `value` - JSON值
    ///
    /// # 返回
    /// * ChangeType 枚举值
    fn determine_change_type(&self, value: &serde_json::Value) -> ChangeType {
        match value {
            serde_json::Value::Null => ChangeType::Remove,
            _ => ChangeType::Modify,
        }
    }

    /// 计算两个字符串之间的语义距离
    ///
    /// # 参数
    /// * `expected` - 预期字符串
    /// * `actual` - 实际字符串
    ///
    /// # 返回
    /// * 语义距离，范围 [0.0, 1.0]
    ///
    /// # 算法说明
    ///
    /// 1. 如果字符串相等，距离为0
    /// 2. 查找两个字符串在知识图谱中的邻居
    /// 3. 如果存在交集，距离为0.5（表示语义相关）
    /// 4. 否则距离为1.0（表示语义无关）
    fn compute_semantic_distance(&self, expected: &str, actual: &str) -> f64 {
        if expected == actual {
            return 0.0;
        }

        let from_neighbors = self.knowledge_graph.get(expected);
        let to_neighbors = self.knowledge_graph.get(actual);

        match (from_neighbors, to_neighbors) {
            (Some(f), Some(t)) => {
                let intersection: Vec<_> = f.iter().filter(|n| t.contains(n)).collect();
                if !intersection.is_empty() {
                    0.5
                } else {
                    1.0
                }
            }
            _ => 1.0,
        }
    }

    /// 计算数值、语义、结构三个维度的残差
    ///
    /// # 参数
    /// * `expected_changes` - 预期变化列表
    /// * `observation_data` - 观测数据
    ///
    /// # 返回
    /// * (数值维度, 语义维度, 结构维度) 元组
    fn compute_value_residuals(
        &self,
        expected_changes: &[ExpectedChange],
        observation_data: &serde_json::Value,
    ) -> (NumericalDimension, SemanticDimension, StructuralDimension) {
        let mut numerical_values = Vec::new();
        let mut semantic_values = Vec::new();
        let mut expected_fields = Vec::new();
        let mut actual_fields = Vec::new();

        if let Some(obj) = observation_data.as_object() {
            for key in obj.keys() {
                actual_fields.push(key.clone());
            }
        }

        for change in expected_changes {
            if let Some(attr) = &change.attribute {
                expected_fields.push(attr.clone());

                let actual = observation_data
                    .get(attr)
                    .unwrap_or(&serde_json::Value::Null);

                if let (Some(e_num), Some(a_num)) =
                    (change.expected_value.as_f64(), actual.as_f64())
                {
                    numerical_values.push((e_num, a_num));
                } else if let (Some(e_str), Some(a_str)) =
                    (change.expected_value.as_str(), actual.as_str())
                {
                    let distance = self.compute_semantic_distance(e_str, a_str);
                    semantic_values.push((e_str.to_string(), a_str.to_string(), distance));
                }
            }
        }

        let numerical = if !numerical_values.is_empty() {
            let (total_e, total_a) = numerical_values
                .iter()
                .fold((0.0, 0.0), |(e, a), (ve, va)| (e + ve, a + va));
            let count = numerical_values.len() as f64;
            NumericalDimension::compute(total_e / count, total_a / count)
        } else {
            NumericalDimension::default()
        };

        let semantic = if !semantic_values.is_empty() {
            let avg_distance: f64 = semantic_values.iter().map(|(_, _, d)| d).sum::<f64>()
                / semantic_values.len() as f64;
            SemanticDimension::compute(&semantic_values[0].0, &semantic_values[0].1, avg_distance)
        } else {
            SemanticDimension::default()
        };

        let nested: Vec<(String, f64)> = Vec::new();
        let structural = StructuralDimension::compute(&expected_fields, &actual_fields, &nested);

        (numerical, semantic, structural)
    }

    /// 根据残差判定匹配级别
    ///
    /// # 参数
    /// * `residual` - 残差结构体引用
    ///
    /// # 返回
    /// * MatchLevel 枚举值
    ///
    /// # 判定规则
    ///
    /// - |degree| < 0.001 → Exact（完全匹配）
    /// - |degree| <= warning阈值 → Partial（部分匹配）
    /// - |degree| > warning阈值 → Mismatch（不匹配）
    pub fn determine_match_level(residual: &Residual) -> MatchLevel {
        let degree = residual.overall_degree;
        let threshold = &residual.severity_assessment.threshold;

        if degree.abs() < 0.001 {
            MatchLevel::Exact
        } else if degree.abs() <= threshold.warning {
            MatchLevel::Partial
        } else {
            MatchLevel::Mismatch
        }
    }

    /// 根据单个差异值判定匹配级别
    ///
    /// # 参数
    /// * `diff_value` - 差异值
    ///
    /// # 返回
    /// * MatchLevel 枚举值
    fn determine_match_level_single(&self, diff_value: f64) -> MatchLevel {
        if diff_value < 0.001 {
            MatchLevel::Exact
        } else if diff_value <= self.thresholds.warning {
            MatchLevel::Partial
        } else {
            MatchLevel::Mismatch
        }
    }

    /// 评估残差的严重程度
    ///
    /// # 参数
    /// * `overall_degree` - 综合残差程度
    /// * `threshold` - 阈值配置
    ///
    /// # 返回
    /// * SeverityAssessment 结构体
    pub fn assess_severity(
        overall_degree: f64,
        threshold: &SeverityThreshold,
    ) -> SeverityAssessment {
        SeverityAssessment::assess(overall_degree, threshold)
    }
}

impl Default for ResidualCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::prediction::ActionRequest;
    use crate::types::residual::SeverityLevel;
    use chrono::Utc;

    fn create_test_calculator() -> ResidualCalculator {
        let mut calc = ResidualCalculator::new();
        let mut graph = HashMap::new();
        graph.insert(
            "success".to_string(),
            vec!["completed".to_string(), "done".to_string()],
        );
        graph.insert(
            "failed".to_string(),
            vec!["error".to_string(), "error_state".to_string()],
        );
        calc.set_knowledge_graph(graph);
        calc
    }

    #[test]
    fn test_exact_match_numerical() {
        let calc = create_test_calculator();
        let prediction_id = Uuid::new_v4();
        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"count": 10, "status": "success"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let expected = vec![
            crate::types::prediction::ExpectedChange {
                change_id: Uuid::new_v4(),
                node_id: "test-node".to_string(),
                attribute: Some("count".to_string()),
                expected_value: serde_json::json!(10),
                previous_value: serde_json::json!(5),
                change_type: ChangeType::Modify,
                expected_confidence: 0.9,
                derivation_path: vec![],
            },
            crate::types::prediction::ExpectedChange {
                change_id: Uuid::new_v4(),
                node_id: "test-node".to_string(),
                attribute: Some("status".to_string()),
                expected_value: serde_json::json!("success"),
                previous_value: serde_json::json!("pending"),
                change_type: ChangeType::Modify,
                expected_confidence: 0.9,
                derivation_path: vec![],
            },
        ];

        let residual = calc.compute_residual(prediction_id, &expected, &observation);
        assert!(residual.dimensions.numerical.computed);
        assert_eq!(residual.dimensions.numerical.value, 0.0);
        assert!(residual.overall_degree < 0.4);
    }

    #[test]
    fn test_partial_match_numerical() {
        let calc = create_test_calculator();
        let prediction_id = Uuid::new_v4();
        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"count": 12, "status": "success"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let expected = vec![
            crate::types::prediction::ExpectedChange {
                change_id: Uuid::new_v4(),
                node_id: "test-node".to_string(),
                attribute: Some("count".to_string()),
                expected_value: serde_json::json!(10),
                previous_value: serde_json::json!(5),
                change_type: ChangeType::Modify,
                expected_confidence: 0.9,
                derivation_path: vec![],
            },
            crate::types::prediction::ExpectedChange {
                change_id: Uuid::new_v4(),
                node_id: "test-node".to_string(),
                attribute: Some("status".to_string()),
                expected_value: serde_json::json!("success"),
                previous_value: serde_json::json!("pending"),
                change_type: ChangeType::Modify,
                expected_confidence: 0.9,
                derivation_path: vec![],
            },
        ];

        let residual = calc.compute_residual(prediction_id, &expected, &observation);
        assert!(residual.overall_degree > 0.0);
        assert!(residual.overall_degree <= 0.6);
    }

    #[test]
    fn test_mismatch_semantic() {
        let calc = create_test_calculator();
        let prediction_id = Uuid::new_v4();
        let observation = Observation {
            format: "json".to_string(),
            data: serde_json::json!({"status": "failed"}),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };

        let expected = vec![crate::types::prediction::ExpectedChange {
            change_id: Uuid::new_v4(),
            node_id: "test-node".to_string(),
            attribute: Some("status".to_string()),
            expected_value: serde_json::json!("success"),
            previous_value: serde_json::json!("pending"),
            change_type: ChangeType::Modify,
            expected_confidence: 0.9,
            derivation_path: vec![],
        }];

        let residual = calc.compute_residual(prediction_id, &expected, &observation);
        assert!(residual.overall_degree > 0.0);
    }

    #[test]
    fn test_structural_diff() {
        let calc = ResidualCalculator::new();
        let expected_fields = vec!["id".to_string(), "name".to_string(), "value".to_string()];
        let actual_fields = vec!["id".to_string(), "name".to_string(), "extra".to_string()];
        let nested: Vec<(String, f64)> = vec![("nested.field".to_string(), 0.2)];

        let structural = StructuralDimension::compute(&expected_fields, &actual_fields, &nested);
        assert!(structural.computed);
        assert_eq!(structural.missing_fields, vec!["value"]);
        assert_eq!(structural.extra_fields, vec!["extra"]);
    }

    #[test]
    fn test_severity_assessment() {
        let threshold = SeverityThreshold::default();
        assert_eq!(
            ResidualCalculator::assess_severity(0.0, &threshold).level,
            SeverityLevel::None
        );
    }
}
