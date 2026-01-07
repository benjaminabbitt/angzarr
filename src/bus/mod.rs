//! Event bus implementations.

pub mod direct;
pub mod in_process;

#[cfg(feature = "amqp")]
pub mod amqp;

pub use direct::{DirectEventBus, ProjectorConfig, SagaConfig};
pub use in_process::InProcessEventBus;

#[cfg(feature = "amqp")]
pub use amqp::{AmqpConfig, AmqpEventBus};
