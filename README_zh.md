# PBSM - 预测信念状态机

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/Python-3.13+-blue?logo=python)](https://www.python.org/)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-545+%20Rust%20%7C%2059+%20Python%20comprehensive-brightgreen)]()

> 一种"预测先于行动"的认知架构，通过验证与反馈闭环持续精炼信念。

---
- [英文文档](README.md)

## 为什么需要 PBSM？

### 问题背景

当前的 AI Agent 系统普遍采用 **"先执行后观察"**（Act-then-Observe）的范式：Agent 调用工具、获取结果、再决定下一步。这种模式存在根本性缺陷：

1. **无法预判后果**：Agent 在执行操作前没有对结果的预期，无法提前发现潜在问题。例如，一个代码修改 Agent 可能在执行 `rm -rf` 之前没有预测到文件将被永久删除。

2. **缺乏自我认知**：系统不知道自己"不知道什么"。当注意力过度集中（Excessive Focus）、意图漂移（Drift）或行为振荡（Oscillation）时，系统无法自我检测和纠正。

3. **信念无结构**：传统 Agent 用扁平的上下文窗口（Context Window）存储信息，缺乏结构化的知识表示。信息之间没有因果、时序、层级关系，导致推理能力受限。

4. **记忆不可靠**：没有分层记忆机制（短期/长期/经验），Agent 无法从过去的成功和失败中学习，每次交互都从零开始。

5. **任务规划脆弱**：简单的任务队列无法处理层级目标、意图漂移和检查点恢复。一旦某步失败，整个任务链崩溃。

PBSM 的核心洞察来自认知科学：**人类在行动前会先预测结果，如果预测与实际不符，就会修正信念并调整行为**。PBSM 将这一认知闭环形式化为可计算的架构。

### PBSM 的解决方案

PBSM 引入了 **"预测→验证→修正"**（Predict → Verify → Correct）的闭环范式：

| 传统 Agent | PBSM Agent |
|-----------|------------|
| 执行 → 观察 → 反应 | 预测 → 执行 → 验证 → 修正 |
| 无预期，事后补救 | 有预期，事前预防 |
| 扁平上下文 | 结构化信念图（Belief Graph） |
| 无自我监控 | 元认知（注意力/遗忘/异常检测） |
| 简单任务队列 | 层级意图栈 + 检查点恢复 |
| 无长期记忆 | 三层记忆（原始日志/快照/经验） |
| 单 Agent | 多 Agent 通信 + 冲突协商 |

**核心差异**：PBSM 的 Agent 在每次行动前都会生成一个结构化的预测（Prediction），描述"我预期会发生什么"。执行后，验证器（Verifier）计算预测与实际的残差（Residual），残差驱动信念更新和行为调整。这使得 Agent 具备了：

- **预见性**：在行动前评估风险和预期收益
- **可解释性**：每个决策都有预测作为依据，可以追溯"为什么这样做"
- **自适应性**：残差反馈驱动持续学习，信念随经验不断精炼
- **鲁棒性**：元认知自动检测异常模式（振荡、漂移、过度聚焦）并触发干预

---

## 设计哲学

### 为什么是"预测先于行动"？

PBSM 的核心范式受以下理论启发：

- **预测编码（Predictive Coding）**：神经科学理论认为，大脑不是被动接收信息，而是持续预测感官输入，用预测误差（Prediction Error）驱动学习。PBSM 的残差计算正是这一理论的工程实现。

- **主动推理（Active Inference）**：Friston 的自由能原理指出，智能系统通过最小化预测误差来行动。PBSM 的闭环设计（预测→验证→修正）直接对应这一原则。

- **信念修正（Belief Revision）**：认知科学中的信念修正理论提供了形式化框架——当新证据与旧信念冲突时，应以最小代价修正信念。PBSM 的置信度调整（验证通过 +0.05，验证失败 -0.15）体现了非对称修正策略。

### 为什么用信念图而不是知识图谱？

| 特性 | 知识图谱 | 信念图 |
|------|---------|--------|
| 核心语义 | "X 是真的" | "X 的置信度为 0.85" |
| 不确定性 | 通常不处理 | 一等公民（confidence 字段） |
| 时变性 | 静态为主 | 动态（创建/更新时间戳 + 有效性窗口） |
| 可遗忘 | 无此概念 | 内建遗忘机制（低价值信念自动淘汰） |
| 快照回滚 | 不支持 | 支持（Snapshot + Checkpoint） |
| 预测关联 | 无 | 边可关联预测，验证后更新置信度 |

信念图的核心优势：**每个节点和边都携带置信度**，使得系统可以量化"有多确定"，而不仅仅是"知道什么"。当预测被验证或证伪时，相关信念的置信度会自动调整——这是传统知识图谱无法做到的。

### 为什么内建元认知？

元认知是 PBSM 区别于其他 Agent 框架的关键设计。没有元认知的 Agent 就像一个无法意识到自己在犯错的系统：

- **注意力控制**：模拟人类注意力的有限性。系统可以在 `LowVigilance`（低警戒）/ `ModerateFocus`（适度聚焦）/ `HighReconnaissance`（高度侦察）模式间切换，避免在单一目标上过度投入或注意力过于分散。

- **遗忘机制**：不是所有信息都值得永远保留。遗忘执行器（ForgettingExecutor）基于价值评分（Value Evaluation）识别低价值信念，支持延迟遗忘（Deferred Forget）和保护机制（Protected Beliefs），防止关键信息被误删。

- **异常检测**：自动检测四种异常模式——振荡（Oscillation）、锁定（Locked）、过度聚焦（Excessive Focus）、漂移（Drift），并触发干预（Intervention）。这使得系统具备了"知道自己出问题了"的能力。

### 为什么用意图栈而不是任务队列？

意图栈（Intention Stack）的设计灵感来自 BDI（Belief-Desire-Intention）架构：

- **层级目标**：支持嵌套意图（主目标包含子目标），每层有独立的计划步骤（Plan Steps）
- **漂移检测**：自动评估当前执行是否偏离原始意图，支持纠正动作（Corrective Action）
- **检查点恢复**：任何层级都可以创建检查点，失败时回滚到最近的稳定状态
- **微预测**：每个意图可以关联一个微预测（Micro Prediction），在推进前验证预期

### 为什么用 Rust？

| 考量 | Rust 的优势 |
|------|------------|
| 性能 | 零成本抽象，信念图查询和预测生成在微秒级完成 |
| 并发安全 | 编译期保证数据竞争自由，`Arc<RwLock>` 模式天然适合读多写少的信念图 |
| 可靠性 | 无空指针、无数据竞争、无未定义行为，545+ 测试零 unsafe 代码 |
| Python 互操作 | PyO3 提供零拷贝的 Rust-Python 桥接，兼顾性能和易用性 |
| 嵌入式部署 | 编译为单一二进制，适合 Docker/K8s 部署，无运行时依赖 |

---

## 概述

PBSM（Predictive Belief State Machine）是一个基于信念图（Belief Graph）的认知架构框架，核心设计理念是 **预测→验证→修正** 的闭环循环：

1. **输入**：外部工具产出数据，通过 ToolAdapter（M3）解析为结构化断言，更新信念图
2. **预测**：PredictionEngine（M2）基于信念图生成预测
3. **执行**：Orchestrator 协调各模块执行系统周期
4. **验证**：验证预测结果，根据偏差更新信念，驱动下一轮预测

### 七大核心模块

| 模块 | 名称 | 职责 |
|------|------|------|
| M1 | BeliefGraph | 信念图 — 知识表示、图结构管理、快照回滚 |
| M2 | PredictionEngine | 预测引擎 — 基于信念图生成和验证预测 |
| M3 | ToolAdapter | 工具适配器 — 外部工具集成、断言提交 |
| M4 | Metacognition | 元认知 — 注意力模式、遗忘机制、异常检测 |
| M5 | IntentionStack | 意图栈 — 任务规划、层级管理、检查点恢复 |
| M6 | Communication | 通信模块 — 跨 Agent 协调、安全过滤 |
| M7 | EventBus | 事件总线 — 系统事件分发、历史记录、订阅管理 |

---

## 架构

### 完整分层架构

![Layered Architecture](docs/architecture_zh.png)

📊 [查看完整架构图](docs/architecture_zh.html)

系统采用五层架构设计：Python 应用层 → PyO3 桥接层 → 编排器层 → 核心模块层（M1-M7）→ 存储层（SQLite/Sled/Snapshot）。详见架构图。

### K8s 部署拓扑

📊 [查看部署拓扑图](docs/deployment_zh.html)

### 数据流图

📊 [查看数据流图](docs/dataflow_zh.html)

---

## 快速开始

### 环境要求

- **Rust** 1.75+（推荐 1.80+）
- **Python** 3.13+（已测试 3.13）
- **maturin** ≥ 1.0（用于构建 Python wheel）

### 构建 Rust Core

```bash
# 克隆项目
git clone <repo-url> && cd pbsm

# 构建并运行测试
cargo build --release
cargo test --workspace

# 运行 clippy 检查
cargo clippy --workspace -- -D warnings
```

### 构建 Python Wheel

```bash
# 创建 Python 3.13 虚拟环境
python3.13 -m venv .venv313
source .venv313/bin/activate

# 安装 maturin
pip install maturin

# 构建 wheel（⚠️ Python 3.14+ 必须设置环境变量）
cd crates/pbsm-python
VIRTUAL_ENV=$(echo $VIRTUAL_ENV) \
PATH="$VIRTUAL_ENV/bin:$PATH" \
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 \
maturin develop --release

# 验证安装
python -c "from pbsm_python import PyPbsmOrchestrator; print('OK')"
```

### 安装 Python 适配器

```bash
cd adapters/tool_adapter
pip install -e .
```

---

## Python API 参考

PBSM 通过 PyO3 绑定将 Rust 核心能力暴露给 Python。以下为 `PyPbsmOrchestrator` 提供的核心方法：

### 信念图操作（M1）

```python
from pbsm_python import PyPbsmOrchestrator, PyPbsmConfig

orch = PyPbsmOrchestrator(PyPbsmConfig('{"graph": {"maxNodes": 500}}'))
```

| 方法 | 签名 | 说明 |
|------|------|------|
| `create_belief` | `create_belief(node_type, name, attributes_json?, source?, source_type?, tags_json?, initial_confidence?)` | 创建信念节点。`node_type` 支持 User/File/Tool/Variable/Concept/Event/Agent/Resource/Process；`source_type` 支持 DirectObservation/ToolReturn/UserInput/Derived/MemoryRestore/AgentSync |
| `create_edge` | `create_edge(edge_type, source_id, target_id, confidence)` | 创建信念边。`edge_type` 支持 Owns/DependsOn/Authorizes/Calls/Contains/RelatedTo/Enables/Blocks/Modifies/References/Precedes/Follows/SynchronizesWith |
| `query_beliefs` | `query_beliefs(query_json)` | 查询信念，支持 `node_type`/`name_contains`/`tags`/`min_confidence`/`limit` 过滤条件 |
| `get_belief` | `get_belief(belief_id)` | 获取单个信念详情，包含属性、标签、边关系等 |
| `get_belief_graph_stats` | `get_belief_graph_stats()` | 获取信念图统计（节点数、边数、平均置信度、高/低置信度数量） |

**示例**：

```python
# 创建信念节点
result = orch.create_belief(
    node_type="File",
    name="src/main.rs",
    attributes_json='{"complexity": 12, "language": "rust"}',
    source="code_analyzer",
    source_type="ToolReturn",
    tags_json='["source-code", "entry-point"]',
    initial_confidence=0.9
)
# → {"belief_id": "uuid-...", "node_type": "File", "name": "src/main.rs"}

# 创建信念边
edge = orch.create_edge(
    edge_type="Contains",
    source_id="uuid-module",
    target_id="uuid-function",
    confidence=0.85
)
# → {"edge_id": "uuid-...", "edge_type": "Contains", ...}

# 查询信念
results = orch.query_beliefs('{"node_type": "File", "min_confidence": 0.7, "limit": 10}')
# → {"status": "ok", "results": [...], "total_count": 3}

# 获取单个信念
belief = orch.get_belief("uuid-...")
# → {"belief_id": "...", "node_type": "File", "name": "...", "attributes": {...}, ...}

# 信念图统计
stats = orch.get_belief_graph_stats()
# → {"node_count": 5, "edge_count": 12, "average_confidence": 0.82, ...}
```

### 意图栈操作（M5）

| 方法 | 签名 | 说明 |
|------|------|------|
| `push_intention` | `push_intention(description, priority?)` | 推入意图。`priority` 支持 Critical/High/Medium/Low，默认 Medium |
| `pop_intention` | `pop_intention()` | 弹出最顶层意图（LIFO 语义），弹出后自动重新计算剩余节点的 level 和索引 |
| `get_intention_stack_state` | `get_intention_stack_state()` | 获取意图栈状态，包含深度和各层描述、优先级、执行状态 |

**示例**：

```python
# 推入意图
push_result = orch.push_intention("重构高复杂度函数", priority="High")
# → {"status": "ok", "layer_index": 0, ...}

# 再推入子意图
push_result2 = orch.push_intention("添加单元测试", priority="Medium")

# 查看意图栈状态
state = orch.get_intention_stack_state()
# → {"depth": 2, "layers": [{"layer_index": 0, ...}, {"layer_index": 1, ...}]}

# 弹出最顶层意图（LIFO）
pop_result = orch.pop_intention()
# → 弹出"添加单元测试"，自动重新计算层级
```

### 元认知操作（M4）

| 方法 | 签名 | 说明 |
|------|------|------|
| `get_attention_status` | `get_attention_status()` | 获取注意力状态（当前模式、注意力值、衰减率等） |
| `detect_anomalies` | `detect_anomalies(window_size?)` | 异常检测，返回异常类型、严重程度和建议干预措施 |

**示例**：

```python
# 获取注意力状态
attention = orch.get_attention_status()
# → {"mode": "ModerateFocus", "attention_value": 0.5, ...}

# 异常检测
anomalies = orch.detect_anomalies(window_size=50)
# → {"has_anomalies": false, "severity": "None", "anomaly_count": 0, "anomalies": []}
```

### 事件总线操作（M7）

| 方法 | 签名 | 说明 |
|------|------|------|
| `get_event_history` | `get_event_history(limit?)` | 获取事件历史，默认返回最近 100 条，包含事件类型和来源模块 |

**示例**：

```python
# 获取事件历史
history = orch.get_event_history(limit=10)
# → {"total_events": 42, "events": [{"event_type": "...", "source_module": "..."}, ...]}
```

---

## 什么时候使用 PBSM？

PBSM 适用于以下场景：

| 场景 | 为什么需要 PBSM |
|------|----------------|
| LLM/Agent 需要在行动前评估风险 | PBSM 的预测机制让 Agent "先想后做"，避免不可逆操作 |
| Agent 需要长期记忆和经验积累 | 三层记忆（原始日志/快照/经验）让 Agent 从历史中学习 |
| 多步任务需要规划和回滚 | 意图栈支持层级目标、漂移检测、检查点恢复 |
| Agent 需要自我监控和纠错 | 元认知自动检测振荡/漂移/过度聚焦，触发干预 |
| 多 Agent 需要共享信念和协调 | 通信模块提供信念同步、冲突协商、安全过滤 |
| 需要可解释的 Agent 决策 | 每个决策都有预测依据，可追溯"为什么这样做" |

**不适合的场景**：简单的单轮问答、无需记忆的无状态工具调用、纯计算任务。

---

## 如何与 LLM/Agent 对接

### 核心交互模型

📊 [查看 LLM/Agent 交互模型图](docs/integration_zh.html)

PBSM 与 LLM/Agent 的交互遵循 **预测→执行→验证→修正** 的闭环：

1. LLM 调用工具，获得原始输出
2. 将原始输出交给 ToolAdapter（M3）解析为结构化断言
3. 断言提交到 PBSM 核心，更新信念图（M1）
4. PredictionEngine（M2）基于信念图生成预测
5. LLM 执行操作后，将观测结果反馈给 PBSM 验证
6. 验证残差驱动信念修正，返回诊断结果给 LLM

**LLM/Agent 输入什么**：工具的原始输出（JSON/HTML/CSV/TEXT/ERROR）
**LLM/Agent 得到什么**：结构化断言 + 预测 + 验证结果 + 诊断信息

### 场景一：代码分析 Agent

一个 LLM Agent 分析代码库，需要理解代码结构、记住历史发现、预测修改影响。

```python
from pbsm_tool_adapter import ToolAdapter, RawOutput

adapter = ToolAdapter(pbsm_config_json='{"graph": {"maxNodes": 2000}}')

# ── 第 1 步：LLM 调用代码分析工具，获得 JSON 输出 ──
# （这一步由 LLM/Agent 框架完成，例如调用 grep、ast 分析器等）
tool_output = '{"findings": [{"file": "src/main.rs", "type": "function", "name": "process", "complexity": 12}]}'

# ── 第 2 步：将工具输出交给 PBSM 解析 ──
# 输入：原始工具输出 → 输出：结构化断言列表
raw = RawOutput(content=tool_output, content_type="application/json")
parsed = adapter.parse_tool_output(raw_output=raw, tool_id="code_analyzer")

# parsed.assertions 是结构化断言，例如：
# [
#   StructuredAssertion(
#     assertion_type=ENTITY_ATTRIBUTE,
#     subject=AssertionSubject(entity_type="function", entity_id="src/main.rs::process"),
#     predicate="has_complexity",
#     object=AssertionObject(value=12, value_type=NUMBER),
#     confidence=ConfidenceInfo(score=0.9, method=EXTRACTED)
#   )
# ]

# ── 第 3 步：将断言提交到 PBSM 核心，更新信念图 ──
# 输入：断言列表 → 输出：提交结果（哪些被接受）
submit_result = adapter.submit_to_core(parsed.assertions)
# {"status": "ok", "accepted": 3, "assertion_ids": [...]}

# ── 第 4 步：启动 PBSM 任务，让系统开始预测循环 ──
# 输入：任务描述 → 输出：任务创建结果
task = adapter.start_task("重构高复杂度函数")
# {"status": "ok", "description": "重构高复杂度函数", ...}

# ── 第 5 步：执行一个系统周期，获取当前状态 ──
# 输入：无 → 输出：注意力模式、活跃预测数、待遗忘数
cycle = adapter.execute_cycle()
# {"attention_mode": "MODERATE_FOCUS", "active_predictions": 2, "pending_forget_count": 0}

# ── 第 6 步：LLM 执行操作后，验证预测 ──
# 输入：预测 ID + 实际观测 → 输出：验证结果（残差、置信度变化）
verify = adapter.verify_prediction(
    prediction_id="pred_001",
    observations=[{"actual_complexity": 8, "refactored": True}],
)
# {"status": "verified", "residual": 0.33, "confidence_change": +0.05}

# ── 第 7 步：如果出错，通知 PBSM 触发干预 ──
# 输入：错误描述 + 严重级别 → 输出：异常数、是否已干预
error = adapter.handle_pbsm_error("重构后测试失败", "high")
# {"anomaly_count": 1, "intervention_applied": True}
```

### 场景二：运维监控 Agent

一个 LLM Agent 监控服务器状态，需要记住历史指标、预测异常、协调多个 Agent。

```python
from pbsm_tool_adapter import ToolAdapter, RawOutput

adapter = ToolAdapter(pbsm_config_json='{"graph": {"maxNodes": 5000}}')

# ── LLM 调用监控 API，获得服务器指标 ──
metrics_json = '{"server": "prod-01", "cpu": 92, "memory": 85, "disk_io": 3400}'

# ── 解析并提交到信念图 ──
raw = RawOutput(content=metrics_json, content_type="application/json")
parsed = adapter.parse_tool_output(raw_output=raw, tool_id="monitor_api")
adapter.submit_to_core(parsed.assertions)

# ── PBSM 基于历史信念生成预测 ──
# 例如：预测 CPU 将在 10 分钟内超过 95%
cycle = adapter.execute_cycle()
# {"attention_mode": "HIGH_RECONNAISSANCE", "active_predictions": 3, ...}
# 注意力自动切换到高度侦察模式

# ── LLM 执行扩容操作后，验证预测 ──
verify = adapter.verify_prediction(
    prediction_id="pred_cpu_spike",
    observations=[{"cpu_after_scaleout": 45}],
)
# 预测"CPU 将超 95%"与实际"扩容后 CPU 45%"不符
# → 残差较大 → 信念修正：扩容有效 → 下次预测会考虑扩容能力

# ── 查看当前信念图状态 ──
stats = adapter.get_belief_graph_stats()
# {"node_count": 47, "edge_count": 132, "has_memory_store": False}
```

### 场景三：Rust 嵌入式集成

如果你的 Agent 框架是 Rust 编写的，可以直接使用核心 API：

```rust
use pbsm_core::{PbsmConfig, PbsmOrchestrator, AnomalySeverity};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = PbsmConfig::default();
    let orchestrator = PbsmOrchestrator::new(config);

    // Agent 提交任务
    let task = orchestrator.start_task("分析代码库结构".to_string(), None).await?;

    // 执行预测-验证周期
    let cycle = orchestrator.execute_cycle().await?;
    // cycle.attention_mode → 当前注意力模式
    // cycle.active_predictions → 活跃预测数量

    // Agent 遇到错误时，通知元认知系统
    let error_result = orchestrator.handle_error(
        "API 超时".to_string(),
        AnomalySeverity::High,
    )?;
    // error_result.anomaly_count → 检测到的异常数
    // error_result.intervention_applied → 是否已自动干预

    // 诊断：一致性检查 + 内存占用
    let report = orchestrator.consistency_check();
    let footprint = orchestrator.memory_footprint();

    Ok(())
}
```

### 数据流总结

| 步骤 | LLM/Agent 输入 | PBSM 输出 | 说明 |
|------|---------------|-----------|------|
| 解析工具输出 | `RawOutput`（原始文本 + 格式） | `ParseResult`（结构化断言列表） | 自动识别格式，提取实体/关系/事件 |
| 提交断言 | `list[StructuredAssertion]` | `{"status", "accepted", "assertion_ids"}` | 断言更新信念图 |
| 启动任务 | 任务描述字符串 | `{"status", "description"}` | 在意图栈中创建新意图 |
| 执行周期 | 无 | `{"attention_mode", "active_predictions", ...}` | 触发预测生成和元认知评估 |
| 验证预测 | 预测 ID + 观测结果 | `{"status", "residual", "confidence_change"}` | 残差驱动信念修正 |
| 错误处理 | 错误描述 + 严重级别 | `{"anomaly_count", "intervention_applied"}` | 触发元认知干预 |
| 诊断 | 无 | 信念图统计 / 一致性报告 / 内存占用 | 运维监控用 |

---

## 多 Agent 支持

PBSM 通过 M6 通信模块原生支持多 Agent 协作。每个 Agent 拥有独立的 `PbsmOrchestrator` 实例（独立的信念图、意图栈、元认知），通过通信模块进行协调。

### 多 Agent 架构

📊 [查看多 Agent 架构图](docs/multi-agent_zh.html)

每个 Agent 拥有独立的 `PbsmOrchestrator` 实例（独立的信念图、意图栈、元认知），通过 M6 通信模块进行协调。通信模块包含：SyncManager（信念同步）、ConflictDetector（冲突检测）、NegotiationHandler（冲突协商）、DelegationManager（任务委派），底层由 AccessController 和 SensitiveDataFilter 提供安全保障。

### 核心能力

| 能力 | 模块 | 说明 |
|------|------|------|
| 信念同步 | `SyncManager` | Agent 间同步信念图快照，支持增量同步和全量同步 |
| 冲突检测 | `ConflictDetector` | 检测属性不一致、关系不一致、意图冲突、置信度冲突 |
| 冲突协商 | `NegotiationHandler` | 通过提案-反提案机制解决冲突，支持自动和手动协商 |
| 任务委派 | `DelegationManager` | 将子任务委派给其他 Agent，支持质量标准和超时控制 |
| 访问控制 | `AccessController` | 基于角色（Coordinator/Collaborator/Observer/Worker）的权限管理 |
| 数据过滤 | `SensitiveDataFilter` | 共享快照时自动过滤/脱敏敏感字段 |

### 多 Agent 使用注意事项

1. **每个 Agent 独立实例**：每个 Agent 应创建自己的 `PbsmOrchestrator`，避免共享状态导致的并发问题。信念同步通过通信模块完成，而非共享内存。

2. **角色与权限**：通信模块定义了 4 种角色——`Coordinator`（协调者）、`Collaborator`（协作者）、`Observer`（观察者）、`Worker`（执行者）。不同角色对资源（Snapshot/Belief/Relation/Intent/Prediction）有不同的读/写/删除/共享/委派权限。

3. **冲突处理策略**：当多个 Agent 对同一信念有不同认知时：
   - `AttributeMismatch`：属性值不一致（如 Agent A 认为 CPU=92%，Agent B 认为 CPU=88%）
   - `RelationMismatch`：关系不一致（如因果链分歧）
   - `IntentMismatch`：意图冲突（如两个 Agent 同时修改同一文件）
   - `ValueConfidenceConflict`：置信度冲突（同一事实，不同置信度）

4. **敏感数据过滤**：共享快照前必须配置 `SensitiveDataFilter`，支持 4 种过滤动作：
   - `Remove`：移除字段
   - `Redact`：替换为 `[REDACTED]`
   - `Mask`：部分遮盖（如 `192.168.***.***`）
   - `Reject`：拒绝整个快照

5. **同步性能**：信念同步涉及快照构造→压缩→传输→验证→融合 5 个阶段。大规模信念图（>1000 节点）建议使用增量同步（`SyncRequestType::Incremental`）而非全量同步。

6. **当前限制**：多 Agent 通信模块目前仅在 Rust 核心层实现，尚未暴露到 Python 绑定。Python 用户如需多 Agent 功能，需通过 Rust API 直接使用，或等待后续版本的 Python 绑定更新。

---

## 配置管理

```python
from pbsm_python import PyPbsmConfig

# 从 TOML 文件加载
config = PyPbsmConfig("/path/to/config.toml")

# 从 JSON 文件加载
config = PyPbsmConfig("/path/to/config.json")

# 从 JSON 字符串创建
config = PyPbsmConfig('{"graph": {"maxNodes": 2000}}')

# 验证配置
config.validate()  # 无效配置会抛出 ValueError

# 读取/修改配置属性
print(config.graph_max_nodes)    # 2000
config.graph_max_nodes = 5000    # 修改
print(config.graph_max_edges)    # 10000
config.graph_max_edges = 20000

# 序列化
json_str = config.to_json()      # 输出 JSON 字符串
config.save("/path/to/output.toml")  # 保存为 TOML
config.save("/path/to/output.json")  # 保存为 JSON
```

### Rust：配置文件加载

```rust
use pbsm_core::orchestrator::PbsmConfig;
use std::path::Path;

// 从 TOML 文件加载
let config = PbsmConfig::load_from_toml(Path::new("config.toml"))?;

// 从 JSON 文件加载
let config = PbsmConfig::load_from_json(Path::new("config.json"))?;

// 从 JSON 字符串创建
let config = PbsmConfig::from_json_str(r#"{"graph": {"maxNodes": 2000}}"#)?;

// 验证
config.validate()?;

// 保存
config.save_to_toml(Path::new("output.toml"))?;
config.save_to_json(Path::new("output.json"))?;
```

---

## 配置参考

### 完整 TOML 配置示例

```toml
[graph]
maxNodes = 500
maxEdges = 2000
defaultConfidence = 0.5

[intention_stack]
max_stack_depth = 20
max_stack_capacity = 500
max_revert_depth = 5
root_visibility_threshold = 0.6
default_step_timeout = 30000
default_max_retries = 3
max_checkpoints_per_layer = 20

[intention_stack.drift_threshold]
warning = 0.3
moderate = 0.5
severe = 0.7
critical = 0.9

[metacognitive.attention]
min_attention = 0.1
max_attention = 1.0
default_attention = 0.5
decay_rate = 0.05
boost_step = 0.4
time_decay_rate = 0.001
max_adjustment = 0.3
min_adjustment_interval_ms = 100

[metacognitive.attention.weight_configuration]
goal_relevance_weight = 0.35
access_frequency_weight = 0.25
recency_weight = 0.20
residual_weight = 0.20

[metacognitive.value_evaluation]
recency_decay_lambda = 0.05
access_window_size = 50
max_access_threshold = 10

[metacognitive.forgetting]
forget_threshold = 0.2
max_active_beliefs = 500
min_survival_steps = 10
forget_cooldown_steps = 20
max_defer_steps = 200
batch_forgive_interval = 50
residual_defer_threshold = 0.7

[metacognitive.anomaly_detection]
coverage_threshold = 0.3
oscillation_threshold = 5
drift_threshold = 0.2
lock_threshold = 100
anomaly_check_interval = 25
anomaly_history_size = 100

[memory]
storage_path = "./data/memory"
cache_size = 100
max_log_age_days = 90
compression_type = "Lz4"
max_recent_sessions = 30
base_confidence_threshold = 0.4
cleanup_auto_trigger_threshold = 0.85
retrieval_default_limit = 20
importance_retention_bonus = 1.5
archive_threshold_days = 30
```

### 配置字段说明

| 配置项 | 字段 | 类型 | 默认值 | 说明 |
|--------|------|------|--------|------|
| `graph` | `maxNodes` | usize | 500 | 信念图最大节点数 |
| `graph` | `maxEdges` | usize | 2000 | 信念图最大边数 |
| `graph` | `defaultConfidence` | f64 | 0.5 | 新节点默认置信度 |
| `intention_stack` | `max_stack_depth` | usize | 20 | 意图栈最大深度 |
| `intention_stack` | `max_stack_capacity` | usize | 500 | 意图栈最大容量 |
| `intention_stack` | `max_revert_depth` | usize | 5 | 最大回退深度 |
| `metacognitive.attention` | `default_attention` | f64 | 0.5 | 默认注意力参数（≤0.3 LowVigilance / 0.3-0.7 ModerateFocus / >0.7 HighReconnaissance） |
| `metacognitive.attention` | `min_attention` | f64 | 0.1 | 注意力下限 |
| `metacognitive.attention` | `max_attention` | f64 | 1.0 | 注意力上限 |
| `metacognitive.forgetting` | `forget_threshold` | f64 | 0.2 | 遗忘阈值（低于此价值的信念将被淘汰） |
| `metacognitive.forgetting` | `max_active_beliefs` | usize | 500 | 最大活跃信念数 |
| `metacognitive.anomaly_detection` | `coverage_threshold` | f64 | 0.3 | 异常覆盖率阈值 |
| `metacognitive.anomaly_detection` | `oscillation_threshold` | usize | 5 | 振荡检测阈值（连续调整次数） |
| `memory` | `storage_path` | PathBuf | "./data/memory" | 存储目录路径 |
| `memory` | `cache_size` | usize | 100 | 缓存大小 |
| `memory` | `max_log_age_days` | u32 | 90 | 日志最大保留天数 |
| `memory` | `compression_type` | enum | Lz4 | 压缩算法（NONE/LZ4/ZSTD） |
| `memory` | `base_confidence_threshold` | f64 | 0.4 | 基础置信度阈值 |
| `memory` | `cleanup_auto_trigger_threshold` | f64 | 0.85 | 自动清理触发阈值 |

---

## 部署

### Docker 构建

```bash
# 构建镜像
docker build -t pbsm-server .

# 本地运行
docker run -p 8080:8080 \
  -v $(pwd)/data:/pbsm/data \
  -e RUST_LOG=info \
  pbsm-server
```

### Kubernetes (kind) 部署

```bash
# 加载镜像到 kind 集群
kind load docker-image pbsm-server:latest

# 应用配置
kubectl apply -f k8s/configmap.yaml
kubectl apply -f k8s/deployment.yaml

# 检查状态
kubectl get pods -l app=pbsm-server
kubectl logs -f deployment/pbsm-server

# 端口转发（本地测试）
kubectl port-forward svc/pbsm-server 8080:8080
```

---

## 性能基准

项目包含 11 组 criterion 基准测试，覆盖所有核心热路径：

| 基准组 | 测试内容 |
|--------|----------|
| `belief_graph/create_belief` | 信念节点创建（10/100/500 节点） |
| `belief_graph/query_by_type` | 按类型查询 |
| `belief_graph/query_by_tag` | 按标签查询 |
| `event_bus/publish` | 事件发布（64/256/1024 容量） |
| `event_bus/subscribe_receive` | 订阅与接收 |
| `metacognitive/attention` | 注意力状态查询 |
| `metacognitive/anomaly_detection` | 异常检测 |
| `orchestrator/execute_cycle` | 执行周期 |
| `orchestrator/start_task` | 启动任务 |
| `prediction_engine/create_prediction` | 创建预测 |
| `config/serialization` | TOML/JSON 序列化 |

```bash
# 运行基准测试
cargo bench --bench pbsm_benchmarks

# 报告生成在 target/criterion/
```

---

## 注意事项

### ⚠️ Python 3.14+ 兼容性

PyO3 0.22 官方最高支持 Python 3.13。使用 Python 3.14+ 时 **必须** 设置环境变量：

```bash
export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
```

否则构建会报错：`the configured Python interpreter version (3.14) is newer than PyO3's maximum supported version (3.13)`。

此环境变量在 `maturin develop` 和 `maturin build` 时都需要设置。

### 🔄 Fallback 模式

当 `pbsm_python` 原生模块未安装时，Python 适配器会自动回退到 **Fallback 模式**：

- 所有操作在 Python 层模拟完成
- 功能等价但性能较低
- 日志中会显示 `PBSM native module not available, using fallback mode`

要启用原生模式，确保 `pbsm_python` wheel 已正确安装：

```bash
python -c "from pbsm_python import PyPbsmOrchestrator; print('Native mode OK')"
```

### 🔒 路径遍历防护

PyO3 绑定中的文件操作（`PyPbsmConfig.new()` 和 `PyPbsmConfig.save()`）会拒绝包含 `..` 的路径，防止路径遍历攻击：

```python
# ❌ 会被拒绝
config = PyPbsmConfig("../../etc/passwd")
config.save("../../tmp/evil.toml")

# ✅ 正常使用
config = PyPbsmConfig("/absolute/path/config.toml")
config.save("./output/config.json")
```

### 💾 内存与容量限制

- 信念图有 `maxNodes` 和 `maxEdges` 限制，超出后创建操作会返回错误
- EventBus（M7）历史默认最多保留 1000 条事件（可通过 `with_history_capacity()` 调整）
- 一致性检查会检测节点/边数量是否超出配置限制
- 使用 `memory_footprint()` 监控内存使用情况

### ⏱️ 异步运行时

PyO3 绑定中的异步方法（`start_task`, `execute_cycle`）内部创建 `tokio::Runtime` 并通过 `block_on` 同步调用。这意味着：

- Python 端调用是同步的，无需 `await`
- 每次调用创建新的 Runtime，有一定开销
- 不适合在已有的 tokio 异步上下文中调用

### 🥞 IntentionStack pop 语义

IntentionStack 的 `pop_intention()` 默认弹出最顶层意图（LIFO 语义），pop 后系统自动重新计算剩余节点的 level 和索引，确保 push/pop 交替使用不会触及 max_depth 限制。

---

## 测试

### Rust 测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定模块测试
cargo test -p pbsm-core -- belief_graph

# 运行并显示输出
cargo test --workspace -- --nocapture
```

当前测试覆盖：**545+ 单元测试 + 30 集成测试**

### Python 测试

```bash
# 确保 wheel 已安装
cd crates/pbsm-python && \
VIRTUAL_ENV=../../.venv313 \
PATH="../../.venv313/bin:$PATH" \
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 \
maturin develop --release

# 运行适配器测试
cd ../../adapters/tool_adapter
pytest tests/ -v

# 运行 Demo 验证测试
cd ../../demo
pytest test_comprehensive.py -v
```

当前测试覆盖：**59+ Python 综合测试**

### 基准测试

```bash
cargo bench --bench pbsm_benchmarks
```

---

## 项目结构

```
pbsm/
├── Cargo.toml                    # Workspace 根配置
├── Dockerfile                    # 多阶段构建 Docker 镜像
├── .dockerignore
├── API_REFERENCE.md              # API 参考文档
├── crates/
│   ├── pbsm-core/                # Rust 核心库
│   │   ├── Cargo.toml
│   │   ├── benches/              # Criterion 基准测试
│   │   │   └── pbsm_benchmarks.rs
│   │   └── src/
│   │       ├── lib.rs            # 公共 API 导出
│   │       ├── orchestrator.rs   # 统一编排器
│   │       ├── event_bus.rs      # M7 事件总线
│   │       ├── error.rs          # 错误类型定义
│   │       ├── types/            # 共享类型
│   │       └── modules/
│   │           ├── belief_graph/     # M1 信念图
│   │           ├── prediction_engine/ # M2 预测引擎
│   │           ├── metacognition/    # M4 元认知
│   │           ├── memory/           # 记忆存储
│   │           ├── intention_stack/  # M5 意图栈
│   │           ├── communication/    # M6 通信
│   │           └── common/           # 共享类型与事件
│   └── pbsm-python/              # PyO3 Python 绑定
│       ├── Cargo.toml
│       ├── pyproject.toml        # maturin 构建配置
│       └── src/
│           └── lib.rs            # PyO3 导出
├── adapters/
│   └── tool_adapter/             # M3 工具适配器
│       ├── pyproject.toml
│       ├── pbsm_tool_adapter/
│       │   ├── __init__.py
│       │   ├── tool_adapter.py   # 主适配器
│       │   └── pbsm_bindings.py  # PyO3 桥接
│       └── tests/                # Python 测试
├── demo/                         # 验证演示
│   ├── comprehensive_demo.py     # 全核心模块深度验证 Demo
│   ├── test_comprehensive.py     # 全面 pytest 测试
│   └── DEMO_REPORT.md            # 验证报告
├── k8s/                          # Kubernetes 部署
│   ├── deployment.yaml           # Deployment + Service + PVC
│   └── configmap.yaml            # 配置映射
└── docs/                         # 文档与架构图
    ├── architecture.html         # 完整分层架构图
    ├── deployment.html           # K8s 部署拓扑图
    ├── dataflow.html             # 数据流图
    ├── integration.html          # LLM/Agent 交互模型图
    └── multi-agent.html          # 多 Agent 架构图
```

---

## 许可证

MIT License
