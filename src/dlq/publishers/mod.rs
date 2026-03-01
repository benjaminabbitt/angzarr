//! DLQ publisher implementations.
//!
//! Each publisher handles a specific transport backend (AMQP, Kafka, etc.).

mod channel;
mod filesystem;
mod logging;
mod noop;
mod offload;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
mod database;

#[cfg(feature = "amqp")]
mod amqp;
#[cfg(feature = "kafka")]
mod kafka;
#[cfg(feature = "pubsub")]
mod pubsub;
#[cfg(feature = "sns-sqs")]
mod sns_sqs;

pub use channel::ChannelDeadLetterPublisher;
pub use filesystem::FilesystemDeadLetterPublisher;
pub use logging::LoggingDeadLetterPublisher;
pub use noop::NoopDeadLetterPublisher;
pub use offload::OffloadFilesystemDlqPublisher;

#[cfg(feature = "gcs")]
pub use offload::OffloadGcsDlqPublisher;
#[cfg(feature = "s3")]
pub use offload::OffloadS3DlqPublisher;

#[cfg(feature = "postgres")]
pub use database::PostgresDlqPublisher;
#[cfg(feature = "sqlite")]
pub use database::SqliteDlqPublisher;

#[cfg(feature = "amqp")]
pub use amqp::AmqpDeadLetterPublisher;
#[cfg(feature = "kafka")]
pub use kafka::KafkaDeadLetterPublisher;
#[cfg(feature = "pubsub")]
pub use pubsub::PubSubDeadLetterPublisher;
#[cfg(feature = "sns-sqs")]
pub use sns_sqs::SnsSqsDeadLetterPublisher;
