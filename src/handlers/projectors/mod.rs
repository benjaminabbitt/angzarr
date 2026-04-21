//! Projector implementations that live inside core.
//!
//! The log, event, and cloudevents (outbound) projectors have been
//! extracted to their own dedicated repos:
//! - angzarr-prj-log
//! - angzarr-prj-event
//! - angzarr-prj-cloudevents
//!
//! Each ships its own binary and image. Core no longer re-exports or
//! path-depends on them; build and deploy from those repos directly.

pub mod stream;

pub use stream::StreamService;
