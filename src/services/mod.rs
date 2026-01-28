//! gRPC service implementations.

pub mod aggregate;
pub mod event_book_repair;
pub mod event_query;
pub mod projector_coord;
pub mod snapshot_handler;
pub mod timeout_scheduler;
pub mod upcaster;

pub use crate::utils::saga_compensation::{
    build_compensation_failed_event, build_compensation_failed_event_book, build_revoke_command,
    build_revoke_command_book, handle_business_response, CompensationContext, CompensationError,
    CompensationOutcome,
};
pub use aggregate::AggregateService;
pub use event_book_repair::{EventBookRepairer, RepairError};
pub use event_query::EventQueryService;
pub use projector_coord::ProjectorCoordinatorService;
pub use timeout_scheduler::{
    StaleProcess, StaleProcessQuery, TimeoutScheduler, TimeoutSchedulerConfig,
};
pub use upcaster::{Upcaster, UpcasterConfig};
