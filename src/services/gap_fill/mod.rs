//! Handler-relative gap detection and filling for EventBooks.
//!
//! Replaces `EventBookRepairer` with handler-aware completeness checking.
//! An EventBook is "complete" for a handler if it contains all events from
//! the handler's checkpoint + 1 to the book's max sequence.

mod analysis;
mod error;
mod filler;

pub use analysis::{analyze_gap, GapAnalysis};
pub use error::{GapFillError, Result};
pub use filler::{
    EventSource, GapFiller, HandlerPositionStore, LocalEventSource, NoOpPositionStore,
    PositionStoreAdapter, RemoteEventSource,
};
