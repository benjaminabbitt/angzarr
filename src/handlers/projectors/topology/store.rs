//! TopologyStore trait and record types.
//!
//! Pluggable backing store for topology data. Implementations exist for
//! SQLite and PostgreSQL, feature-gated on their respective storage backends.

use async_trait::async_trait;

/// Result type for topology store operations.
pub type Result<T> = std::result::Result<T, TopologyError>;

/// Errors from topology store operations.
#[derive(Debug, thiserror::Error)]
pub enum TopologyError {
    #[error("database error: {0}")]
    Database(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

#[cfg(any(feature = "sqlite", feature = "postgres"))]
impl From<sqlx::Error> for TopologyError {
    fn from(err: sqlx::Error) -> Self {
        TopologyError::Database(err.to_string())
    }
}

/// A discovered node in the topology graph.
#[derive(Debug, Clone)]
pub struct NodeRecord {
    pub id: String,
    pub title: String,
    pub component_type: String,
    pub domain: String,
    /// Output domains (command targets) from the component descriptor.
    pub outputs: Vec<String>,
    pub event_count: i64,
    pub last_event_type: String,
    pub last_seen: String,
    pub created_at: String,
}

/// A discovered edge between two nodes.
#[derive(Debug, Clone)]
pub struct EdgeRecord {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub event_count: i64,
    pub event_types: String,
    pub last_correlation_id: String,
    pub last_seen: String,
    pub created_at: String,
}

/// Pluggable backing store for topology data.
///
/// Implementations persist discovered nodes, edges, and correlation mappings.
/// The topology projector calls these methods as it observes events on the bus.
#[async_trait]
pub trait TopologyStore: Send + Sync + 'static {
    /// Create tables and indexes if they don't exist.
    async fn init_schema(&self) -> Result<()>;

    /// Record that a correlation_id was seen in a domain.
    ///
    /// Returns the list of all domains this correlation_id has been seen in,
    /// enabling edge discovery when multiple domains share a correlation.
    async fn record_correlation(
        &self,
        correlation_id: &str,
        domain: &str,
        event_type: &str,
        timestamp: &str,
    ) -> Result<Vec<String>>;

    /// Register a node with an authoritative component type.
    ///
    /// Unlike `upsert_node`, this always updates `component_type` on conflict.
    /// Used by `register_components()` so that descriptor-provided types
    /// (saga, projector, process_manager) win over the default "aggregate"
    /// inferred by `process_event()`, regardless of insertion order.
    async fn register_node(
        &self,
        node_id: &str,
        component_type: &str,
        domain: &str,
        outputs: &[String],
        timestamp: &str,
    ) -> Result<()>;

    /// Create or update a node for a discovered component.
    async fn upsert_node(
        &self,
        node_id: &str,
        component_type: &str,
        domain: &str,
        event_type: &str,
        timestamp: &str,
    ) -> Result<()>;

    /// Create or update an edge between two components.
    async fn upsert_edge(
        &self,
        source: &str,
        target: &str,
        event_type: &str,
        correlation_id: &str,
        timestamp: &str,
    ) -> Result<()>;

    /// Retrieve all nodes.
    async fn get_nodes(&self) -> Result<Vec<NodeRecord>>;

    /// Retrieve all edges.
    async fn get_edges(&self) -> Result<Vec<EdgeRecord>>;

    /// Delete a node and its associated edges.
    ///
    /// Used when a K8s pod is deleted to remove it from the topology graph.
    /// Cascade deletes all edges where this node is source or target.
    async fn delete_node(&self, node_id: &str) -> Result<()>;

    /// Delete correlations older than the given RFC3339 timestamp.
    ///
    /// Returns the number of pruned rows.
    async fn prune_correlations(&self, older_than: &str) -> Result<u64>;
}
