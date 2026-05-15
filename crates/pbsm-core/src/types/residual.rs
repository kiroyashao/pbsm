//! 预测残差数据类型定义
//!
//! 本模块定义了预测性信念状态机（PBSM）中预测残差的核心数据结构。
//! 根据架构设计中的"误差驱动原则"，预测残差是预测与实际观测之间偏差的结构化表示，
//! 是驱动信念状态动态更新与修正的核心信号。
//!
//! # 核心概念
//!
//! - **多维度残差**：从数值、语义、时序、结构四个维度综合计算预测与观测的偏差
//! - **匹配级别**：Exact（精确匹配）、Partial（部分匹配）、Mismatch（完全失配）
//! - **严重程度**：None、Warning、Error、Critical 四级评估
//! - **根因分析**：追溯导致预测失败的可能原因和传播路径
//!
//! # 残差计算公式
//!
//! 综合残差程度计算为：
//! R_overall = w_n * R_numerical + w_s * R_semantic + w_t * R_temporal + w_st * R_structural
//! 其中权重配置为：w_n = 0.3, w_s = 0.3, w_t = 0.2, w_st = 0.2
//!
//! # 架构位置
//!
//! 本模块与 types/prediction.rs 共同构成预测引擎的完整数据类型体系，
//! 为 PredictionVerifier 和 ResidualCalculator 提供数据结构支持

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 匹配级别枚举，描述预测与观测之间的匹配程度
///
/// 匹配级别决定了预测状态转换和后续处理流程
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum MatchLevel {
    /// 精确匹配：预测值与观测值完全一致或偏差几乎为零（< 0.001）
    #[default]
    Exact,
    /// 部分匹配：预测与观测存在一定偏差但在容忍范围内
    Partial,
    /// 完全失配：预测与观测完全相反或偏差超出容忍范围
    Mismatch,
}

impl MatchLevel {
    /// 根据综合残差程度判断匹配级别
    ///
    /// # 参数
    /// * `degree` - 综合残差程度值
    /// * `warning_threshold` - 警告阈值
    /// * `_error_threshold` - 错误阈值（当前未使用，预留扩展）
    ///
    /// # 返回
    /// * `Exact` - 残差几乎为零
    /// * `Partial` - 残差在警告阈值以内
    /// * `Mismatch` - 残差超出警告阈值
    pub fn from_overall_degree(degree: f64, warning_threshold: f64, _error_threshold: f64) -> Self {
        if degree.abs() < 0.001 {
            Self::Exact
        } else if degree.abs() <= warning_threshold {
            Self::Partial
        } else {
            Self::Mismatch
        }
    }
}

/// 数值维度残差结构体，计算数值类型属性的预测偏差
///
/// 数值维度残差通过归一化差值计算
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericalDimension {
    /// 是否已计算
    pub computed: bool,
    /// 归一化残差值，取值范围 [-1.0, 1.0]
    pub value: f64,
    /// 预期值
    pub expected: f64,
    /// 实际观测值
    pub actual: f64,
    /// 相对差值：(actual - expected) / expected
    pub relative_diff: f64,
    /// 绝对差值：(actual - expected).abs()
    pub absolute_diff: f64,
}

impl Default for NumericalDimension {
    fn default() -> Self {
        Self {
            computed: false,
            value: 0.0,
            expected: 0.0,
            actual: 0.0,
            relative_diff: 0.0,
            absolute_diff: 0.0,
        }
    }
}

impl NumericalDimension {
    /// 计算数值维度残差
    ///
    /// # 参数
    /// * `expected` - 预期值
    /// * `actual` - 实际观测值
    ///
    /// # 返回
    /// * 配置完整的 NumericalDimension 实例
    ///
    /// # 计算公式
    /// value = (actual - expected) / max(|expected|, 1)  // 归一化到 [-1, 1]
    /// absolute_diff = |actual - expected|
    /// relative_diff = |actual - expected| / |expected|  (expected != 0)
    pub fn compute(expected: f64, actual: f64) -> Self {
        let max_abs = expected.abs().max(1.0);
        let residual = (actual - expected) / max_abs;
        let abs_diff = (actual - expected).abs();
        let relative_diff = if expected != 0.0 {
            (actual - expected).abs() / expected.abs()
        } else {
            abs_diff
        };

        Self {
            computed: true,
            value: residual.clamp(-1.0, 1.0),
            expected,
            actual,
            absolute_diff: abs_diff,
            relative_diff,
        }
    }
}

/// 语义维度残差结构体，计算分类或标签类型属性的预测偏差
///
/// 语义维度使用知识图谱距离计算
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticDimension {
    /// 是否已计算
    pub computed: bool,
    /// 归一化语义距离，取值范围 [0.0, 1.0]
    pub value: f64,
    /// 类别是否匹配
    pub category_match: bool,
    /// 预期标签
    pub label_expected: String,
    /// 实际标签
    pub label_actual: String,
    /// 语义距离，通过知识图谱计算
    pub semantic_distance: f64,
}

impl Default for SemanticDimension {
    fn default() -> Self {
        Self {
            computed: false,
            value: 0.0,
            category_match: true,
            label_expected: String::new(),
            label_actual: String::new(),
            semantic_distance: 0.0,
        }
    }
}

impl SemanticDimension {
    /// 计算语义维度残差
    ///
    /// # 参数
    /// * `label_expected` - 预期标签
    /// * `label_actual` - 实际标签
    /// * `semantic_distance` - 预计算的语义距离
    ///
    /// # 返回
    /// * 配置完整的 SemanticDimension 实例
    pub fn compute(label_expected: &str, label_actual: &str, semantic_distance: f64) -> Self {
        let category_match = label_expected == label_actual;
        let max_distance = 10.0;

        Self {
            computed: true,
            value: (semantic_distance / max_distance).min(1.0),
            category_match,
            label_expected: label_expected.to_string(),
            label_actual: label_actual.to_string(),
            semantic_distance,
        }
    }

    /// 创建精确匹配的语义维度（用于测试或默认值）
    ///
    /// # 参数
    /// * `label` - 标签值
    ///
    /// # 返回
    /// * 完全匹配的语义维度实例
    pub fn exact_match(label: &str) -> Self {
        Self {
            computed: true,
            value: 0.0,
            category_match: true,
            label_expected: label.to_string(),
            label_actual: label.to_string(),
            semantic_distance: 0.0,
        }
    }
}

/// 时序维度残差结构体，计算时间相关属性的预测偏差
///
/// 时序维度考虑预期时间与实际时间的差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDimension {
    /// 是否已计算
    pub computed: bool,
    /// 归一化延迟比例，取值范围 [-1.0, 1.0]
    pub value: f64,
    /// 预期时间
    pub expected_time: DateTime<Utc>,
    /// 实际时间
    pub actual_time: DateTime<Utc>,
    /// 延迟比例：duration / expected_duration
    pub delay_ratio: f64,
}

impl Default for TemporalDimension {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            computed: false,
            value: 0.0,
            expected_time: now,
            actual_time: now,
            delay_ratio: 0.0,
        }
    }
}

impl TemporalDimension {
    /// 计算时序维度残差
    ///
    /// # 参数
    /// * `expected_time` - 预期发生时间
    /// * `actual_time` - 实际发生时间
    ///
    /// # 返回
    /// * 配置完整的 TemporalDimension 实例
    ///
    /// # 计算公式
    /// duration = (actual_time - expected_time).num_milliseconds()
    /// delay_ratio = duration / max(expected_duration_ms, 1)
    /// value = (delay_ratio).clamp(-1.0, 1.0)
    pub fn compute(expected_time: DateTime<Utc>, actual_time: DateTime<Utc>, expected_duration_ms: f64) -> Self {
        let duration = (actual_time - expected_time).num_milliseconds() as f64;
        let safe_duration = expected_duration_ms.max(1.0);

        Self {
            computed: true,
            value: (duration / safe_duration).clamp(-1.0, 1.0),
            expected_time,
            actual_time,
            delay_ratio: duration / safe_duration,
        }
    }

    /// 创建超时情况的时序维度（用于测试）
    ///
    /// # 返回
    /// * 表示超时的 TemporalDimension 实例
    pub fn timeout(expected_duration_ms: f64) -> Self {
        let now = Utc::now();
        let duration_ms = 60_000.0_f64;
        let safe_duration = expected_duration_ms.max(1.0);
        let delay_ratio = duration_ms / safe_duration;

        Self {
            computed: true,
            value: delay_ratio.clamp(-1.0, 1.0),
            expected_time: now - chrono::Duration::seconds(60),
            actual_time: now,
            delay_ratio,
        }
    }
}

/// 嵌套差异结构体，记录嵌套对象的路径和差异值
///
/// 用于 StructuralDimension 中记录复杂对象的逐层差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NestedDiff {
    /// 对象路径，JSONPath 格式
    pub path: String,
    /// 该路径处的差异值
    pub diff: f64,
}

/// 结构维度残差结构体，计算复杂对象结构属性的预测偏差
///
/// 结构维度分析预期字段与实际字段的差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralDimension {
    /// 是否已计算
    pub computed: bool,
    /// 归一化结构差异值，取值范围 [0.0, 1.0]
    pub value: f64,
    /// 预期字段列表
    pub expected_fields: Vec<String>,
    /// 实际字段列表
    pub actual_fields: Vec<String>,
    /// 缺失字段列表（预期中有但实际中没有）
    pub missing_fields: Vec<String>,
    /// 多余字段列表（实际中有但预期中没有）
    pub extra_fields: Vec<String>,
    /// 嵌套对象的逐层差异
    pub nested_diff: Vec<NestedDiff>,
}

impl Default for StructuralDimension {
    fn default() -> Self {
        Self {
            computed: false,
            value: 0.0,
            expected_fields: Vec::new(),
            actual_fields: Vec::new(),
            missing_fields: Vec::new(),
            extra_fields: Vec::new(),
            nested_diff: Vec::new(),
        }
    }
}

impl StructuralDimension {
    /// 计算结构维度残差
    ///
    /// # 参数
    /// * `expected_fields` - 预期字段列表
    /// * `actual_fields` - 实际字段列表
    /// * `nested_comparisons` - 嵌套比较的路径和差异值对
    ///
    /// # 返回
    /// * 配置完整的 StructuralDimension 实例
    ///
    /// # 计算公式
    /// missing_ratio = |missing| / max(|expected|, 1)
    /// extra_ratio = |extra| / max(|actual|, 1)
    /// value = 0.4 * missing_ratio + 0.2 * extra_ratio + 0.4 * nested_diff_avg
    pub fn compute(
        expected_fields: &[String],
        actual_fields: &[String],
        nested_comparisons: &[(String, f64)],
    ) -> Self {
        let expected_set: std::collections::HashSet<_> = expected_fields.iter().collect();
        let actual_set: std::collections::HashSet<_> = actual_fields.iter().collect();

        let missing: Vec<_> = expected_set.difference(&actual_set).collect();
        let extra: Vec<_> = actual_set.difference(&expected_set).collect();

        let missing_extra_ratio =
            (missing.len() + extra.len()) as f64 / expected_fields.len().max(1) as f64;

        let total_nested_diff: f64 = nested_comparisons.iter().map(|(_, d)| d).sum();
        let max_fields = expected_fields.len().max(1) as f64;
        let nested_diff_sum = total_nested_diff / max_fields;

        let residual = missing_extra_ratio + nested_diff_sum;

        Self {
            computed: true,
            value: residual.min(1.0),
            expected_fields: expected_fields.to_vec(),
            actual_fields: actual_fields.to_vec(),
            missing_fields: missing.into_iter().map(|s| (*s).to_string()).collect(),
            extra_fields: extra.into_iter().map(|s| (*s).to_string()).collect(),
            nested_diff: nested_comparisons
                .iter()
                .map(|(p, d)| NestedDiff {
                    path: p.clone(),
                    diff: *d,
                })
                .collect(),
        }
    }

    /// 创建精确匹配的结构维度（用于测试或默认值）
    ///
    /// # 参数
    /// * `fields` - 字段列表
    ///
    /// # 返回
    /// * 完全匹配的结构维度实例
    pub fn exact_match(fields: &[String]) -> Self {
        Self {
            computed: true,
            value: 0.0,
            expected_fields: fields.to_vec(),
            actual_fields: fields.to_vec(),
            missing_fields: Vec::new(),
            extra_fields: Vec::new(),
            nested_diff: Vec::new(),
        }
    }
}

/// 多维度残差容器结构体，整合数值、语义、时序、结构四个维度的残差
///
/// 残差计算需要在四个维度上综合评估
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResidualDimensions {
    /// 数值维度残差
    pub numerical: NumericalDimension,
    /// 语义维度残差
    pub semantic: SemanticDimension,
    /// 时序维度残差
    pub temporal: TemporalDimension,
    /// 结构维度残差
    pub structural: StructuralDimension,
}

impl ResidualDimensions {
    /// 计算综合残差程度
    ///
    /// # 参数
    /// * `weights` - 各维度权重配置
    ///
    /// # 返回
    /// * 综合残差程度值，取值范围 [0.0, 1.0]
    ///
    /// # 计算公式
    /// total = sum of (dimension.value.abs() * weight) for computed dimensions
    /// weight_sum = sum of weight for computed dimensions
    /// overall = total / weight_sum
    pub fn compute_overall(&self, weights: &DimensionWeights) -> f64 {
        let mut total = 0.0;

        if self.numerical.computed {
            total += self.numerical.value.abs() * weights.numerical;
        }
        if self.semantic.computed {
            total += self.semantic.value * weights.semantic;
        }
        if self.temporal.computed {
            total += self.temporal.value.abs() * weights.temporal;
        }
        if self.structural.computed {
            total += self.structural.value * weights.structural;
        }

        total
    }
}

/// 维度权重配置结构体，定义各维度在综合残差计算中的权重
///
/// 默认权重配置为：
/// 数值 0.3、语义 0.3、时序 0.2、结构 0.2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionWeights {
    /// 数值维度权重
    pub numerical: f64,
    /// 语义维度权重
    pub semantic: f64,
    /// 时序维度权重
    pub temporal: f64,
    /// 结构维度权重
    pub structural: f64,
}

impl Default for DimensionWeights {
    fn default() -> Self {
        Self {
            numerical: 0.3,
            semantic: 0.3,
            temporal: 0.2,
            structural: 0.2,
        }
    }
}

/// 分量残差结构体，记录单个预期变化的残差信息
///
/// 用于追溯具体是哪个预期变化导致了整体残差的产生
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentResidual {
    /// 关联的预期变化ID
    pub change_id: Uuid,
    /// 变化的节点ID
    pub node_id: String,
    /// 变化的属性名
    pub attribute: String,
    /// 该分量的匹配级别
    pub match_level: MatchLevel,
    /// 该分量的差异值
    pub diff_value: f64,
    /// 详细的差异信息（JSON 格式）
    pub diff_details: serde_json::Value,
}

/// 严重程度级别枚举，描述残差的严重程度
///
/// 严重程度决定后续处理流程
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum SeverityLevel {
    /// 无偏差：预测与观测完全一致
    #[default]
    None,
    /// 警告级别：存在轻微偏差，但不影响主要结论
    Warning,
    /// 错误级别：存在明显偏差，需要关注和修正
    Error,
    /// 严重级别：存在重大偏差，需要立即回滚或重做
    Critical,
}

impl SeverityLevel {
    /// 根据残差程度判断严重级别
    ///
    /// # 参数
    /// * `degree` - 残差程度值
    /// * `warning_threshold` - 警告阈值
    /// * `error_threshold` - 错误阈值
    ///
    /// # 返回
    /// * 对应的 SeverityLevel
    ///
    /// # 判断规则
    /// - |degree| < 0.001 → None
    /// - warning_threshold < |degree| ≤ error_threshold → Error
    /// - |degree| > error_threshold → Critical
    pub fn from_degree(degree: f64, warning_threshold: f64, error_threshold: f64, tolerance_margin: f64) -> Self {
        let abs_degree = degree.abs();
        if abs_degree < 0.001 {
            Self::None
        } else if abs_degree <= warning_threshold + tolerance_margin {
            Self::Warning
        } else if abs_degree <= error_threshold + tolerance_margin {
            Self::Error
        } else {
            Self::Critical
        }
    }
}

/// 严重程度阈值配置结构体，定义各严重级别判定阈值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeverityThreshold {
    /// 警告阈值（低于此值判定为 Warning）
    pub warning: f64,
    /// 错误阈值（高于此值判定为 Critical）
    pub error: f64,
    /// 严重阈值上限（固定为 1.0）
    pub critical: f64,
    pub tolerance_margin: f64,
}

impl Default for SeverityThreshold {
    fn default() -> Self {
        Self {
            warning: 0.3,
            error: 0.7,
            critical: 1.0,
            tolerance_margin: 0.05,
        }
    }
}

/// 严重程度评估结果结构体，包含评估的完整信息
///
/// 这是预测验证时判断偏差严重程度的核心数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeverityAssessment {
    /// 严重级别
    pub level: SeverityLevel,
    /// 评分（残差程度 × 100），取值范围 [0, 100]
    pub score: f64,
    /// 使用的阈值配置
    pub threshold: SeverityThreshold,
    /// 触发评估的组件标识
    pub triggered_by: String,
}

impl SeverityAssessment {
    /// 执行严重程度评估
    ///
    /// # 参数
    /// * `degree` - 残差程度值
    /// * `threshold` - 判定阈值配置
    ///
    /// # 返回
    /// * 评估结果
    pub fn assess(degree: f64, threshold: &SeverityThreshold, dimensions: &ResidualDimensions) -> Self {
        let mut level = SeverityLevel::from_degree(
            degree,
            threshold.warning,
            threshold.error,
            threshold.tolerance_margin,
        );

        if dimensions.numerical.computed && dimensions.numerical.value.abs() >= 1.0
            || dimensions.semantic.computed && dimensions.semantic.value >= 1.0
            || dimensions.temporal.computed && dimensions.temporal.value >= 1.0
            || dimensions.structural.computed && dimensions.structural.value >= 1.0
        {
            level = SeverityLevel::Critical;
        }

        if dimensions.structural.computed
            && dimensions.structural.value >= 0.5
            && matches!(level, SeverityLevel::None | SeverityLevel::Warning)
        {
            level = SeverityLevel::Error;
        }

        if dimensions.semantic.computed
            && dimensions.semantic.value >= 1.0
            && !dimensions.semantic.category_match
            && matches!(level, SeverityLevel::None | SeverityLevel::Warning)
        {
            level = SeverityLevel::Error;
        }

        Self {
            level,
            score: degree.abs() * 100.0,
            threshold: threshold.clone(),
            triggered_by: "M2".to_string(),
        }
    }
}

/// 根因分析结构体，记录预测失败的可能原因分析
///
/// 根因分析支持追溯错误传播路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseAnalysis {
    /// 假设原因描述
    pub hypothesis: String,
    /// 假设置信度
    pub confidence: f64,
    /// 支持证据列表
    pub evidence: Vec<String>,
    /// 与该假设矛盾的其他信念
    pub contradicts_beliefs: Vec<String>,
    /// 错误传播深度
    pub propagation_depth: u32,
}

impl Default for RootCauseAnalysis {
    fn default() -> Self {
        Self {
            hypothesis: String::new(),
            confidence: 0.0,
            evidence: Vec::new(),
            contradicts_beliefs: Vec::new(),
            propagation_depth: 0,
        }
    }
}

/// 残差元数据结构体，包含残差计算的管理信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualMetadata {
    /// 残差计算时间
    pub computed_at: DateTime<Utc>,
    /// 收到观测结果的时间
    pub observation_received_at: DateTime<Utc>,
    /// 计算延迟（毫秒）
    pub latency_ms: i64,
    /// 执行计算的组件标识
    pub validator: String,
}

impl ResidualMetadata {
    /// 创建残差元数据
    ///
    /// # 参数
    /// * `observation_time` - 收到观测结果的时间
    ///
    /// # 返回
    /// * 新的 ResidualMetadata 实例
    pub fn new(observation_time: DateTime<Utc>) -> Self {
        let now = Utc::now();
        Self {
            computed_at: now,
            observation_received_at: observation_time,
            latency_ms: (now - observation_time).num_milliseconds(),
            validator: "M2".to_string(),
        }
    }
}

/// 残差主结构体，是预测与观测之间偏差的完整表示
///
/// 残差包含：
/// - 基本信息（ID、关联预测ID）
/// - 匹配级别和综合残差程度
/// - 四维度残差详情
/// - 分量残差列表
/// - 严重程度评估
/// - 根因分析
/// - 元数据
///
/// # 架构意义
///
/// 残差是整个 PBSM 架构的核心驱动力信号。系统利用残差追溯信念图中的受影响节点，
/// 定位需要修正的旧假设，并阻止基于错误信念继续决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Residual {
    /// 残差唯一标识符
    pub residual_id: Uuid,
    /// 关联的预测ID
    pub prediction_id: Uuid,
    /// 匹配级别
    pub match_level: MatchLevel,
    /// 综合残差程度，取值范围 [0.0, 1.0]
    pub overall_degree: f64,
    /// 四维度残差详情
    pub dimensions: ResidualDimensions,
    /// 单个预期变化对应的分量残差列表
    pub component_residuals: Vec<ComponentResidual>,
    /// 严重程度评估结果
    pub severity_assessment: SeverityAssessment,
    /// 根因分析结果
    pub root_cause_analysis: RootCauseAnalysis,
    /// 元数据信息
    pub metadata: ResidualMetadata,
}

impl Residual {
    /// 创建新的残差实例
    ///
    /// # 参数
    /// * `prediction_id` - 关联的预测ID
    /// * `observation_time` - 收到观测结果的时间
    ///
    /// # 返回
    /// * 初始化为默认状态的 Residual 实例
    pub fn new(prediction_id: Uuid, observation_time: DateTime<Utc>) -> Self {
        Self {
            residual_id: Uuid::new_v4(),
            prediction_id,
            match_level: MatchLevel::Exact,
            overall_degree: 0.0,
            dimensions: ResidualDimensions::default(),
            component_residuals: Vec::new(),
            severity_assessment: SeverityAssessment {
                level: SeverityLevel::None,
                score: 0.0,
                threshold: SeverityThreshold::default(),
                triggered_by: "M2".to_string(),
            },
            root_cause_analysis: RootCauseAnalysis::default(),
            metadata: ResidualMetadata::new(observation_time),
        }
    }

    /// 计算匹配级别和严重程度
    ///
    /// # 参数
    /// * `threshold` - 判定阈值配置
    ///
    /// # 说明
    ///
    /// 在完成残差计算后调用此方法，根据综合残差程度更新匹配级别和严重程度评估
    pub fn compute_match_level(&mut self, threshold: &SeverityThreshold) {
        self.match_level = MatchLevel::from_overall_degree(
            self.overall_degree,
            threshold.warning,
            threshold.error,
        );
        self.severity_assessment =
            SeverityAssessment::assess(self.overall_degree, threshold, &self.dimensions);
    }

    pub fn populate_root_cause(&mut self) {
        if self.component_residuals.is_empty() {
            return;
        }

        let max_diff = self
            .component_residuals
            .iter()
            .map(|c| c.diff_value)
            .fold(0.0_f64, f64::max);

        let high_diff_threshold = max_diff * 0.8;
        let high_diff_components: Vec<_> = self
            .component_residuals
            .iter()
            .filter(|c| c.diff_value >= high_diff_threshold)
            .collect();

        let hypothesis = high_diff_components
            .iter()
            .map(|c| format!("{}:{}", c.node_id, c.attribute))
            .collect::<Vec<_>>()
            .join(",");

        let evidence: Vec<String> = high_diff_components
            .iter()
            .map(|c| format!("diff={:.3} on {}.{}", c.diff_value, c.node_id, c.attribute))
            .collect();

        let confidence = if max_diff >= 1.0 {
            0.95
        } else {
            max_diff * 0.9
        };

        self.root_cause_analysis = RootCauseAnalysis {
            hypothesis: if hypothesis.is_empty() {
                "unknown".to_string()
            } else {
                hypothesis
            },
            confidence,
            evidence,
            contradicts_beliefs: Vec::new(),
            propagation_depth: high_diff_components.len() as u32,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numerical_dimension_computation() {
        let dim = NumericalDimension::compute(100.0, 110.0);
        assert!(dim.computed);
        assert!((dim.value - 0.1).abs() < 0.001);
        assert!((dim.relative_diff - 0.1).abs() < 0.001);
        assert!((dim.absolute_diff - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_semantic_dimension_exact_match() {
        let dim = SemanticDimension::exact_match("success");
        assert!(dim.computed);
        assert_eq!(dim.value, 0.0);
        assert!(dim.category_match);
    }

    #[test]
    fn test_match_level_from_degree() {
        assert_eq!(
            MatchLevel::from_overall_degree(0.0, 0.3, 0.7),
            MatchLevel::Exact
        );
        assert_eq!(
            MatchLevel::from_overall_degree(0.2, 0.3, 0.7),
            MatchLevel::Partial
        );
        assert_eq!(
            MatchLevel::from_overall_degree(0.8, 0.3, 0.7),
            MatchLevel::Mismatch
        );
    }

    #[test]
    fn test_severity_level_from_degree() {
        assert_eq!(
            SeverityLevel::from_degree(0.0, 0.3, 0.7, 0.05),
            SeverityLevel::None
        );
        assert_eq!(
            SeverityLevel::from_degree(0.2, 0.3, 0.7, 0.05),
            SeverityLevel::Warning
        );
        assert_eq!(
            SeverityLevel::from_degree(0.5, 0.3, 0.7, 0.05),
            SeverityLevel::Error
        );
        assert_eq!(
            SeverityLevel::from_degree(0.9, 0.3, 0.7, 0.05),
            SeverityLevel::Critical
        );
    }

    #[test]
    fn test_severity_level_tolerance_margin() {
        assert_eq!(
            SeverityLevel::from_degree(0.32, 0.3, 0.7, 0.05),
            SeverityLevel::Warning
        );
        assert_eq!(
            SeverityLevel::from_degree(0.72, 0.3, 0.7, 0.05),
            SeverityLevel::Error
        );
        assert_eq!(
            SeverityLevel::from_degree(0.32, 0.3, 0.7, 0.0),
            SeverityLevel::Error
        );
        assert_eq!(
            SeverityLevel::from_degree(0.72, 0.3, 0.7, 0.0),
            SeverityLevel::Critical
        );
    }

    #[test]
    fn test_residual_creation() {
        let prediction_id = Uuid::new_v4();
        let now = Utc::now();
        let residual = Residual::new(prediction_id, now);
        assert_eq!(residual.prediction_id, prediction_id);
        assert_eq!(residual.match_level, MatchLevel::Exact);
        assert_eq!(residual.overall_degree, 0.0);
    }

    #[test]
    fn test_compute_overall_direct_weighted_sum() {
        let mut dims = ResidualDimensions::default();
        dims.numerical = NumericalDimension::compute(100.0, 110.0);
        dims.semantic = SemanticDimension::exact_match("ok");
        let weights = DimensionWeights::default();

        let overall = dims.compute_overall(&weights);
        let expected = dims.numerical.value.abs() * weights.numerical
            + dims.semantic.value * weights.semantic;
        assert!((overall - expected).abs() < 0.001);
    }

    #[test]
    fn test_compute_overall_uncomputed_contributes_zero() {
        let mut dims = ResidualDimensions::default();
        dims.numerical = NumericalDimension::compute(100.0, 110.0);
        let weights = DimensionWeights::default();

        let overall = dims.compute_overall(&weights);
        let expected = dims.numerical.value.abs() * weights.numerical;
        assert!((overall - expected).abs() < 0.001);
    }

    #[test]
    fn test_temporal_dimension_with_expected_duration() {
        let now = Utc::now();
        let expected = now - chrono::Duration::milliseconds(500);
        let dim = TemporalDimension::compute(expected, now, 1000.0);
        assert!(dim.computed);
        assert!((dim.value - 0.5).abs() < 0.001);
        assert!((dim.delay_ratio - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_temporal_timeout_with_duration() {
        let dim = TemporalDimension::timeout(60_000.0);
        assert!(dim.computed);
        assert!((dim.value - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_structural_dimension_hld_formula() {
        let expected_fields = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let actual_fields = vec!["a".to_string(), "d".to_string()];
        let nested: Vec<(String, f64)> = vec![("x".to_string(), 0.3)];

        let dim = StructuralDimension::compute(&expected_fields, &actual_fields, &nested);
        assert!(dim.computed);

        let missing_extra_ratio = 3.0_f64 / 3.0;
        let nested_sum = 0.3 / 3.0;
        let expected_value = missing_extra_ratio + nested_sum;
        assert!((dim.value - expected_value.min(1.0)).abs() < 0.001);
    }

    #[test]
    fn test_severity_auto_promotion_critical() {
        let threshold = SeverityThreshold::default();
        let mut dims = ResidualDimensions::default();
        dims.numerical = NumericalDimension::compute(1.0, 100.0);

        let assessment = SeverityAssessment::assess(0.2, &threshold, &dims);
        assert_eq!(assessment.level, SeverityLevel::Critical);
    }

    #[test]
    fn test_severity_auto_promotion_structural_error() {
        let threshold = SeverityThreshold::default();
        let mut dims = ResidualDimensions::default();
        dims.structural = StructuralDimension {
            computed: true,
            value: 0.6,
            expected_fields: vec!["a".to_string(), "b".to_string()],
            actual_fields: vec!["a".to_string()],
            missing_fields: vec!["b".to_string()],
            extra_fields: vec![],
            nested_diff: vec![],
        };

        let assessment = SeverityAssessment::assess(0.1, &threshold, &dims);
        assert_eq!(assessment.level, SeverityLevel::Error);
    }

    #[test]
    fn test_severity_auto_promotion_semantic_contradiction() {
        let threshold = SeverityThreshold::default();
        let mut dims = ResidualDimensions::default();
        dims.semantic = SemanticDimension {
            computed: true,
            value: 1.0,
            category_match: false,
            label_expected: "success".to_string(),
            label_actual: "failed".to_string(),
            semantic_distance: 10.0,
        };

        let assessment = SeverityAssessment::assess(0.1, &threshold, &dims);
        assert_eq!(assessment.level, SeverityLevel::Critical);
    }

    #[test]
    fn test_populate_root_cause() {
        let prediction_id = Uuid::new_v4();
        let now = Utc::now();
        let mut residual = Residual::new(prediction_id, now);

        residual.component_residuals.push(ComponentResidual {
            change_id: Uuid::new_v4(),
            node_id: "node-1".to_string(),
            attribute: "status".to_string(),
            match_level: MatchLevel::Mismatch,
            diff_value: 0.9,
            diff_details: serde_json::json!({}),
        });
        residual.component_residuals.push(ComponentResidual {
            change_id: Uuid::new_v4(),
            node_id: "node-2".to_string(),
            attribute: "count".to_string(),
            match_level: MatchLevel::Partial,
            diff_value: 0.3,
            diff_details: serde_json::json!({}),
        });

        residual.populate_root_cause();

        assert!(!residual.root_cause_analysis.hypothesis.is_empty());
        assert!(residual.root_cause_analysis.confidence > 0.0);
        assert!(!residual.root_cause_analysis.evidence.is_empty());
        assert_eq!(residual.root_cause_analysis.propagation_depth, 1);
    }

    #[test]
    fn test_populate_root_cause_empty() {
        let prediction_id = Uuid::new_v4();
        let now = Utc::now();
        let mut residual = Residual::new(prediction_id, now);

        residual.populate_root_cause();

        assert!(residual.root_cause_analysis.hypothesis.is_empty());
        assert_eq!(residual.root_cause_analysis.confidence, 0.0);
    }
}
