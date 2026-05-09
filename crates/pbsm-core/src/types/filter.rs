//! 预测过滤器和统计类型定义
//!
//! 本模块定义了用于预测查询和统计的数据结构。
//! PredictionFilter 用于条件筛选预测集合，PredictionStatistics 用于汇总分析预测数据。
//!
//! # 核心概念
//!
//! - **预测过滤器**：支持按状态、动作类型、目标节点、时间范围等多维度过滤
//! - **预测统计**：汇总预测的验证率、证伪率、平均残差等关键指标
//! - **取消原因**：记录预测被取消的具体原因
//!
//! # 使用场景
//!
//! - `PredictionEngine::get_active_predictions()` 使用过滤器查询预测
//! - `PredictionEngine::get_prediction_statistics()` 生成统计报告

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::prediction::PredictionState;

/// 预测过滤器结构体，用于条件筛选预测集合
///
/// 支持多维度组合过滤，包括状态、动作类型、目标节点、时间范围等。
/// 所有过滤条件为 AND 关系，即只有满足全部条件的预测才会被返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionFilter {
    /// 状态过滤列表，空列表表示不限制状态
    pub status: Vec<PredictionState>,
    /// 动作类型过滤，None 表示不限制
    pub associated_action: Option<String>,
    /// 目标节点过滤，None 表示不限制
    pub target_node: Option<String>,
    /// 时间范围过滤，None 表示不限制
    pub time_range: Option<TimeRange>,
    /// 最小严重程度过滤（预留字段）
    pub min_severity: Option<String>,
    /// 返回结果数量上限
    pub limit: Option<usize>,
    /// 返回结果偏移量，用于分页
    pub offset: Option<usize>,
}

impl Default for PredictionFilter {
    fn default() -> Self {
        Self {
            status: vec![PredictionState::Pending],
            associated_action: None,
            target_node: None,
            time_range: None,
            min_severity: None,
            limit: Some(100),
            offset: None,
        }
    }
}

impl PredictionFilter {
    /// 判断预测是否满足过滤条件
    ///
    /// # 参数
    /// * `prediction` - 待检查的预测实例
    ///
    /// # 返回
    /// * `true` - 满足所有过滤条件
    /// * `false` - 不满足任一过滤条件
    pub fn matches(&self, prediction: &crate::types::prediction::Prediction) -> bool {
        if !self.status.is_empty() && !self.status.contains(&prediction.status) {
            return false;
        }

        if let Some(ref action_type) = self.associated_action {
            let action_type_str = format!("{:?}", prediction.associated_action.action_type);
            if action_type_str != *action_type {
                return false;
            }
        }

        if let Some(ref target) = self.target_node {
            if prediction.associated_action.target_node.as_ref() != Some(target) {
                return false;
            }
        }

        if let Some(ref range) = self.time_range {
            if prediction.metadata.created_at < range.start
                || prediction.metadata.created_at > range.end
            {
                return false;
            }
        }

        true
    }
}

/// 时间范围结构体，定义查询的时间区间
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    /// 起始时间（包含）
    pub start: DateTime<Utc>,
    /// 结束时间（包含）
    pub end: DateTime<Utc>,
}

/// 错误模式结构体，记录高频出现的错误类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    /// 错误模式描述
    pub pattern: String,
    /// 出现次数
    pub count: u64,
}

/// 预测统计结构体，包含预测集合的汇总分析指标
///
/// 根据 HLD-M2-PredictionEngine.md 第 4 章，统计信息用于监控预测引擎的健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionStatistics {
    /// 预测总数
    pub total: u64,
    /// 按状态分布的预测数量
    pub by_status: HashMap<String, u64>,
    /// 按匹配级别分布的预测数量
    pub by_match_level: HashMap<String, u64>,
    /// 按严重程度分布的预测数量
    pub by_severity: HashMap<String, u64>,
    /// 平均残差程度
    pub average_residual: f64,
    /// 验证成功率（Verified / total）
    pub verification_rate: f64,
    /// 证伪率（Falsified / total）
    pub falsification_rate: f64,
    /// 平均验证延迟（毫秒）
    pub average_latency_ms: f64,
    /// 高频错误模式列表
    pub top_error_patterns: Vec<ErrorPattern>,
}

impl Default for PredictionStatistics {
    fn default() -> Self {
        Self {
            total: 0,
            by_status: HashMap::new(),
            by_match_level: HashMap::new(),
            by_severity: HashMap::new(),
            average_residual: 0.0,
            verification_rate: 0.0,
            falsification_rate: 0.0,
            average_latency_ms: 0.0,
            top_error_patterns: Vec::new(),
        }
    }
}

/// 残差趋势方向枚举
///
/// 用于描述残差随时间的变化趋势
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualTrend {
    /// 趋势方向
    pub trend: TrendDirection,
    /// 前一周期的平均残差
    pub previous_average: f64,
    /// 当前周期的平均残差
    pub current_average: f64,
}

/// 趋势方向枚举，描述指标的变化方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrendDirection {
    /// 改善：残差在下降
    Improving,
    /// 稳定：残差无显著变化
    Stable,
    /// 恶化：残差在上升
    Degrading,
}

impl TrendDirection {
    /// 根据前后变化判断趋势方向
    ///
    /// # 参数
    /// * `previous` - 前一周期值
    /// * `current` - 当前周期值
    ///
    /// # 返回
    /// * 趋势方向
    ///
    /// # 判断规则
    /// - current < previous - 0.1 → Improving
    /// - current > previous + 0.1 → Degrading
    /// - 否则 → Stable
    pub fn from_change(previous: f64, current: f64) -> Self {
        let diff = current - previous;
        if diff < -0.1 {
            Self::Improving
        } else if diff > 0.1 {
            Self::Degrading
        } else {
            Self::Stable
        }
    }
}

/// 注释结构体，用于标注残差历史中的关键点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    /// 注释时间戳
    pub timestamp: DateTime<Utc>,
    /// 注释内容
    pub note: String,
}

/// 残差历史结构体，记录单个预测的残差变化轨迹
///
/// 用于追踪预测验证的历史过程和支持可视化分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidualHistory {
    /// 关联的预测ID
    pub prediction_id: String,
    /// 残差计算时间点列表
    pub computed_at: Vec<DateTime<Utc>>,
    /// 残差趋势分析
    pub trend: ResidualTrend,
    /// 注释列表
    pub annotations: Vec<Annotation>,
}

impl Default for ResidualHistory {
    fn default() -> Self {
        Self {
            prediction_id: String::new(),
            computed_at: Vec::new(),
            trend: ResidualTrend {
                trend: TrendDirection::Stable,
                previous_average: 0.0,
                current_average: 0.0,
            },
            annotations: Vec::new(),
        }
    }
}

/// 预测列表结构体，包含分页信息的预测结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PredictionList {
    /// 预测实例列表
    pub predictions: Vec<crate::types::prediction::Prediction>,
    /// 符合条件的预测总数
    pub total: usize,
    /// 是否还有更多结果（用于分页）
    pub has_more: bool,
}

/// 取消原因枚举，记录预测被取消的具体原因
///
/// 根据 HLD-M2-PredictionEngine.md 第 2.3 节，取消原因用于日志和审计
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CancellationReason {
    /// 用户主动请求取消
    UserRequest,
    /// 前置条件不满足
    PrerequisiteFailed,
    /// 上下文发生重大变化
    ContextChanged,
    /// 动作冗余（重复执行相同操作）
    RedundantAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::prediction::Prediction;

    #[test]
    fn test_prediction_filter_default() {
        let filter = PredictionFilter::default();
        assert_eq!(filter.status, vec![PredictionState::Pending]);
        assert_eq!(filter.limit, Some(100));
    }

    #[test]
    fn test_prediction_filter_matches() {
        let mut prediction = Prediction::new();
        prediction.status = PredictionState::Pending;
        prediction.associated_action.action_type = crate::types::prediction::ActionType::ToolCall;

        let filter = PredictionFilter::default();
        assert!(filter.matches(&prediction));

        let mut status_filter = PredictionFilter::default();
        status_filter.status = vec![PredictionState::Verified];
        assert!(!status_filter.matches(&prediction));
    }

    #[test]
    fn test_trend_direction() {
        assert_eq!(
            TrendDirection::from_change(0.5, 0.3),
            TrendDirection::Improving
        );
        assert_eq!(
            TrendDirection::from_change(0.5, 0.5),
            TrendDirection::Stable
        );
        assert_eq!(
            TrendDirection::from_change(0.5, 0.7),
            TrendDirection::Degrading
        );
    }
}
