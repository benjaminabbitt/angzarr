//! Orchestration layer for unified standalone and distributed execution.
//!
//! Each sub-module defines a trait (interface) in `mod.rs` with shared orchestration logic.
//! Transport-specific implementations live in `local/` (in-process) and `grpc/` (remote)
//! subdirectories, named by their key differentiating factor.

pub mod aggregate;
pub mod command;
pub mod correlation;
pub mod destination;
pub mod process_manager;
pub mod projector;
pub mod saga;
pub mod shared;
