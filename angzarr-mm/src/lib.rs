//! Memory management subsystem
//!
//! This module provides memory allocation and management primitives.

#![cfg_attr(not(test), no_std)]

pub mod allocator;

pub use allocator::*;
