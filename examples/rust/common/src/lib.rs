//! Common utilities for Angzarr example implementations.

use angzarr::proto::{event_page::Sequence, EventBook};

pub mod identity;
pub mod proto;
pub mod server;

pub use server::{
    init_tracing, run_aggregate_server, run_saga_server, AggregateWrapper, SagaLogic, SagaWrapper,
};

/// Get the next sequence number for new events based on prior EventBook state.
///
/// Examines the EventBook to find the highest existing sequence:
/// - If pages exist, uses the last page's sequence + 1
/// - If only snapshot exists, uses snapshot.sequence + 1
/// - If empty/None, returns 0
pub fn next_sequence(event_book: Option<&EventBook>) -> u32 {
    let Some(book) = event_book else {
        return 0;
    };

    // Check last event page first (most recent)
    if let Some(last_page) = book.pages.last() {
        if let Some(Sequence::Num(n)) = &last_page.sequence {
            return n + 1;
        }
    }

    // Fall back to snapshot sequence
    // snapshot.sequence is the last event sequence used to create the snapshot
    if let Some(snapshot) = &book.snapshot {
        return snapshot.sequence + 1;
    }

    0
}
