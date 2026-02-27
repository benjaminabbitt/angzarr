//! Step definitions for acceptance tests.

pub mod player;

// Re-export step functions for cucumber registration.
// These are used by cucumber-rs macro registration, not direct imports.
#[allow(unused_imports)]
pub use player::*;
