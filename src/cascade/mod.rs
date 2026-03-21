//! Cascade (2PC) support for atomic multi-aggregate transactions.
//!
//! This module provides:
//! - `CascadeReaper`: Background task for cleaning up stale cascades
//!
//! # Background
//!
//! When `execute_atomic()` is used, events are written with `committed=false`.
//! On success, a `Confirmation` event is written; on failure, a `Revocation`.
//! If the process crashes mid-cascade, uncommitted events remain in storage.
//!
//! The `CascadeReaper` periodically scans for stale cascades (uncommitted events
//! older than a threshold) and writes `Revocation` events to clean them up.
//!
//! # Example
//!
//! ```ignore
//! use angzarr::cascade::CascadeReaper;
//! use std::time::Duration;
//!
//! let reaper = CascadeReaper::new(event_store, Duration::from_secs(300))
//!     .with_interval(Duration::from_secs(60));
//!
//! // Spawn as background task
//! let handle = reaper.spawn();
//! ```

mod reaper;

pub use reaper::CascadeReaper;
