//! Domain repositories.

mod event_book;
mod snapshot;

pub use event_book::EventBookRepository;
pub use snapshot::SnapshotRepository;
