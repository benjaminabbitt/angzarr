//! Core kernel types and utilities
//!
//! This crate provides fundamental types used throughout the kernel.

#![cfg_attr(not(test), no_std)]

pub mod types;

pub use types::*;

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        assert!(true);
    }
}
