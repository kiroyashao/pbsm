//! 预测引擎核心数据类型定义
//!
//! 本模块定义了预测性信念状态机（PBSM）中预测引擎（M2）的核心数据结构。根据架构设计中的"预测先于行动"原则，
//! 每次动作执行前必须生成对该动作结果的显式预测。预测数据结构包含预期变化、观测格式定义、有效期窗口等完整信息。
//!
//! # 核心概念
//!
//! - **预测状态机**：预测从创建到终结的完整生命周期，包括 Pending、Verified、Falsified、Expired、Cancelled 五种状态
//! - **预测残差**：预测与实际观测之间的偏差，是驱动信念修正的核心信号
//! - **有效期窗口**：预测的有效期限定义，支持时间、步骤数和事件驱动三种模式
//!
//! # 架构位置
//!
//! 本模块位于系统的推理控制层，是预测引擎（M2）与信念图管理器（M1）、元认知控制器（M3）交互的数据基础。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 预测状态枚举，定义预测对象从创建到终结的完整生命周期
///
/// # 状态转换规则
///
/// - `Pending` 是唯一允许转换到其他所有状态的起始状态
/// - `Verified`、`Falsified`、`Expired`、`Cancelled` 均为终态，不可再转换
///
/// # 架构意义
///
/// 根据 HLD-M2-PredictionEngine.md 第 2.3 节，预测状态机体现了"误差驱动原则"：
/// 只有在预测与实际观测出现偏差时才触发深层信念修正，完全一致的观测则被忽略细节
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum PredictionState {
    /// 待验证状态：预测已创建并关联动作已执行，等待观测结果返回
    #[default]
    Pending,
    /// 已证实状态：预测与观测完全一致或偏差在容忍范围内（终态）
    Verified,
    /// 已证伪状态：预测与观测存在不可容忍的偏差（终态）
    Falsified,
    /// 已失效状态：预测因超时或环境变化而失效（终态）
    Expired,
    /// 已取消状态：预测因动作取消或前置条件失败而失效（终态）
    Cancelled,
}

impl PredictionState {
    /// 判断当前状态是否可以转换到目标状态
    ///
    /// # 参数
    /// * `target` - 目标状态
    ///
    /// # 返回
    /// * `true` - 可以转换
    /// * `false` - 不可以转换
    ///
    /// # 状态转换矩阵
    ///
    /// 只有 `Pending` 状态可以转换到其他状态，其他状态均为终态
    pub fn can_transition_to(&self, _target: PredictionState) -> bool {
        matches!(self, PredictionState::Pending)
    }

    /// 判断当前状态是否为终态
    ///
    /// # 返回
    /// * `true` - 是终态，不能再转换到其他状态
    /// * `false` - 不是终态，可以继续转换
    pub fn is_terminal(&self) -> bool {
        let result = matches!(
            self,
            PredictionState::Verified
                | PredictionState::Falsified
                | PredictionState::Expired
                | PredictionState::Cancelled
        );
        result
    }
}

/// 动作类型枚举，定义可以生成预测的动作类别
///
/// 根据架构设计，动作类型决定了预测生成时的上下文检索范围和影响分析方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    /// 工具调用动作：通过工具接口执行的外部操作，如文件操作、API调用等
    ToolCall,
    /// 用户消息动作：来自用户的输入或指令
    UserMessage,
    /// 内部推理动作：Agent内部的推理或计算过程
    InternalInference,
}

/// 变化类型枚举，描述预期状态变化的类别
///
/// 在预测生成阶段，系统分析动作效果后标记预期的变化类型，用于残差计算时的语义分析
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChangeType {
    /// 新增：目标节点或属性被添加到信念图中
    Add,
    /// 移除：目标节点或属性从信念图中删除
    Remove,
    /// 修改：目标节点或属性的值发生变更
    Modify,
    /// 保留：预期该值保持不变，用于验证环境稳定性
    Preserve,
    /// 未知：无法确定变化类型，需要在残差计算时进行额外分析
    Unknown,
}

/// 关联动作结构体，记录触发预测的具体动作信息
///
/// 根据"预测先于行动"原则，每个预测必须关联一个具体的待执行动作。
/// 此结构体在预测生成时创建，用于后续的预测验证和残差追溯
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssociatedAction {
    /// 动作唯一标识符，用于关联预测与执行记录
    pub action_id: Uuid,
    /// 动作类型，决定上下文检索和影响分析的方式
    pub action_type: ActionType,
    /// 动作名称，可读的动作标识如 "unlock_file"
    pub action_name: String,
    /// 动作参数字典，包含执行该动作所需的所有参数
    pub parameters: serde_json::Value,
    /// 目标节点标识符，指向信念图中该动作的主要作用对象
    pub target_node: Option<String>,
    /// 受影响节点列表，记录该动作可能影响的所有信念节点
    pub affected_nodes: Vec<String>,
}

impl Default for AssociatedAction {
    fn default() -> Self {
        Self {
            action_id: Uuid::new_v4(),
            action_type: ActionType::InternalInference,
            action_name: String::new(),
            parameters: serde_json::Value::Null,
            target_node: None,
            affected_nodes: Vec::new(),
        }
    }
}

/// 预期变化结构体，描述动作执行后预期发生的单个状态变更
///
/// 在预测生成阶段，影响分析模块会生成一个或多个预期变化，用于后续与实际观测比对
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedChange {
    /// 变化唯一标识符，用于残差计算时的分量追溯
    pub change_id: Uuid,
    /// 变化的节点标识符，指向信念图中的目标节点
    pub node_id: String,
    /// 变化的属性名，如果为 None 则表示节点级别的变化
    pub attribute: Option<String>,
    /// 预期值，动作成功后该属性应具有的值
    pub expected_value: serde_json::Value,
    /// 前值，变化前该属性的原值，用于计算变化幅度
    pub previous_value: serde_json::Value,
    /// 变化类型，描述变化的性质
    pub change_type: ChangeType,
    /// 预期置信度，该预测的置信度初值，验证成功后可提升
    pub expected_confidence: f64,
    /// 推导路径，记录该预期值是如何得出的，用于错误追溯
    pub derivation_path: Vec<String>,
}

impl Default for ExpectedChange {
    fn default() -> Self {
        Self {
            change_id: Uuid::new_v4(),
            node_id: String::new(),
            attribute: None,
            expected_value: serde_json::Value::Null,
            previous_value: serde_json::Value::Null,
            change_type: ChangeType::Unknown,
            expected_confidence: 0.5,
            derivation_path: Vec::new(),
        }
    }
}

/// 字段映射结构体，定义观测数据字段与信念节点的对应关系
///
/// 在预测生成时，系统会建立从观测格式到信念属性的映射，便于验证阶段提取和比对
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    /// 输出字段名，观测数据中的字段名
    pub output_field: String,
    /// 对应的信念节点ID
    pub maps_to_node: String,
    /// 对应的信念属性名
    pub maps_to_attribute: String,
}

/// 提取提示结构体，提供从原始观测中提取结构化数据的辅助信息
///
/// 这些提示用于处理非标准格式的观测响应，提高预测验证的鲁棒性
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionHints {
    /// JSONPath 表达式，用于从 JSON 结构中提取目标字段
    pub json_path: Option<String>,
    /// 正则表达式，用于从字符串中匹配目标内容
    pub regex_pattern: Option<String>,
    /// 分隔符，用于解析分隔符格式的数据
    pub delimiter: Option<String>,
}

/// 预期观测结构体，定义预测验证时期望观察到的观测格式
///
/// 根据 HLD-M2-PredictionEngine.md 第 2.1 节，预期观测包含格式定义和字段映射，
/// 用于指导观测结果的解析和残差计算
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedObservation {
    /// 观测数据格式，如 "json"、"xml"、"text" 等
    pub format: String,
    /// 样本值，可选的典型观测值示例，用于验证格式解析的正确性
    pub sample_value: Option<serde_json::Value>,
    /// 字段映射列表，建立从观测字段到信念属性的对应关系
    pub field_mappings: Vec<FieldMapping>,
    /// 提取提示，提供解析非标准格式的辅助信息
    pub extraction_hints: ExtractionHints,
}

impl Default for ExpectedObservation {
    fn default() -> Self {
        Self {
            format: "json".to_string(),
            sample_value: None,
            field_mappings: Vec::new(),
            extraction_hints: ExtractionHints::default(),
        }
    }
}

/// 有效期类型枚举，定义预测有效期的计量方式
///
/// 根据架构设计，预测必须在有效期内得到验证，逾期则自动失效
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum ValidityType {
    /// 时间驱动：指定毫秒数后过期
    Time,
    /// 步骤驱动：指定步数后过期，适用于异步或多阶段操作
    #[default]
    Steps,
    /// 事件驱动：由特定事件触发失效，如收到特定信号
    EventBased,
}

/// 有效期窗口结构体，定义预测的有效期限和超时行为
///
/// 根据 HLD-M2-PredictionEngine.md 第 2.1 节，预测残差只在有效期内有效计算，
/// 逾期未验证的预测自动转为 Expired 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidityWindow {
    /// 有效期类型，决定过期判断的计量方式
    pub validity_type: ValidityType,
    /// 持续时间，Time 类型时为毫秒数，Steps 类型时为步数
    pub duration_ms: i64,
    /// 截止时间，Time 类型时的绝对时间戳，Steps/EventBased 类型时为 None
    pub deadline: Option<DateTime<Utc>>,
    /// 最大重试次数，EventBased 类型时允许的重试上限
    pub max_retries: u32,
}

impl ValidityWindow {
    /// 创建基于时间窗口的新有效期
    ///
    /// # 参数
    /// * `duration_ms` - 有效期时长（毫秒）
    ///
    /// # 返回
    /// * `ValidityWindow` - 配置好的有效期窗口，截止时间为当前时间 + duration_ms
    pub fn new_time_window(duration_ms: i64) -> Self {
        let deadline = Utc::now() + chrono::Duration::milliseconds(duration_ms);
        Self {
            validity_type: ValidityType::Time,
            duration_ms,
            deadline: Some(deadline),
            max_retries: 0,
        }
    }

    /// 创建基于步骤窗口的新有效期
    ///
    /// # 参数
    /// * `steps` - 有效步数
    ///
    /// # 返回
    /// * `ValidityWindow` - 配置好的有效期窗口，无绝对截止时间
    pub fn new_steps_window(steps: i64) -> Self {
        Self {
            validity_type: ValidityType::Steps,
            duration_ms: steps,
            deadline: None,
            max_retries: 0,
        }
    }

    /// 检查当前有效期是否已过期
    ///
    /// # 返回
    /// * `true` - 已过期
    /// * `false` - 未过期或无过期概念（如 Steps 类型）
    pub fn is_expired(&self) -> bool {
        match self.deadline {
            Some(deadline) => Utc::now() > deadline,
            None => false,
        }
    }

    /// 获取剩余有效期（毫秒）
    ///
    /// # 返回
    /// 剩余毫秒数，如果已过期返回 0
    pub fn remaining_ms(&self) -> i64 {
        self.deadline
            .map(|d| (d - Utc::now()).num_milliseconds())
            .unwrap_or(0)
            .max(0)
    }
}

impl Default for ValidityWindow {
    fn default() -> Self {
        Self::new_steps_window(10)
    }
}

/// 状态历史条目结构体，记录预测状态的每次转换
///
/// 此结构体用于追溯预测的生命周期，支持错误诊断和审计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusHistoryEntry {
    /// 状态转换后的状态
    pub status: PredictionState,
    /// 状态转换发生的时间戳
    pub timestamp: DateTime<Utc>,
    /// 状态转换的原因描述
    pub reason: String,
    /// 触发状态转换的组件标识，如 "M2" 表示由预测引擎触发
    pub triggered_by: Option<String>,
}

impl StatusHistoryEntry {
    /// 创建新的状态历史条目
    ///
    /// # 参数
    /// * `status` - 状态值
    /// * `reason` - 转换原因
    /// * `triggered_by` - 触发者标识
    ///
    /// # 返回
    /// * 新的 StatusHistoryEntry 实例
    pub fn new(status: PredictionState, reason: &str, triggered_by: Option<String>) -> Self {
        Self {
            status,
            timestamp: Utc::now(),
            reason: reason.to_string(),
            triggered_by,
        }
    }
}

/// 上下文快照结构体，记录预测生成时的信念状态上下文
///
/// 根据 HLD-M2-PredictionEngine.md 第 2.1 节，预测生成时会记录相关信念节点的快照，
/// 用于验证时对比上下文是否发生显著变化
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextSnapshot {
    /// 信念状态哈希值，用于快速检测上下文是否发生实质性变化
    pub belief_state_hash: String,
    /// 相关信念节点ID列表，预测生成时涉及的所有信念节点
    pub relevant_nodes: Vec<String>,
    /// 意图层级，预测所属的意图栈层级（0 表示顶层意图）
    pub intention_level: u32,
}

/// 预测元数据结构体，包含预测的管理信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionMetadata {
    /// 预测创建时间
    pub created_at: DateTime<Utc>,
    /// 创建者标识，通常为模块标识如 "M2"
    pub created_by: String,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
    /// 验证成功时间，仅在状态为 Verified 时有值
    pub verified_at: Option<DateTime<Utc>>,
    /// 预测置信度初始值，取值范围 [0.0, 1.0]
    pub confidence: f64,
    /// 标签列表，用于分类和检索
    pub tags: Vec<String>,
}

impl Default for PredictionMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            created_by: "M2".to_string(),
            updated_at: now,
            verified_at: None,
            confidence: 0.5,
            tags: Vec::new(),
        }
    }
}

/// 预测主结构体，是预测引擎的核心数据结构
///
/// 根据 HLD-M2-PredictionEngine.md 第 2.1 节，预测包含：
/// - 关联动作信息
/// - 预期变化列表
/// - 预期观测格式
/// - 有效期窗口
/// - 当前状态及状态历史
/// - 残差计算结果
/// - 上下文快照
///
/// # 使用流程
///
/// 1. 通过 `PredictionGenerator::create_prediction()` 创建预测
/// 2. 执行关联的动作
/// 3. 通过 `PredictionVerifier::verify_prediction()` 验证观测
/// 4. 系统根据验证结果更新状态和驱动信念修正
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// 预测唯一标识符
    pub prediction_id: Uuid,
    /// 版本号，用于乐观并发控制
    pub version: u32,
    /// 关联动作的完整描述
    pub associated_action: AssociatedAction,
    /// 预期变化列表，至少包含一项
    pub expected_changes: Vec<ExpectedChange>,
    /// 预期观测格式定义
    pub expected_observation: ExpectedObservation,
    /// 有效期窗口定义
    pub validity_window: ValidityWindow,
    /// 当前状态
    pub status: PredictionState,
    /// 状态转换历史
    pub status_history: Vec<StatusHistoryEntry>,
    /// 残差计算结果，初始为 None，验证后填充
    pub residuals: Option<super::residual::Residual>,
    /// 预测生成时的上下文快照
    pub context_snapshot: ContextSnapshot,
    /// 元数据信息
    pub metadata: PredictionMetadata,
}

impl Default for Prediction {
    fn default() -> Self {
        Self {
            prediction_id: Uuid::new_v4(),
            version: 1,
            associated_action: AssociatedAction::default(),
            expected_changes: Vec::new(),
            expected_observation: ExpectedObservation::default(),
            validity_window: ValidityWindow::default(),
            status: PredictionState::Pending,
            status_history: Vec::new(),
            residuals: None,
            context_snapshot: ContextSnapshot::default(),
            metadata: PredictionMetadata::default(),
        }
    }
}

impl Prediction {
    /// 创建新的预测实例
    ///
    /// # 返回
    /// * 初始化为默认状态的 Prediction 实例
    pub fn new() -> Self {
        Self::default()
    }

    /// 执行状态转换
    ///
    /// # 参数
    /// * `new_state` - 目标状态
    /// * `reason` - 转换原因
    ///
    /// # 返回
    /// * `Ok(())` - 转换成功
    /// * `Err(PredictionError::StateTransition)` - 状态转换非法
    pub fn transition_to(
        &mut self,
        new_state: PredictionState,
        reason: &str,
    ) -> Result<(), crate::error::PredictionError> {
        if !self.status.can_transition_to(new_state) {
            return Err(crate::error::PredictionError::StateTransition {
                reason: format!(
                    "Cannot transition from {:?} to {:?}",
                    self.status, new_state
                ),
            });
        }

        let entry = StatusHistoryEntry::new(new_state, reason, Some("M2".to_string()));
        self.status_history.push(entry);
        self.status = new_state;
        self.metadata.updated_at = Utc::now();

        if new_state == PredictionState::Verified {
            self.metadata.verified_at = Some(Utc::now());
        }

        Ok(())
    }

    /// 重置预测为初始状态
    ///
    /// 通常在对象池回收预测对象时调用，将预测重置为默认状态以便复用
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 动作请求结构体，用于向预测引擎发起创建预测的请求
///
/// 这是从外部（如推理引擎）向预测引擎输入的主要数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    /// 动作类型
    pub action_type: ActionType,
    /// 动作名称
    pub action_name: String,
    /// 动作参数
    pub parameters: serde_json::Value,
    /// 目标节点标识符
    pub target_id: Option<String>,
}

impl ActionRequest {
    /// 将动作请求转换为关联动作结构体
    ///
    /// # 返回
    /// * 配置好的 AssociatedAction 实例
    ///
    /// # 说明
    ///
    /// 此转换在预测生成流水线中调用，生成完整的关联动作信息
    pub fn into_associated_action(self) -> AssociatedAction {
        AssociatedAction {
            action_id: Uuid::new_v4(),
            action_type: self.action_type,
            action_name: self.action_name,
            parameters: self.parameters,
            target_node: self.target_id.clone(),
            affected_nodes: self.target_id.map(|id| vec![id]).unwrap_or_default(),
        }
    }
}

/// 观测结构体，表示动作执行后返回的实际结果
///
/// 根据"预测先于行动"原则，观测是预测验证的输入，用于计算预测残差
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// 观测数据格式
    pub format: String,
    /// 观测数据内容，结构化数据格式
    pub data: serde_json::Value,
    /// 观测发生的时间戳
    pub timestamp: DateTime<Utc>,
    /// 观测来源标识，如 "tool_response"、"user_input" 等
    pub source: String,
}

impl Default for Observation {
    fn default() -> Self {
        Self {
            format: "json".to_string(),
            data: serde_json::Value::Null,
            timestamp: Utc::now(),
            source: String::new(),
        }
    }
}

/// 下一步动作枚举，定义预测验证后的建议操作
///
/// 根据 HLD-M2-PredictionEngine.md 第 3.2 节，验证结果会触发不同的后续处理流程
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum NextAction {
    /// 确认：预测验证成功，维持当前信念状态
    Confirm,
    /// 修正：预测出现偏差，需要修正相关信念
    Revise,
    /// 回滚：预测严重失败，需要回滚到之前的信念状态
    Rollback,
    /// 记录：仅记录偏差，不触发信念修正
    #[default]
    Log,
}

/// 验证结果结构体，包含预测验证的完整输出
///
/// 这是 `PredictionVerifier::verify_prediction()` 的返回类型，包含匹配级别、
/// 残差详情、建议的下一步动作等信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// 被验证的预测ID
    pub prediction_id: Uuid,
    /// 匹配级别（精确匹配、部分匹配、失配）
    pub match_level: super::residual::MatchLevel,
    /// 完整的残差计算结果
    pub residual: super::residual::Residual,
    /// 验证过程受影响的信念节点ID列表
    pub affected_beliefs: Vec<String>,
    /// 建议的下一步动作
    pub next_action: NextAction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prediction_state_transitions() {
        let pending = PredictionState::Pending;
        assert!(pending.can_transition_to(PredictionState::Verified));
        assert!(pending.can_transition_to(PredictionState::Falsified));
        assert!(pending.can_transition_to(PredictionState::Expired));
        assert!(pending.can_transition_to(PredictionState::Cancelled));

        let verified = PredictionState::Verified;
        assert!(!verified.can_transition_to(PredictionState::Pending));
        assert!(!verified.can_transition_to(PredictionState::Falsified));
        assert!(verified.is_terminal());
    }

    #[test]
    fn test_validity_window_expiry() {
        let window = ValidityWindow::new_time_window(100);
        assert!(!window.is_expired());
        assert!(window.remaining_ms() <= 100);

        let expired_window = ValidityWindow {
            validity_type: ValidityType::Time,
            duration_ms: 0,
            deadline: Some(Utc::now() - chrono::Duration::seconds(1)),
            max_retries: 0,
        };
        assert!(expired_window.is_expired());
    }

    #[test]
    fn test_prediction_creation() {
        let mut prediction = Prediction::new();
        assert_eq!(prediction.status, PredictionState::Pending);

        prediction
            .transition_to(PredictionState::Verified, "Test verified")
            .unwrap();
        assert_eq!(prediction.status, PredictionState::Verified);
        assert!(prediction.metadata.verified_at.is_some());

        let result = prediction.transition_to(PredictionState::Falsified, "Should fail");
        assert!(result.is_err());
    }
}
