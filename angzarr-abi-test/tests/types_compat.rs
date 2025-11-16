//! Core Types ABI Compatibility Tests
//!
//! Verify that angzarr_core types match Linux kernel types

use angzarr_core::{Pid, Uid, Gid, Kref};
use static_assertions::*;

// Linux kernel types:
// typedef struct { int counter; } atomic_t;
// pid_t is int (i32)
// uid_t is unsigned int (u32)
// gid_t is unsigned int (u32)

#[test]
fn test_pid_size() {
    // pid_t is typically int (4 bytes)
    assert_eq!(
        core::mem::size_of::<Pid>(),
        4,
        "Pid must be 4 bytes like Linux pid_t"
    );
}

#[test]
fn test_uid_size() {
    // uid_t is unsigned int (4 bytes)
    assert_eq!(
        core::mem::size_of::<Uid>(),
        4,
        "Uid must be 4 bytes like Linux uid_t"
    );
}

#[test]
fn test_gid_size() {
    // gid_t is unsigned int (4 bytes)
    assert_eq!(
        core::mem::size_of::<Gid>(),
        4,
        "Gid must be 4 bytes like Linux gid_t"
    );
}

#[test]
fn test_kref_size() {
    // struct kref { atomic_t refcount; }
    // atomic_t is int (4 bytes) + padding for atomic operations
    // On 64-bit, AtomicU32 is typically 4 bytes
    assert_eq!(
        core::mem::size_of::<Kref>(),
        4,
        "Kref size must match Linux kref"
    );
}

#[test]
fn test_kref_alignment() {
    // Atomic types need proper alignment
    assert!(
        core::mem::align_of::<Kref>() >= 4,
        "Kref must be at least 4-byte aligned for atomic operations"
    );
}

// Compile-time size assertions
assert_eq_size!(Pid, i32);
assert_eq_size!(Uid, u32);
assert_eq_size!(Gid, u32);

#[test]
fn test_pid_representation() {
    // Verify Pid uses correct underlying type
    let pid = Pid(1234);
    assert_eq!(pid.0, 1234);

    // Verify it's a transparent wrapper
    let pid_ptr = &pid as *const Pid as *const i32;
    unsafe {
        assert_eq!(*pid_ptr, 1234);
    }
}

#[test]
fn test_uid_representation() {
    let uid = Uid(1000);
    assert_eq!(uid.0, 1000);

    let uid_ptr = &uid as *const Uid as *const u32;
    unsafe {
        assert_eq!(*uid_ptr, 1000);
    }
}

#[test]
fn test_kref_atomic_operations() {
    let kref = Kref::new();

    // Initial count should be 1
    assert_eq!(kref.count(), 1);

    // Test increment
    kref.get();
    assert_eq!(kref.count(), 2);

    // Test decrement
    let should_release = kref.put();
    assert!(!should_release);
    assert_eq!(kref.count(), 1);

    // Final decrement
    let should_release = kref.put();
    assert!(should_release);
    assert_eq!(kref.count(), 0);
}
