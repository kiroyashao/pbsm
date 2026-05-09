//! 预测对象池实现
//!
//! 本模块实现了预测对象的池化复用机制，用于优化内存分配。
//!
//! # 核心职责
//!
//! - 预分配预测对象池
//! - 提供对象获取和归还接口
//! - 支持对象句柄管理
//!
//! # 池化策略
//!
//! 1. 创建时预分配指定数量的预测对象
//! 2. 获取时从池中弹出可用对象，空池时创建新对象
//! 3. 归还时重置对象状态后压回池中
//! 4. 池满时归还操作会被忽略

use parking_lot::Mutex;
use std::sync::Arc;
use uuid::Uuid;

use crate::types::prediction::Prediction;

/// 预测对象池结构体
///
/// # 设计说明
///
/// 对象池是一种内存优化模式，通过复用已分配对象减少分配开销：
/// - 使用互斥锁保证线程安全
/// - 支持配置最大池容量
/// - 获取时使用句柄模式管理对象生命周期
pub struct PredictionPool {
    pool: Mutex<Vec<Prediction>>,
    max_size: usize,
}

impl PredictionPool {
    /// 创建新的预测对象池
    ///
    /// # 参数
    /// * `max_size` - 对象池最大容量
    ///
    /// # 返回
    /// * 包含预分配预测对象的池实例
    pub fn new(max_size: usize) -> Self {
        let pool: Vec<Prediction> = (0..max_size).map(|_| Prediction::default()).collect();
        Self {
            pool: Mutex::new(pool),
            max_size,
        }
    }

    /// 从池中获取一个预测对象句柄
    ///
    /// # 返回
    /// * PredictionHandle 预测对象句柄
    ///
    /// # 说明
    ///
    /// 如果池中有可用对象，则弹出并返回；否则创建新的默认预测对象。
    /// 获取的对象必须通过 release 方法归还。
    pub fn acquire(&self) -> PredictionHandle {
        let mut guard = self.pool.lock();
        let prediction = guard.pop().unwrap_or_default();
        let id = Uuid::new_v4();
        PredictionHandle {
            id,
            prediction: Arc::new(Mutex::new(Some(prediction))),
        }
    }

    /// 归还预测对象到池中
    ///
    /// # 参数
    /// * `prediction` - 待归还的预测对象
    ///
    /// # 说明
    ///
    /// 归还时会先调用 reset 重置对象状态。
    /// 如果池已满，对象会被丢弃而不会压回池中。
    pub fn release(&self, mut prediction: Prediction) {
        prediction.reset();
        let mut guard = self.pool.lock();
        if guard.len() < self.max_size {
            guard.push(prediction);
        }
    }

    /// 获取池中可用对象数量
    pub fn len(&self) -> usize {
        self.pool.lock().len()
    }

    /// 判断池是否为空
    pub fn is_empty(&self) -> bool {
        self.pool.lock().is_empty()
    }
}

impl Default for PredictionPool {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// 预测对象句柄结构体
///
/// # 设计说明
///
/// 句柄模式用于管理池化对象的生命周期：
/// - 通过 Arc+Mutex 实现共享所有权的对象引用
/// - 支持多次获取对象引用
/// - take 方法会消耗句柄并返回对象所有权
pub struct PredictionHandle {
    id: Uuid,
    prediction: Arc<Mutex<Option<Prediction>>>,
}

impl PredictionHandle {
    /// 获取句柄ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// 获取预测对象的可变引用
    ///
    /// # 返回
    /// * MutexGuard 保护的对象引用
    pub fn get(&self) -> parking_lot::MutexGuard<'_, Option<Prediction>> {
        self.prediction.lock()
    }

    /// 获取预测对象所有权并从句柄中移除
    ///
    /// # 返回
    /// * Some(Prediction) 如果对象存在
    /// * None 如果对象已被取走
    pub fn take(&self) -> Option<Prediction> {
        self.prediction.lock().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_creation() {
        let pool = PredictionPool::new(10);
        assert_eq!(pool.len(), 10);
        assert!(!pool.is_empty());
    }

    #[test]
    fn test_pool_acquire_release() {
        let pool = PredictionPool::new(10);

        let handle1 = pool.acquire();
        assert_eq!(pool.len(), 9);

        {
            let mut guard = handle1.get();
            if let Some(ref mut pred) = *guard {
                pred.prediction_id = Uuid::new_v4();
            }
        }

        if let Some(p) = handle1.take() {
            pool.release(p);
        }

        assert_eq!(pool.len(), 10);
    }

    #[test]
    fn test_pool_capacity_limit() {
        let pool = PredictionPool::new(2);

        let handle1 = pool.acquire();
        let handle2 = pool.acquire();

        assert_eq!(pool.len(), 0);

        if let Some(p) = handle1.take() {
            pool.release(p);
        }

        assert_eq!(pool.len(), 1);

        if let Some(p) = handle2.take() {
            pool.release(p);
        }

        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_handle_id() {
        let pool = PredictionPool::new(10);
        let handle = pool.acquire();
        assert!(handle.id() != Uuid::nil());
    }
}
