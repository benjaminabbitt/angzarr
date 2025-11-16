//! Linux Kernel ABI Compatibility Adapter
//!
//! This crate provides the adapter/boundary layer between Angzarr's internal
//! Rust API and the Linux kernel's C ABI. It translates between the two without
//! constraining Angzarr's internal design.
//!
//! # Architecture
//!
//! ```text
//! Linux C Code
//!      ↓
//! Linux Compat Layer (this crate) - Translation boundary
//!      ↓
//! Angzarr Core (pure Rust)
//! ```
//!
//! # Design Principles
//!
//! - **Zero Runtime Cost**: Translation should be zero-cost
//! - **Perfect ABI Match**: Must match Linux kernel exactly
//! - **Safety Boundary**: All unsafe code isolated here
//! - **No Business Logic**: Only translation, no implementation
//! - **Stable Interface**: Linux ABI never changes
//!
//! # Usage
//!
//! C code can include Linux-compatible headers and use familiar APIs:
//!
//! ```c
//! #include <linux/list.h>
//! #include <linux/rbtree.h>
//!
//! struct list_head my_list;
//! INIT_LIST_HEAD(&my_list);
//! list_add(new_entry, &my_list);
//! ```
//!
//! The adapter translates these calls to Angzarr's safe Rust API internally.

#![cfg_attr(not(test), no_std)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

pub mod list;
pub mod rbtree;
pub mod error;
pub mod types;

// Re-export for convenience
pub use list::list_head;
pub use rbtree::{rb_node, rb_root};
pub use error::errno_to_result;
pub use types::*;

/// Marker module to identify Linux compatibility layer
pub mod linux_compat {
    //! This module exists to identify code that is part of the Linux
    //! compatibility boundary. Code here should:
    //!
    //! - Match Linux behavior exactly
    //! - Use #[repr(C)] for all structs
    //! - Export with #[no_mangle]
    //! - Use extern "C" calling convention
    //! - Translate to Angzarr's safe APIs
}
