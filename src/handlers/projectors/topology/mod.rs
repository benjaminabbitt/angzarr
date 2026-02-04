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

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use prost::Message;
use tracing::{debug, info, warn};

use crate::proto::{ComponentDescriptor, EventBook};
use crate::proto_ext::{META_TOPOLOGY_DOMAIN, DESCRIPTOR_TYPE_URL, PROJECTION_DOMAIN_PREFIX};

use store::{TopologyError, TopologyStore};

/// Topology projector that discovers runtime component graph from the event stream.
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
    ///
    /// Populated by `register_components()`. Used by `process_event()` to
    /// pass correct types to `upsert_node` on first insert.
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
    /// Called at startup by the runtime. Creates nodes with correct component_type
    /// and "subscription" edges from input domains to each component.
    /// Output edges are discovered at runtime via event observation.
    pub async fn register_components(
        &self,
        descriptors: &[ComponentDescriptor],
    ) -> Result<(), TopologyError> {
        let now = chrono::Utc::now().to_rfc3339();

        // Batch-update the type cache
        {
            let mut cache = self.type_cache.write().expect("type_cache poisoned");
            for desc in descriptors {
                if !desc.name.is_empty() {
                    cache.insert(desc.name.clone(), desc.component_type.clone());
                }
            }
        }

        // Pass 1: register all nodes before creating edges.
        // Edges have FK constraints on topology_nodes(id), so both source
        // and target nodes must exist before any edge can be inserted.
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

        // Include nodes already in the store (from earlier registrations or
        // process_event calls) so edges to pre-existing domains succeed.
        let existing_nodes = self.store.get_nodes().await?;
        for node in &existing_nodes {
            registered.insert(&node.id);
        }

        // Pass 2: create subscription edges. Skip edges whose source node
        // doesn't exist yet — they'll be discovered at runtime via correlation.
        for desc in descriptors {
            if desc.name.is_empty() {
                continue;
            }

            for input in &desc.inputs {
                if input.domain.is_empty() {
                    continue;
                }
                if !registered.contains(input.domain.as_str()) {
                    debug!(
                        source = %input.domain,
                        target = %desc.name,
                        "skipping subscription edge: source node not yet registered"
                    );
                    continue;
                }
                self.store
                    .upsert_edge(&input.domain, &desc.name, "subscription", "", &now)
                    .await?;
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

    /// Process an event book and update the topology graph.
    ///
    /// This is the core logic shared by both standalone (`ProjectorHandler`) and
    /// distributed (`EventHandler`) modes.
    ///
    /// Handles two event types:
    /// - **Meta-events** (`_meta.topology` domain): descriptor registrations from
    ///   services at startup. Decoded and forwarded to `register_components`.
    /// - **Domain events**: normal event stream observation for node/edge discovery.
    pub async fn process_event(&self, events: &EventBook) -> Result<(), TopologyError> {
        let cover = match &events.cover {
            Some(c) => c,
            None => return Ok(()),
        };

        let domain = &cover.domain;
        if domain.is_empty() {
            return Ok(());
        }

        // Handle topology meta-events (descriptor registration from services)
        if domain == META_TOPOLOGY_DOMAIN {
            let descriptors: Vec<ComponentDescriptor> = events
                .pages
                .iter()
                .filter_map(|page| {
                    page.event.as_ref().and_then(|e| {
                        if e.type_url == DESCRIPTOR_TYPE_URL {
                            ComponentDescriptor::decode(e.value.as_slice()).ok()
                        } else {
                            None
                        }
                    })
                })
                .collect();

            if !descriptors.is_empty() {
                info!(count = descriptors.len(), "Received descriptor registration via bus");
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

            // Upsert node for this domain
            if let Err(e) = self
                .store
                .upsert_node(domain, &component_type, domain, event_type, &now)
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

                            // Causal direction: the other domain appeared earlier in
                            // the correlation chain (already recorded), so it's the
                            // source. The current domain just emitted an event in
                            // response, so it's the target.
                            //
                            // Edge ID uses alphabetical order for stable dedup — the
                            // same pair always produces the same ID regardless of
                            // which event arrives first.
                            let (source, target) =
                                (other_domain.as_str(), domain.as_str());

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

    /// Resolve the component type for a domain, preferring the authoritative
    /// type from descriptor registration over the domain-name heuristic.
    fn resolve_component_type(&self, domain: &str) -> String {
        if let Some(t) = self.type_cache.read().expect("type_cache poisoned").get(domain) {
            return t.clone();
        }
        Self::infer_component_type(domain).to_string()
    }

    /// Infer the component type from a domain name.
    ///
    /// - `_projection.{name}.{domain}` -> "projector"
    /// - Everything else -> "aggregate" (sagas/PMs are discovered as their own domains)
    fn infer_component_type(domain: &str) -> &'static str {
        if domain.strip_prefix(PROJECTION_DOMAIN_PREFIX).is_some_and(|rest| rest.starts_with('.')) {
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

// publish_descriptors lives in proto_ext (non-feature-gated) so that
// distributed binaries can call it without enabling the topology feature.

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
