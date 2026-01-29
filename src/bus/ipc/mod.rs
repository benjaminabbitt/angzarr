//! IPC-based event bus for embedded multi-process mode.
//!
//! Substitutes for Kafka/RabbitMQ in embedded mode using UDS and named pipes.
//!
//! Architecture (SNS/SQS-like):
//! ```text
//! ┌─────────────┐     ┌─────────────┐
//! │  Aggregate  │────▶│   Broker    │
//! │  (publish)  │ UDS │  (fanout)   │
//! └─────────────┘     └─────────────┘
//!                           │
//!          ┌────────────────┼────────────────┐
//!          ▼                ▼                ▼
//!    ┌──────────┐     ┌──────────┐     ┌──────────┐
//!    │ pipe-A   │     │ pipe-B   │     │ pipe-C   │
//!    │(projector)│    │ (saga)   │     │(projector)│
//!    └──────────┘     └──────────┘     └──────────┘
//! ```
//!
//! Usage:
//! 1. Orchestrator creates `IpcBroker` and registers subscribers
//! 2. Orchestrator calls `broker.run()` to start listening
//! 3. Aggregates use `IpcEventBus::publisher()` to connect and publish
//! 4. Projectors/sagas use `IpcEventBus::subscriber()` to read from pipes

mod broker;
pub(crate) mod checkpoint;
mod client;

pub use broker::{IpcBroker, IpcBrokerConfig, SubscriberInfo};
pub use client::{IpcConfig, IpcEventBus, SUBSCRIBERS_ENV_VAR};

/// Default base path for IPC sockets/pipes.
pub const DEFAULT_BASE_PATH: &str = "/tmp/angzarr";

/// Socket name for the broker.
pub const BROKER_SOCKET: &str = "event-broker.sock";

/// Pipe prefix for subscribers.
pub const SUBSCRIBER_PIPE_PREFIX: &str = "subscriber-";
