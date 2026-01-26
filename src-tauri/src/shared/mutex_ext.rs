//! Usage: Mutex 扩展 trait，提供 poisoned 状态自动恢复能力

use std::sync::{Mutex, MutexGuard};

/// 为 Mutex 提供自动恢复能力的扩展 trait
pub(crate) trait MutexExt<T> {
    /// 获取 Mutex 锁，若发生 poisoned 则自动恢复并记录日志
    fn lock_or_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    #[track_caller]
    fn lock_or_recover(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let loc = std::panic::Location::caller();
                tracing::error!(
                    mutex_type = std::any::type_name::<T>(),
                    file = loc.file(),
                    line = loc.line(),
                    column = loc.column(),
                    "Mutex poisoned (线程 panic 导致)，已自动恢复数据但状态可能不一致"
                );
                poisoned.into_inner()
            }
        }
    }
}

#[cfg(test)]
mod tests;
