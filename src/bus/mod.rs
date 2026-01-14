//! Event bus implementations.

pub mod direct;

#[cfg(feature = "amqp")]
pub mod amqp;

pub use direct::{DirectEventBus, ProjectorConfig, SagaConfig};

#[cfg(feature = "amqp")]
pub use amqp::{AmqpConfig, AmqpEventBus};
