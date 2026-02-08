//! Topology projector: builds a graph of runtime components from descriptors.
//!
//! Graph STRUCTURE is declarative (from ComponentDescriptor inputs).
//! Only METRICS (event counts, last seen) are dynamic from event observation.
//!
//! Serves the graph via REST for Grafana's Node Graph panel.
//!
//! # Discovery Modes
//!
//! - **K8s mode**: `TopologyK8sWatcher` watches pod annotations for descriptors
//! - **Event bus mode**: Descriptors published to `_meta.topology` domain (legacy)

#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub mod schema;
pub mod store;
pub mod rest;
pub mod k8s_watcher;

pub use k8s_watcher::TopologyK8sWatcher;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use prost::Message;
use tracing::{debug, info, warn};

use crate::proto::{ComponentDescriptor, ComponentRegistered, EventBook};
use crate::proto_ext::{
    COMPONENT_REGISTERED_TYPE_URL, META_ANGZARR_DOMAIN, PROJECTION_DOMAIN_PREFIX,
};

use store::{TopologyError, TopologyStore};

/// Topology projector that builds component graph from descriptors.
///
/// In standalone mode, register via `RuntimeBuilder::register_projector` (implements
/// `ProjectorHandler` when sqlite feature is enabled). In distributed mode, the
/// `angzarr-topology` binary calls `process_event` directly from its `EventHandler`.
///
/// Safe to share across multiple runtimes — `init()` is idempotent (schema creation
/// and REST server start happen only on the first call).
pub struct TopologyProjector {
    store: Arc<dyn TopologyStore>,
    rest_port: u16,
    initialized: AtomicBool,
    /// Authoritative component types from descriptor registration.
    type_cache: std::sync::RwLock<HashMap<String, String>>,
}

impl TopologyProjector {
    /// Create a new topology projector.
    ///
    /// - `store`: Backing store for topology data (SQLite or Postgres).
    /// - `rest_port`: Port for the REST API serving Grafana Node Graph data.
    pub fn new(store: Arc<dyn TopologyStore>, rest_port: u16) -> Self {
        Self {
            store,
            rest_port,
            initialized: AtomicBool::new(false),
            type_cache: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Initialize schema and start the REST server.
    ///
    /// Idempotent — subsequent calls are no-ops. Safe to call from multiple
    /// runtimes sharing the same projector.
    pub async fn init(&self) -> Result<(), TopologyError> {
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }

        self.store.init_schema().await?;

        let store = Arc::clone(&self.store);
        let port = self.rest_port;
        tokio::spawn(async move {
            if let Err(e) = rest::serve(store, port).await {
                warn!(error = %e, "topology REST server exited with error");
            }
        });

        self.initialized.store(true, Ordering::Release);
        info!(port = self.rest_port, "topology projector initialized");
        Ok(())
    }

    /// Register components from their descriptors.
    ///
    /// Creates nodes and edges from descriptor inputs (subscriptions).
    /// This is the ONLY source of graph structure.
    pub async fn register_components(
        &self,
        descriptors: &[ComponentDescriptor],
    ) -> Result<(), TopologyError> {
        let now = chrono::Utc::now().to_rfc3339();

        // Update type cache
        {
            let mut cache = self.type_cache.write().expect("type_cache poisoned");
            for desc in descriptors {
                if !desc.name.is_empty() {
                    cache.insert(desc.name.clone(), desc.component_type.clone());
                }
            }
        }

        // Pass 1: register all nodes before creating edges (FK constraints)
        let mut registered: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for desc in descriptors {
            if desc.name.is_empty() {
                continue;
            }
            self.store
                .register_node(&desc.name, &desc.component_type, &desc.name, &now)
                .await?;
            registered.insert(&desc.name);
        }

        // Include nodes already in store
        let existing_nodes = self.store.get_nodes().await?;
        for node in &existing_nodes {
            registered.insert(&node.id);
        }

        // Pass 2: create edges from inputs (subscriptions)
        for desc in descriptors {
            if desc.name.is_empty() {
                continue;
            }

            // Input edges: source_domain -> this_component (subscription)
            for input in &desc.inputs {
                if input.domain.is_empty() {
                    continue;
                }
                if !registered.contains(input.domain.as_str()) {
                    debug!(
                        source = %input.domain,
                        target = %desc.name,
                        "skipping input edge: source node not yet registered"
                    );
                    continue;
                }
                // Register each subscribed event type (or one placeholder if none specified)
                if input.types.is_empty() {
                    self.store
                        .upsert_edge(&input.domain, &desc.name, "*", "", &now)
                        .await?;
                } else {
                    for event_type in &input.types {
                        self.store
                            .upsert_edge(&input.domain, &desc.name, event_type, "", &now)
                            .await?;
                    }
                }
            }

            info!(
                name = %desc.name,
                component_type = %desc.component_type,
                inputs = desc.inputs.len(),
                "Registered component in topology"
            );
        }
        Ok(())
    }

    /// Process an event book and update metrics (event counts, last seen).
    ///
    /// Does NOT create new edges — graph structure comes only from descriptors.
    /// Only updates:
    /// - Node event counts and last_event_type
    /// - Edge event counts (for existing edges)
    pub async fn process_event(&self, events: &EventBook) -> Result<(), TopologyError> {
        let cover = match &events.cover {
            Some(c) => c,
            None => return Ok(()),
        };

        let domain = &cover.domain;
        if domain.is_empty() {
            return Ok(());
        }

        // Handle _angzarr domain (component registration events)
        if domain == META_ANGZARR_DOMAIN {
            let descriptors: Vec<ComponentDescriptor> = events
                .pages
                .iter()
                .filter_map(|page| {
                    page.event.as_ref().and_then(|e| {
                        if e.type_url == COMPONENT_REGISTERED_TYPE_URL {
                            ComponentRegistered::decode(e.value.as_slice())
                                .ok()
                                .and_then(|r| r.descriptor)
                        } else {
                            None
                        }
                    })
                })
                .collect();

            if !descriptors.is_empty() {
                info!(count = descriptors.len(), "Received component registration");
                self.register_components(&descriptors).await?;
            }
            return Ok(());
        }

        let component_type = self.resolve_component_type(domain);
        let correlation_id = &cover.correlation_id;
        let now = chrono::Utc::now().to_rfc3339();

        for page in &events.pages {
            let event_type = page
                .event
                .as_ref()
                .map(|e| Self::short_event_type(&e.type_url))
                .unwrap_or("unknown");

            // Update node metrics (event count, last event type)
            if let Err(e) = self
                .store
                .upsert_node(domain, &component_type, domain, event_type, &now)
                .await
            {
                warn!(domain = domain, error = %e, "failed to upsert topology node");
                continue;
            }

            // Record correlation for edge metrics (but don't create new edges)
            if !correlation_id.is_empty() {
                if let Err(e) = self
                    .store
                    .record_correlation(correlation_id, domain, event_type, &now)
                    .await
                {
                    debug!(
                        correlation_id = correlation_id,
                        error = %e,
                        "failed to record correlation"
                    );
                }
                // Note: We don't create edges from correlation anymore.
                // Edges come only from descriptors.
            }
        }

        Ok(())
    }

    /// Resolve the component type for a domain.
    fn resolve_component_type(&self, domain: &str) -> String {
        if let Some(t) = self.type_cache.read().expect("type_cache poisoned").get(domain) {
            return t.clone();
        }
        Self::infer_component_type(domain).to_string()
    }

    /// Infer the component type from a domain name.
    fn infer_component_type(domain: &str) -> &'static str {
        if domain.strip_prefix(PROJECTION_DOMAIN_PREFIX).is_some_and(|rest| rest.starts_with('.')) {
            "projector"
        } else {
            "aggregate"
        }
    }

    /// Extract the short event type name from a protobuf type_url.
    fn short_event_type(type_url: &str) -> &str {
        type_url.rsplit('.').next().unwrap_or(type_url)
    }
}

// ProjectorHandler impl for standalone mode (requires sqlite for standalone runtime)
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
