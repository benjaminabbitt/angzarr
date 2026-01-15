//! gRPC service implementations.

pub mod entity;
pub mod event_book_repair;
pub mod event_query;
pub mod projector_coord;
pub mod saga_compensation;
pub mod saga_coord;

pub use entity::EntityService;
pub use event_book_repair::{EventBookRepairer, RepairError};
pub use event_query::EventQueryService;
pub use projector_coord::{ProjectorCoordinatorService, ProjectorEndpoint};
pub use saga_compensation::{
    build_compensation_failed_event, build_compensation_failed_event_book, build_revoke_command,
    build_revoke_command_book, handle_business_response, CompensationContext, CompensationError,
    CompensationOutcome,
};
pub use saga_coord::{SagaCoordinatorService, SagaEndpoint};
