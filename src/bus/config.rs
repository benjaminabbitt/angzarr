//! Bus configuration types.

use serde::Deserialize;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
use super::outbox;
use crate::dlq::config::DlqConfig;

/// Messaging configuration.
///
/// The `messaging_type` field is a string that identifies which backend to use.
/// Each backend module checks if the type matches and handles creation.
///
/// Known types: "amqp", "kafka", "channel", "ipc", "nats", "pubsub", "sns-sqs"
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MessagingConfig {
    /// Messaging type identifier (e.g., "amqp", "kafka", "channel").
    #[serde(rename = "type")]
    pub messaging_type: String,
    /// AMQP-specific configuration.
    pub amqp: AmqpBusConfig,
    /// Kafka-specific configuration.
    pub kafka: KafkaConfig,
    /// IPC-specific configuration (for embedded mode).
    #[cfg(unix)]
    pub ipc: IpcBusConfig,
    /// NATS-specific configuration.
    pub nats: NatsBusConfig,
    /// Google Pub/Sub-specific configuration.
    pub pubsub: PubSubBusConfig,
    /// AWS SNS/SQS-specific configuration.
    pub sns_sqs: SnsSqsBusConfig,
    /// Outbox pattern configuration for guaranteed delivery.
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    pub outbox: outbox::OutboxConfig,
    /// Dead letter queue configuration.
    pub dlq: DlqConfig,
}

impl Default for MessagingConfig {
    fn default() -> Self {
        Self {
            messaging_type: "channel".to_string(),
            amqp: AmqpBusConfig::default(),
            kafka: KafkaConfig::default(),
            #[cfg(unix)]
            ipc: IpcBusConfig::default(),
            nats: NatsBusConfig::default(),
            pubsub: PubSubBusConfig::default(),
            sns_sqs: SnsSqsBusConfig::default(),
            #[cfg(any(feature = "postgres", feature = "sqlite"))]
            outbox: outbox::OutboxConfig::default(),
            dlq: DlqConfig::default(),
        }
    }
}

/// Mode for event bus initialization.
#[derive(Debug, Clone)]
pub enum EventBusMode {
    /// Publisher-only mode (no consuming).
    Publisher,
    /// Subscriber mode for a specific domain.
    Subscriber {
        /// Queue/group name.
        queue: String,
        /// Domain to subscribe to.
        domain: String,
    },
    /// Subscriber mode for all domains.
    SubscriberAll {
        /// Queue/group name.
        queue: String,
    },
}

// ============================================================================
// Backend-specific configurations
// ============================================================================

/// AMQP-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AmqpBusConfig {
    /// AMQP connection URL.
    pub url: String,
    /// Domain to subscribe to (for aggregate mode, this is the command queue).
    pub domain: Option<String>,
    /// Domains to subscribe to (for projector/saga modes).
    pub domains: Option<Vec<String>>,
}

impl Default for AmqpBusConfig {
    fn default() -> Self {
        Self {
            url: "amqp://localhost:5672".to_string(),
            domain: None,
            domains: None,
        }
    }
}

/// Kafka-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KafkaConfig {
    /// Kafka bootstrap servers (comma-separated).
    pub bootstrap_servers: String,
    /// Topic prefix for events.
    pub topic_prefix: String,
    /// Consumer group ID.
    pub group_id: Option<String>,
    /// Domains to subscribe to (for consumers).
    pub domains: Option<Vec<String>>,
    /// SASL username (optional, for authenticated clusters).
    pub sasl_username: Option<String>,
    /// SASL password (optional, for authenticated clusters).
    pub sasl_password: Option<String>,
    /// SASL mechanism (PLAIN, SCRAM-SHA-256, SCRAM-SHA-512).
    pub sasl_mechanism: Option<String>,
    /// Security protocol (PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL).
    pub security_protocol: Option<String>,
    /// SSL CA certificate path (for SSL connections).
    pub ssl_ca_location: Option<String>,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            topic_prefix: "angzarr".to_string(),
            group_id: None,
            domains: None,
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: None,
            ssl_ca_location: None,
        }
    }
}

/// IPC-specific configuration (for embedded mode).
#[cfg(unix)]
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct IpcBusConfig {
    /// Base path for pipes.
    pub base_path: String,
    /// Subscriber name (for subscriber mode).
    pub subscriber_name: Option<String>,
    /// Single domain to subscribe to (simpler env var).
    pub domain: Option<String>,
    /// Domains to subscribe to (for subscriber mode) - comma-separated when set via env var.
    pub domains: Option<Vec<String>>,
}

#[cfg(unix)]
impl IpcBusConfig {
    /// Get domains as a Vec, preferring `domains` over `domain`.
    pub fn get_domains(&self) -> Vec<String> {
        self.domains
            .clone()
            .or_else(|| {
                self.domain.as_ref().map(|d| {
                    // Support comma-separated domains in the single domain field
                    d.split(',').map(|s| s.trim().to_string()).collect()
                })
            })
            .unwrap_or_default()
    }
}

#[cfg(unix)]
impl Default for IpcBusConfig {
    fn default() -> Self {
        Self {
            base_path: "/tmp/angzarr".to_string(),
            subscriber_name: None,
            domain: None,
            domains: None,
        }
    }
}

/// NATS JetStream-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NatsBusConfig {
    /// NATS server URL.
    pub url: String,
    /// Stream prefix for topics.
    pub stream_prefix: String,
    /// Consumer name for subscriptions.
    pub consumer_name: Option<String>,
    /// Number of stream replicas.
    pub replicas: u32,
    /// Retention policy: "limits", "interest", "workqueue".
    pub retention: String,
    /// Maximum age for messages in hours.
    pub max_age_hours: u64,
    /// Domains to subscribe to.
    pub domains: Option<Vec<String>>,
}

impl Default for NatsBusConfig {
    fn default() -> Self {
        Self {
            url: "nats://localhost:4222".to_string(),
            stream_prefix: "angzarr".to_string(),
            consumer_name: None,
            replicas: 1,
            retention: "limits".to_string(),
            max_age_hours: 168, // 7 days
            domains: None,
        }
    }
}

/// Google Pub/Sub-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PubSubBusConfig {
    /// GCP project ID.
    pub project_id: String,
    /// Topic prefix for events.
    pub topic_prefix: String,
    /// Subscription ID for consuming.
    pub subscription_id: Option<String>,
    /// Domains to subscribe to.
    pub domains: Option<Vec<String>>,
}

impl Default for PubSubBusConfig {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            topic_prefix: "angzarr".to_string(),
            subscription_id: None,
            domains: None,
        }
    }
}

/// AWS SNS/SQS-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SnsSqsBusConfig {
    /// AWS region.
    pub region: Option<String>,
    /// Topic prefix for SNS topics.
    pub topic_prefix: String,
    /// Subscription ID for SQS queue naming.
    pub subscription_id: Option<String>,
    /// Domains to subscribe to.
    pub domains: Option<Vec<String>>,
}

impl Default for SnsSqsBusConfig {
    fn default() -> Self {
        Self {
            region: None,
            topic_prefix: "angzarr".to_string(),
            subscription_id: None,
            domains: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // IpcBusConfig::get_domains Tests
    // ============================================================================
    //
    // The get_domains() method resolves domain subscriptions from two sources:
    // - `domains`: explicit Vec<String> (preferred)
    // - `domain`: single string supporting comma-separated values
    //
    // This dual-source pattern supports both YAML config (domains array) and
    // env vars (single comma-separated string). Tests ensure both paths work.

    #[cfg(unix)]
    mod ipc_config {
        use super::*;

        /// When domains vec is set, use it directly (preferred source).
        ///
        /// YAML config typically sets this directly as an array.
        #[test]
        fn test_get_domains_prefers_domains_over_domain() {
            let config = IpcBusConfig {
                domains: Some(vec!["player".to_string(), "table".to_string()]),
                domain: Some("hand".to_string()), // Should be ignored
                ..Default::default()
            };

            let result = config.get_domains();

            assert_eq!(result, vec!["player", "table"]);
        }

        /// When only domain is set, use it as fallback.
        ///
        /// Simpler env var path: ANGZARR_IPC_DOMAIN=player
        #[test]
        fn test_get_domains_falls_back_to_domain() {
            let config = IpcBusConfig {
                domains: None,
                domain: Some("player".to_string()),
                ..Default::default()
            };

            let result = config.get_domains();

            assert_eq!(result, vec!["player"]);
        }

        /// Comma-separated values in domain field are split.
        ///
        /// Env var can specify multiple: ANGZARR_IPC_DOMAIN=player,table,hand
        #[test]
        fn test_get_domains_splits_comma_separated() {
            let config = IpcBusConfig {
                domains: None,
                domain: Some("player,table,hand".to_string()),
                ..Default::default()
            };

            let result = config.get_domains();

            assert_eq!(result, vec!["player", "table", "hand"]);
        }

        /// Whitespace around commas is trimmed.
        ///
        /// User-friendly: "player, table, hand" works same as "player,table,hand"
        #[test]
        fn test_get_domains_trims_whitespace() {
            let config = IpcBusConfig {
                domains: None,
                domain: Some("player , table , hand".to_string()),
                ..Default::default()
            };

            let result = config.get_domains();

            assert_eq!(result, vec!["player", "table", "hand"]);
        }

        /// When neither domains nor domain is set, return empty vec.
        ///
        /// Publisher-only mode: no subscriptions needed.
        #[test]
        fn test_get_domains_returns_empty_when_none_set() {
            let config = IpcBusConfig {
                domains: None,
                domain: None,
                ..Default::default()
            };

            let result = config.get_domains();

            assert!(result.is_empty());
        }

        /// Empty domains vec is returned as-is (not fallen back to domain).
        ///
        /// Explicit empty array means "subscribe to nothing", not "check domain field".
        #[test]
        fn test_get_domains_empty_vec_is_explicit() {
            let config = IpcBusConfig {
                domains: Some(vec![]),
                domain: Some("player".to_string()), // Should still be ignored
                ..Default::default()
            };

            let result = config.get_domains();

            assert!(result.is_empty());
        }
    }
}
