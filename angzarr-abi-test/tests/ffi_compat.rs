//! FFI Layer Compatibility Tests
//!
//! Verify that FFI types and error codes match Linux kernel

use angzarr_ffi::{GfpFlags, KernelError};
use static_assertions::*;

// Linux GFP flags (from include/linux/gfp.h)
const LINUX_GFP_KERNEL: u32 = 0x0cc0;
const LINUX_GFP_ATOMIC: u32 = 0x0020;
const LINUX_GFP_NOWAIT: u32 = 0x0000;
const LINUX___GFP_ZERO: u32 = 0x8000;

#[test]
fn test_gfp_flags_values() {
    // Verify GFP flag values match Linux exactly
    assert_eq!(GfpFlags::GFP_KERNEL.0, LINUX_GFP_KERNEL);
    assert_eq!(GfpFlags::GFP_ATOMIC.0, LINUX_GFP_ATOMIC);
    assert_eq!(GfpFlags::GFP_NOWAIT.0, LINUX_GFP_NOWAIT);
    assert_eq!(GfpFlags::__GFP_ZERO.0, LINUX___GFP_ZERO);
}

#[test]
fn test_gfp_flags_size() {
    // GFP flags should be u32
    assert_eq!(core::mem::size_of::<GfpFlags>(), 4);
}

#[test]
fn test_gfp_flags_transparent() {
    // Verify GfpFlags is a transparent wrapper
    let gfp = GfpFlags::GFP_KERNEL;
    let ptr = &gfp as *const GfpFlags as *const u32;
    unsafe {
        assert_eq!(*ptr, LINUX_GFP_KERNEL);
    }
}

// Linux errno values (from include/uapi/asm-generic/errno-base.h)
const LINUX_EPERM: i32 = 1;
const LINUX_ENOENT: i32 = 2;
const LINUX_EINTR: i32 = 4;
const LINUX_EIO: i32 = 5;
const LINUX_ENOMEM: i32 = 12;
const LINUX_EACCES: i32 = 13;
const LINUX_EFAULT: i32 = 14;
const LINUX_EBUSY: i32 = 16;
const LINUX_EEXIST: i32 = 17;
const LINUX_EINVAL: i32 = 22;
const LINUX_ENOSPC: i32 = 28;
const LINUX_EAGAIN: i32 = 11;

#[test]
fn test_kernel_error_values() {
    // Verify error codes match Linux errno values
    assert_eq!(KernelError::EPERM as i32, LINUX_EPERM);
    assert_eq!(KernelError::ENOENT as i32, LINUX_ENOENT);
    assert_eq!(KernelError::EINTR as i32, LINUX_EINTR);
    assert_eq!(KernelError::EIO as i32, LINUX_EIO);
    assert_eq!(KernelError::ENOMEM as i32, LINUX_ENOMEM);
    assert_eq!(KernelError::EACCES as i32, LINUX_EACCES);
    assert_eq!(KernelError::EFAULT as i32, LINUX_EFAULT);
    assert_eq!(KernelError::EBUSY as i32, LINUX_EBUSY);
    assert_eq!(KernelError::EEXIST as i32, LINUX_EEXIST);
    assert_eq!(KernelError::EINVAL as i32, LINUX_EINVAL);
    assert_eq!(KernelError::ENOSPC as i32, LINUX_ENOSPC);
    assert_eq!(KernelError::EAGAIN as i32, LINUX_EAGAIN);
}

#[test]
fn test_kernel_error_to_errno() {
    // Verify to_errno returns negative values like Linux
    assert_eq!(KernelError::ENOMEM.to_errno(), -12);
    assert_eq!(KernelError::EINVAL.to_errno(), -22);
    assert_eq!(KernelError::EACCES.to_errno(), -13);
}

#[test]
fn test_kernel_error_size() {
    // Error enum should be i32 sized
    assert_eq!(core::mem::size_of::<KernelError>(), 4);
}

// Compile-time assertions
assert_eq_size!(GfpFlags, u32);
assert_eq_size!(KernelError, i32);
