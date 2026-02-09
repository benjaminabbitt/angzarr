//! Timeout Scheduler for Process Managers.
//!
//! Queries for stale process manager instances and emits ProcessTimeout events.
//! Runs as a separate service, typically as a CronJob or long-running daemon.
//!
//! ## Architecture
//!
//! The scheduler:
//! 1. Queries a read model for process instances past their deadline
//! 2. Emits ProcessTimeout events to the event bus
//! 3. Process managers handle timeout events like any other event
//!
//! ## Read Model Requirement
//!
//! The scheduler requires a queryable view of process manager state.
//! This is typically maintained by a projector that subscribes to PM events
//! and stores current state with deadlines.

use std::sync::Arc;
use std::time::Duration;

use prost_types::Timestamp;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::bus::EventBus;
use crate::config::TimeoutConfig;
use crate::proto::{Cover, EventBook, EventPage, ProcessTimeout, Uuid as ProtoUuid};

/// Represents a process instance that has timed out.
#[derive(Debug, Clone)]
pub struct StaleProcess {
    /// Correlation ID of the workflow.
    pub correlation_id: String,
    /// Process type (e.g., "order-fulfillment").
    pub process_type: String,
    /// Timeout type (e.g., "payment", "reservation").
    pub timeout_type: String,
    /// When the timeout was supposed to fire.
    pub deadline: chrono::DateTime<chrono::Utc>,
}

/// Trait for querying stale process instances.
///
/// Implementations query a read model (projection) of process manager state.
#[async_trait::async_trait]
pub trait StaleProcessQuery: Send + Sync {
    /// Find all process instances past their deadline for a given timeout type.
    async fn find_stale(
        &self,
        process_type: &str,
        timeout_type: &str,
        max_age: Duration,
    ) -> Result<Vec<StaleProcess>, Box<dyn std::error::Error + Send + Sync>>;
}

/// Timeout scheduler configuration.
#[derive(Debug, Clone)]
pub struct TimeoutSchedulerConfig {
    /// Process type this scheduler handles.
    pub process_type: String,
    /// Domain for timeout events.
    pub timeout_domain: String,
    /// Timeout configurations by type.
    pub timeouts: Vec<(String, TimeoutConfig)>,
    /// How often to check for timeouts.
    pub check_interval: Duration,
}

/// Timeout scheduler service.
///
/// Periodically queries for stale processes and emits timeout events.
pub struct TimeoutScheduler {
    config: TimeoutSchedulerConfig,
    query: Arc<dyn StaleProcessQuery>,
    publisher: Arc<dyn EventBus>,
}

impl TimeoutScheduler {
    /// Create a new timeout scheduler.
    pub fn new(
        config: TimeoutSchedulerConfig,
        query: Arc<dyn StaleProcessQuery>,
        publisher: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            config,
            query,
            publisher,
        }
    }

    /// Run the scheduler loop.
    ///
    /// This runs indefinitely, checking for timeouts at the configured interval.
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            process_type = %self.config.process_type,
            check_interval = ?self.config.check_interval,
            timeout_types = self.config.timeouts.len(),
            "Starting timeout scheduler"
        );

        let mut ticker = interval(self.config.check_interval);

        loop {
            ticker.tick().await;

            for (timeout_type, timeout_config) in &self.config.timeouts {
                let max_age = Duration::from_secs(timeout_config.duration_minutes as u64 * 60);

                match self
                    .query
                    .find_stale(&self.config.process_type, timeout_type, max_age)
                    .await
                {
                    Ok(stale_processes) => {
                        if !stale_processes.is_empty() {
                            info!(
                                process_type = %self.config.process_type,
                                timeout_type = %timeout_type,
                                count = stale_processes.len(),
                                "Found stale processes"
                            );
                        }

                        for process in stale_processes {
                            if let Err(e) = self.emit_timeout(&process).await {
                                error!(
                                    correlation_id = %process.correlation_id,
                                    error = %e,
                                    "Failed to emit timeout event"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            process_type = %self.config.process_type,
                            timeout_type = %timeout_type,
                            error = %e,
                            "Failed to query stale processes"
                        );
                    }
                }
            }
        }
    }

    /// Emit a ProcessTimeout event for a stale process.
    async fn emit_timeout(
        &self,
        process: &StaleProcess,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let timeout_event = ProcessTimeout {
            correlation_id: process.correlation_id.clone(),
            process_type: process.process_type.clone(),
            timeout_type: process.timeout_type.clone(),
            deadline: Some(Timestamp {
                seconds: process.deadline.timestamp(),
                nanos: process.deadline.timestamp_subsec_nanos() as i32,
            }),
        };

        // Wrap in EventPage
        let event_page = EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Force(true)),
            created_at: Some(Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: chrono::Utc::now().timestamp_subsec_nanos() as i32,
            }),
            event: Some(prost_types::Any {
                type_url: "type.googleapis.com/angzarr.ProcessTimeout".to_string(),
                value: prost::Message::encode_to_vec(&timeout_event),
            }),
        };

        // Create EventBook for the timeout domain
        // Use correlation_id as root UUID (hashed if not already UUID)
        let root_uuid = correlation_to_uuid(&process.correlation_id);

        let event_book = EventBook {
            cover: Some(Cover {
                domain: self.config.timeout_domain.clone(),
                root: Some(ProtoUuid {
                    value: root_uuid.as_bytes().to_vec(),
                }),
                correlation_id: process.correlation_id.clone(),
                edition: None,
            }),
            pages: vec![event_page],
            snapshot: None,
            next_sequence: 0,
        };

        debug!(
            correlation_id = %process.correlation_id,
            timeout_type = %process.timeout_type,
            "Emitting ProcessTimeout event"
        );

        self.publisher
            .publish(Arc::new(event_book))
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        Ok(())
    }
}

/// Convert correlation_id to UUID.
///
/// If already a valid UUID, returns it directly.
/// Otherwise, hashes to UUID v5 for deterministic mapping.
fn correlation_to_uuid(correlation_id: &str) -> uuid::Uuid {
    // Try parsing as UUID first
    if let Ok(uuid) = uuid::Uuid::parse_str(correlation_id) {
        return uuid;
    }

    // Hash to UUID v5 (deterministic)
    // Using a namespace UUID specific to process managers
    const PROCESS_MANAGER_NAMESPACE: uuid::Uuid = uuid::Uuid::from_bytes([
        0x6b, 0xa7, 0xb8, 0x14, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);

    uuid::Uuid::new_v5(&PROCESS_MANAGER_NAMESPACE, correlation_id.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlation_to_uuid_valid_uuid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = correlation_to_uuid(uuid_str);
        assert_eq!(result.to_string(), uuid_str);
    }

    #[test]
    fn test_correlation_to_uuid_string_deterministic() {
        let corr_id = "order-123";
        let result1 = correlation_to_uuid(corr_id);
        let result2 = correlation_to_uuid(corr_id);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_correlation_to_uuid_different_strings() {
        let result1 = correlation_to_uuid("order-123");
        let result2 = correlation_to_uuid("order-456");
        assert_ne!(result1, result2);
    }
}
