//! Kernel synchronization primitives
//!
//! This module provides Rust implementations of kernel synchronization
//! primitives like spinlocks, mutexes, and semaphores.

#![cfg_attr(not(test), no_std)]

pub mod spinlock;

pub use spinlock::*;

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        assert!(true);
    }
}
