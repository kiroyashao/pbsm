//! 外部记忆存储模块
//!
//! 本模块是预测性信念状态机（PBSM）中 M4 外部记忆存储的实现。
//!
//! # 核心职责
//!
//! - 三层记忆存储：原始日志、快照、经验的结构化存储
//! - 上下文感知检索：基于信念状态和置信度缺口的智能检索
//! - 问题导向检索：从历史经验中匹配相似问题及解决方案
//! - 快照管理：信念状态和意图状态的完整快照与恢复
//! - 记忆清理：多层记忆的自动清理与归档策略
//!
//! # 三层记忆架构
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │              Experience Layer               │
//! │  结构化经验 · 长期保留 · 模式识别 · 知识积累   │
//! ├─────────────────────────────────────────────┤
//! │              Snapshot Layer                 │
//! │  状态快照 · 中期保留 · 错误恢复 · 会话管理     │
//! ├─────────────────────────────────────────────┤
//! │              RawLog Layer                   │
//! │  原始日志 · 短期保留 · 事件溯源 · 审计追踪     │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # 性能约束
//!
//! - 检索响应时间：≤50ms（标准深度）
//! - 快照写入时间：≤100ms
//! - 快照恢复时间：≤200ms

pub mod cache;
pub mod cleanup;
pub mod config;
pub mod error;
pub mod events;
pub mod layers;
pub mod retrieval;
pub mod storage;
pub mod store;
pub mod types;

pub use cleanup::CleanupEngine;
pub use error::{MemoryError, Result};
pub use events::EventSeverity;
pub use storage::{ExperienceRow, MemoryIndexRow, SnapshotRow, SqliteStorage};
pub use store::ExternalMemoryStore;
pub use types::*;
