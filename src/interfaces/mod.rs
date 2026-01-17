//! Abstract interfaces for angzarr components.
//!
//! These traits define the contracts for:
//! - Event storage (persistence)
//! - Snapshot storage (optimization)
//! - Event bus (async delivery)
//! - Business logic client (BC communication)
//! - Projectors (in-process read model builders)
//! - Sagas (in-process cross-aggregate coordination)

pub mod business_client;
pub mod event_bus;
pub mod event_store;
pub mod projector;
pub mod saga;
pub mod snapshot_store;

pub use business_client::{BusinessError, BusinessLogicClient};
pub use event_bus::{BusError, EventBus, EventHandler, PublishResult};
pub use event_store::{EventStore, StorageError};
pub use projector::{Projector, ProjectorError};
pub use saga::{Saga, SagaError};
pub use snapshot_store::SnapshotStore;
