//! Topology projector: builds a graph of runtime components from the event stream.
//!
//! Subscribes to all domains and discovers nodes (aggregates, sagas, process managers,
//! projectors) and edges (command/event flows via correlation_id chains). Serves the
//! graph via REST for Grafana's Node Graph panel.
//!
//! Pure event-stream observation — works identically in standalone and distributed modes.

#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub mod schema;
pub mod store;
pub mod rest;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::proto::EventBook;

use store::{TopologyError, TopologyStore};

/// Topology projector that discovers runtime component graph from the event stream.
///
/// In standalone mode, register via `RuntimeBuilder::register_projector` (implements
/// `ProjectorHandler` when sqlite feature is enabled). In distributed mode, the
/// `angzarr-topology` binary calls `process_event` directly from its `EventHandler`.
pub struct TopologyProjector {
    store: Arc<dyn TopologyStore>,
    rest_port: u16,
}

impl TopologyProjector {
    /// Create a new topology projector.
    ///
    /// - `store`: Backing store for topology data (SQLite or Postgres).
    /// - `rest_port`: Port for the REST API serving Grafana Node Graph data.
    pub fn new(store: Arc<dyn TopologyStore>, rest_port: u16) -> Self {
        Self { store, rest_port }
    }

    /// Initialize schema and start the REST server.
    pub async fn init(&self) -> Result<(), TopologyError> {
        self.store.init_schema().await?;

        let store = Arc::clone(&self.store);
        let port = self.rest_port;
        tokio::spawn(async move {
            if let Err(e) = rest::serve(store, port).await {
                warn!(error = %e, "topology REST server exited with error");
            }
        });

        info!(port = self.rest_port, "topology projector initialized");
        Ok(())
    }

    /// Process an event book and update the topology graph.
    ///
    /// This is the core logic shared by both standalone (`ProjectorHandler`) and
    /// distributed (`EventHandler`) modes.
    pub async fn process_event(&self, events: &EventBook) -> Result<(), TopologyError> {
        let cover = match &events.cover {
            Some(c) => c,
            None => return Ok(()),
        };

        let domain = &cover.domain;
        if domain.is_empty() {
            return Ok(());
        }

        let component_type = Self::infer_component_type(domain);
        let correlation_id = &cover.correlation_id;
        let now = chrono::Utc::now().to_rfc3339();

        for page in &events.pages {
            let event_type = page
                .event
                .as_ref()
                .map(|e| Self::short_event_type(&e.type_url))
                .unwrap_or("unknown");

            // Upsert node for this domain
            if let Err(e) = self
                .store
                .upsert_node(domain, component_type, domain, event_type, &now)
                .await
            {
                warn!(domain = domain, error = %e, "failed to upsert topology node");
                continue;
            }

            // Track correlation for edge discovery
            if !correlation_id.is_empty() {
                match self
                    .store
                    .record_correlation(correlation_id, domain, event_type, &now)
                    .await
                {
                    Ok(domains) => {
                        // Create edges between all domain pairs sharing this correlation
                        for other_domain in &domains {
                            if other_domain == domain {
                                continue;
                            }
                            // Edge direction: alphabetical order for consistency
                            let (source, target) = if other_domain < domain {
                                (other_domain.as_str(), domain.as_str())
                            } else {
                                (domain.as_str(), other_domain.as_str())
                            };

                            if let Err(e) = self
                                .store
                                .upsert_edge(source, target, event_type, correlation_id, &now)
                                .await
                            {
                                debug!(
                                    source = source,
                                    target = target,
                                    error = %e,
                                    "failed to upsert topology edge"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        debug!(
                            correlation_id = correlation_id,
                            error = %e,
                            "failed to record correlation"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Infer the component type from a domain name.
    ///
    /// - `_projection.{name}.{domain}` -> "projector"
    /// - Everything else -> "aggregate" (sagas/PMs are discovered as their own domains)
    fn infer_component_type(domain: &str) -> &'static str {
        if domain.starts_with("_projection.") {
            "projector"
        } else {
            "aggregate"
        }
    }

    /// Extract the short event type name from a protobuf type_url.
    ///
    /// e.g. "type.googleapis.com/ecommerce.OrderPlaced" -> "OrderPlaced"
    fn short_event_type(type_url: &str) -> &str {
        type_url.rsplit('.').next().unwrap_or(type_url)
    }
}

// ProjectorHandler impl for standalone mode (requires sqlite for standalone traits)
#[cfg(feature = "sqlite")]
mod handler {
    use async_trait::async_trait;
    use tracing::warn;

    use crate::proto::{EventBook, Projection};
    use crate::standalone::{ProjectionMode, ProjectorHandler};

    use super::TopologyProjector;

    #[async_trait]
    impl ProjectorHandler for TopologyProjector {
        async fn handle(
            &self,
            events: &EventBook,
            mode: ProjectionMode,
        ) -> Result<Projection, tonic::Status> {
            if mode == ProjectionMode::Speculate {
                return Ok(Projection::default());
            }

            if let Err(e) = self.process_event(events).await {
                warn!(error = %e, "topology projector failed");
            }

            Ok(Projection::default())
        }
    }
}
