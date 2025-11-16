//! FFI compatibility layer for C/Rust interoperability
//!
//! This crate provides the foundational types and utilities for maintaining
//! binary compatibility with C code during the kernel migration.

#![cfg_attr(not(test), no_std)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

#[cfg(not(test))]
use core::ffi::c_void;

#[cfg(test)]
use std::ffi::c_void;

pub use libc::{c_char, c_int, c_long, c_ulong, size_t};

/// Kernel pointer type (matches C void*)
pub type KernelPtr = *mut c_void;

/// GFP (Get Free Page) flags matching Linux kernel
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct GfpFlags(pub u32);

impl GfpFlags {
    pub const GFP_KERNEL: Self = GfpFlags(0x0cc0);
    pub const GFP_ATOMIC: Self = GfpFlags(0x0020);
    pub const GFP_NOWAIT: Self = GfpFlags(0x0000);
    pub const __GFP_ZERO: Self = GfpFlags(0x8000);
}

/// Error codes matching Linux kernel errno values
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KernelError {
    EPERM = 1,
    ENOENT = 2,
    EINTR = 4,
    EIO = 5,
    ENOMEM = 12,
    EACCES = 13,
    EFAULT = 14,
    EBUSY = 16,
    EEXIST = 17,
    EINVAL = 22,
    ENOSPC = 28,
    EAGAIN = 11,
}

impl KernelError {
    pub fn to_errno(self) -> c_int {
        -(self as c_int)
    }
}

/// Result type for kernel operations
pub type KernelResult<T> = Result<T, KernelError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gfp_flags() {
        assert_eq!(GfpFlags::GFP_KERNEL.0, 0x0cc0);
        assert_eq!(GfpFlags::GFP_ATOMIC.0, 0x0020);
    }

    #[test]
    fn test_kernel_error_errno() {
        assert_eq!(KernelError::ENOMEM.to_errno(), -12);
        assert_eq!(KernelError::EINVAL.to_errno(), -22);
    }
}
