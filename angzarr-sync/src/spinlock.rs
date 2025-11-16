//! Spinlock implementation

use core::sync::atomic::{AtomicBool, Ordering};
use core::cell::UnsafeCell;

/// A basic spinlock
#[repr(C)]
pub struct Spinlock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for Spinlock<T> {}
unsafe impl<T: Send> Sync for Spinlock<T> {}

impl<T> Spinlock<T> {
    /// Create a new spinlock
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire the lock (busy wait)
    ///
    /// # Safety
    /// Must be used carefully to avoid deadlocks
    pub unsafe fn lock(&self) -> SpinlockGuard<T> {
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinlockGuard { lock: self }
    }

    /// Try to acquire the lock without blocking
    pub fn try_lock(&self) -> Option<SpinlockGuard<T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(SpinlockGuard { lock: self })
        } else {
            None
        }
    }

    /// Unlock the spinlock
    ///
    /// # Safety
    /// Must only be called by the lock holder
    unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

/// RAII guard for spinlock
pub struct SpinlockGuard<'a, T> {
    lock: &'a Spinlock<T>,
}

impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            self.lock.unlock();
        }
    }
}

impl<'a, T> core::ops::Deref for SpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinlock_basic() {
        let lock = Spinlock::new(42);
        unsafe {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
    }

    #[test]
    fn test_spinlock_try_lock() {
        let lock = Spinlock::new(100);
        let guard = lock.try_lock();
        assert!(guard.is_some());
        assert_eq!(*guard.unwrap(), 100);
    }
}
