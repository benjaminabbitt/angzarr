//! gRPC service implementations.

pub mod aggregate;
pub mod event_book_repair;
pub mod event_query;
pub mod projector_coord;
pub(crate) mod repairable;
pub mod saga_coord;
pub mod snapshot_handler;

pub use aggregate::AggregateService;
pub use event_book_repair::{EventBookRepairer, RepairError};
pub use event_query::EventQueryService;
pub use crate::clients::ProjectorEndpoint;
pub use projector_coord::ProjectorCoordinatorService;
pub use crate::utils::saga_compensation::{
    build_compensation_failed_event, build_compensation_failed_event_book, build_revoke_command,
    build_revoke_command_book, handle_business_response, CompensationContext, CompensationError,
    CompensationOutcome,
};
pub use crate::clients::SagaEndpoint;
pub use saga_coord::SagaCoordinatorService;
