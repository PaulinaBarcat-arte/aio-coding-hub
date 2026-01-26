use super::*;
use std::sync::Arc;

#[test]
fn test_lock_or_recover_normal() {
    let mutex = Mutex::new(42);
    let guard = mutex.lock_or_recover();
    assert_eq!(*guard, 42);
}

#[test]
fn test_lock_or_recover_after_panic() {
    let mutex = Arc::new(Mutex::new(0));
    let mutex_clone = Arc::clone(&mutex);

    // 模拟导致 poisoned 的 panic
    let _ = std::thread::spawn(move || {
        let mut guard = mutex_clone.lock().unwrap();
        *guard = 100;
        panic!("模拟 panic");
    })
    .join();

    // 应能恢复并读取到 panic 前设置的值
    let guard = mutex.lock_or_recover();
    assert_eq!(*guard, 100);
}
