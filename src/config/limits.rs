//! Resource limits for message processing and query results.
//!
//! Defaults are set to the most restrictive bus (AWS SQS/SNS: 256 KB).
//! Override in config for bus backends with higher limits.
//!
//! # Bus Message Size Limits (Researched: 2026-02-09)
//!
//! | Bus        | Max Message Size | Notes                           |
//! |------------|------------------|---------------------------------|
//! | SQS/SNS    | 256 KB           | Hard AWS limit                  |
//! | IPC        | 10 MB            | Configurable in code            |
//! | Pub/Sub    | 10 MB            | Google Cloud service limit      |
//! | Kafka      | 1-10 MB          | Broker-configurable             |
//! | AMQP       | 128 MB           | RabbitMQ default                |
//! | Channel    | Unlimited        | Memory-bound (in-process only)  |
//!
//! *Verify current service limits before deployment - cloud provider
//! limits may change.*

use serde::Deserialize;

/// Default maximum payload size per command/event page (256 KB - SQS/SNS limit).
pub const DEFAULT_MAX_PAYLOAD_BYTES: usize = 256 * 1024;

/// Default maximum pages per CommandBook/EventBook.
pub const DEFAULT_MAX_PAGES_PER_BOOK: usize = 100;

/// Default maximum events returned per query.
pub const DEFAULT_QUERY_RESULT_LIMIT: usize = 1000;

/// Default channel bus capacity.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

/// Resource limits for message processing.
///
/// Defaults are set to the most restrictive bus (AWS SQS/SNS: 256 KB).
/// Override for specific bus backends:
///
/// | Bus        | Max Message Size | Notes                    |
/// |------------|------------------|--------------------------|
/// | SQS/SNS    | 256 KB           | Hard AWS limit           |
/// | IPC        | 10 MB            | Configurable in code     |
/// | Pub/Sub    | 10 MB            | Google service limit     |
/// | Kafka      | 1-10 MB          | Broker-configurable      |
/// | AMQP       | 128 MB           | RabbitMQ default         |
/// | Channel    | Unlimited        | Memory-bound             |
///
/// *Limits researched: 2026-02-09. Verify current service limits before deployment.*
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ResourceLimits {
    /// Maximum payload size per command/event page in bytes.
    ///
    /// Default: 262,144 (256 KB - SQS/SNS limit).
    /// Increase to 10,485,760 (10 MB) for IPC/Pub/Sub/Kafka deployments.
    pub max_payload_bytes: usize,

    /// Maximum pages per CommandBook/EventBook.
    ///
    /// Default: 100. Limits memory usage and processing time.
    pub max_pages_per_book: usize,

    /// Maximum events returned per query.
    ///
    /// Default: 1,000. Prevents unbounded result sets.
    pub query_result_limit: usize,

    /// Channel bus message queue capacity.
    ///
    /// Default: 1,024. Only applies to in-process channel bus.
    pub channel_capacity: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_BYTES,
            max_pages_per_book: DEFAULT_MAX_PAGES_PER_BOOK,
            query_result_limit: DEFAULT_QUERY_RESULT_LIMIT,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
        }
    }
}

impl ResourceLimits {
    /// Create limits optimized for IPC/local deployments (10 MB payload).
    pub fn for_ipc() -> Self {
        Self {
            max_payload_bytes: 10 * 1024 * 1024, // 10 MB
            ..Default::default()
        }
    }

    /// Create limits optimized for AWS SQS/SNS (256 KB payload).
    pub fn for_aws() -> Self {
        Self::default()
    }

    /// Create limits optimized for Google Pub/Sub (10 MB payload).
    pub fn for_pubsub() -> Self {
        Self {
            max_payload_bytes: 10 * 1024 * 1024, // 10 MB
            ..Default::default()
        }
    }

    /// Create limits optimized for Kafka (1 MB default, adjustable).
    pub fn for_kafka() -> Self {
        Self {
            max_payload_bytes: 1024 * 1024, // 1 MB (Kafka default)
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_payload_bytes, 256 * 1024); // 256 KB
        assert_eq!(limits.max_pages_per_book, 100);
        assert_eq!(limits.query_result_limit, 1000);
        assert_eq!(limits.channel_capacity, 1024);
    }

    #[test]
    fn test_ipc_limits() {
        let limits = ResourceLimits::for_ipc();
        assert_eq!(limits.max_payload_bytes, 10 * 1024 * 1024); // 10 MB
        assert_eq!(limits.max_pages_per_book, 100); // Others unchanged
    }

    #[test]
    fn test_aws_limits() {
        let limits = ResourceLimits::for_aws();
        assert_eq!(limits.max_payload_bytes, 256 * 1024); // 256 KB
    }

    #[test]
    fn test_pubsub_limits() {
        let limits = ResourceLimits::for_pubsub();
        assert_eq!(limits.max_payload_bytes, 10 * 1024 * 1024); // 10 MB
    }

    #[test]
    fn test_kafka_limits() {
        let limits = ResourceLimits::for_kafka();
        assert_eq!(limits.max_payload_bytes, 1024 * 1024); // 1 MB
    }
}
