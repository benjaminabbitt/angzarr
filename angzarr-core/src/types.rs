//! Core kernel types

use core::sync::atomic::AtomicU32;

/// Process ID type
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(pub i32);

/// User ID type
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uid(pub u32);

/// Group ID type
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gid(pub u32);

/// Atomic reference counter (kref equivalent)
#[repr(transparent)]
pub struct Kref {
    refcount: AtomicU32,
}

impl Kref {
    /// Create a new reference counter with initial value of 1
    pub const fn new() -> Self {
        Self {
            refcount: AtomicU32::new(1),
        }
    }

    /// Increment the reference count
    pub fn get(&self) {
        self.refcount.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }

    /// Decrement the reference count and return true if it reaches 0
    pub fn put(&self) -> bool {
        self.refcount.fetch_sub(1, core::sync::atomic::Ordering::Release) == 1
    }

    /// Get current reference count (for debugging/testing only)
    pub fn count(&self) -> u32 {
        self.refcount.load(core::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kref() {
        let kref = Kref::new();
        assert_eq!(kref.count(), 1);

        kref.get();
        assert_eq!(kref.count(), 2);

        assert!(!kref.put());
        assert_eq!(kref.count(), 1);

        assert!(kref.put());
        assert_eq!(kref.count(), 0);
    }

    #[test]
    fn test_pid() {
        let pid = Pid(1234);
        assert_eq!(pid.0, 1234);
    }
}
