//! Event bus implementations.

pub mod amqp;
pub mod direct;

pub use amqp::{AmqpConfig, AmqpEventBus};
pub use direct::{DirectEventBus, ProjectorConfig, SagaConfig};
