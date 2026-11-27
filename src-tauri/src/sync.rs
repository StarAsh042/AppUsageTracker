//! 取锁的统一入口。
//!
//! 全仓的 `Mutex` 都用这里取锁：锁被毒化（持锁线程 panic）时取回内部数据继续
//! 用，而不是跟着 panic。统计线程一旦 panic 就再也没人采样，界面上的数字会
//! 停在原地而用户毫不知情 —— 相比「可能读到 panic 前写入的一致快照」，停摆
//! 是更坏的结局。

use std::sync::{Mutex, MutexGuard};

pub(crate) trait MutexExt<T> {
    /// 取锁；被毒化时取回内部数据。
    fn lock_or_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_or_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn 锁被毒化后仍能取回数据() {
        let shared = Arc::new(Mutex::new(7i32));
        let writer = shared.clone();
        let _ = std::thread::spawn(move || {
            let _guard = writer.lock().unwrap();
            panic!("故意在持锁时 panic");
        })
        .join();
        assert!(shared.lock().is_err(), "持锁 panic 应让锁进入毒化状态");
        assert_eq!(*shared.lock_or_recover(), 7, "毒化后仍应读到 panic 前的值");
    }
}
