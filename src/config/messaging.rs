//! Messaging and event bus configuration types.

use serde::Deserialize;

/// Messaging type discriminator.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessagingType {
    /// Direct in-process messaging (no external broker).
    #[default]
    Direct,
    /// AMQP/RabbitMQ messaging.
    Amqp,
}

/// Messaging configuration (discriminated union).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MessagingConfig {
    /// Messaging type discriminator.
    #[serde(rename = "type")]
    pub messaging_type: MessagingType,
    /// AMQP-specific configuration.
    pub amqp: AmqpConfig,
}

impl Default for MessagingConfig {
    fn default() -> Self {
        Self {
            messaging_type: MessagingType::Direct,
            amqp: AmqpConfig::default(),
        }
    }
}

/// AMQP-specific configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AmqpConfig {
    /// AMQP connection URL.
    pub url: String,
    /// Domain to subscribe to (for aggregate mode, this is the command queue).
    pub domain: Option<String>,
    /// Domains to subscribe to (for projector/saga modes).
    pub domains: Option<Vec<String>>,
}

impl Default for AmqpConfig {
    fn default() -> Self {
        Self {
            url: "amqp://localhost:5672".to_string(),
            domain: None,
            domains: None,
        }
    }
}
