//! Redis implementation of TopologyStore.

use async_trait::async_trait;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use tracing::info;

use crate::handlers::projectors::topology::store::{
    EdgeRecord, NodeRecord, Result, TopologyError, TopologyStore,
};

/// Redis-backed topology store.
///
/// Uses Redis hashes and sets for topology data:
/// - Nodes: Hash per node (id -> fields)
/// - Edges: Hash per edge (id -> fields)
/// - Correlations: Set per correlation_id (domains)
pub struct RedisTopologyStore {
    conn: ConnectionManager,
    key_prefix: String,
}

impl RedisTopologyStore {
    /// Create a new Redis topology store.
    ///
    /// # Arguments
    /// * `url` - Redis connection URL (e.g., redis://localhost:6379)
    /// * `key_prefix` - Prefix for all keys (default: "angzarr")
    pub async fn new(url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let client = Client::open(url).map_err(|e| TopologyError::Database(e.to_string()))?;
        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        info!(url = %url, "Connected to Redis for topology store");

        Ok(Self {
            conn,
            key_prefix: key_prefix.unwrap_or("angzarr").to_string(),
        })
    }

    /// Build the key for the nodes index set.
    fn nodes_index_key(&self) -> String {
        format!("{}:topology:nodes", self.key_prefix)
    }

    /// Build the key for a specific node hash.
    fn node_key(&self, node_id: &str) -> String {
        format!("{}:topology:node:{}", self.key_prefix, node_id)
    }

    /// Build the key for the edges index set.
    fn edges_index_key(&self) -> String {
        format!("{}:topology:edges", self.key_prefix)
    }

    /// Build the key for a specific edge hash.
    fn edge_key(&self, edge_id: &str) -> String {
        format!("{}:topology:edge:{}", self.key_prefix, edge_id)
    }

    /// Build the key for a correlation set.
    fn correlation_key(&self, correlation_id: &str) -> String {
        format!("{}:topology:correlation:{}", self.key_prefix, correlation_id)
    }

    /// Build the key for the correlations index (for pruning).
    fn correlations_index_key(&self) -> String {
        format!("{}:topology:correlations", self.key_prefix)
    }
}

#[async_trait]
impl TopologyStore for RedisTopologyStore {
    async fn init_schema(&self) -> Result<()> {
        // Redis doesn't need schema initialization
        Ok(())
    }

    async fn record_correlation(
        &self,
        correlation_id: &str,
        domain: &str,
        _event_type: &str,
        timestamp: &str,
    ) -> Result<Vec<String>> {
        let mut conn = self.conn.clone();

        let correlation_key = self.correlation_key(correlation_id);
        let correlations_index = self.correlations_index_key();

        // Add domain to the correlation set
        let _: () = conn
            .sadd(&correlation_key, domain)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Track correlation in index with timestamp (for pruning)
        // Use sorted set with timestamp as score
        let _: () = conn
            .zadd(&correlations_index, correlation_id, timestamp)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Get all domains for this correlation
        let domains: Vec<String> = conn
            .smembers(&correlation_key)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        Ok(domains)
    }

    async fn register_node(
        &self,
        node_id: &str,
        component_type: &str,
        domain: &str,
        timestamp: &str,
    ) -> Result<()> {
        let mut conn = self.conn.clone();

        let node_key = self.node_key(node_id);
        let nodes_index = self.nodes_index_key();

        // Check if node exists
        let exists: bool = conn
            .exists(&node_key)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        if exists {
            // Update component_type (register always wins)
            let _: () = conn
                .hset_multiple(
                    &node_key,
                    &[
                        ("component_type", component_type),
                    ],
                )
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
        } else {
            // Create new node
            let _: () = conn
                .hset_multiple(
                    &node_key,
                    &[
                        ("id", node_id),
                        ("title", node_id),
                        ("component_type", component_type),
                        ("domain", domain),
                        ("event_count", "0"),
                        ("last_event_type", "registered"),
                        ("last_seen", timestamp),
                        ("created_at", timestamp),
                    ],
                )
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            // Add to nodes index
            let _: () = conn
                .sadd(&nodes_index, node_id)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
        }

        Ok(())
    }

    async fn upsert_node(
        &self,
        node_id: &str,
        component_type: &str,
        domain: &str,
        event_type: &str,
        timestamp: &str,
    ) -> Result<()> {
        let mut conn = self.conn.clone();

        let node_key = self.node_key(node_id);
        let nodes_index = self.nodes_index_key();

        // Check if node exists
        let exists: bool = conn
            .exists(&node_key)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        if exists {
            // Increment event_count and update last_* fields
            let _: () = conn
                .hincr(&node_key, "event_count", 1_i64)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
            let _: () = conn
                .hset_multiple(
                    &node_key,
                    &[("last_event_type", event_type), ("last_seen", timestamp)],
                )
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
        } else {
            // Create new node with event_count = 1
            let _: () = conn
                .hset_multiple(
                    &node_key,
                    &[
                        ("id", node_id),
                        ("title", node_id),
                        ("component_type", component_type),
                        ("domain", domain),
                        ("event_count", "1"),
                        ("last_event_type", event_type),
                        ("last_seen", timestamp),
                        ("created_at", timestamp),
                    ],
                )
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            // Add to nodes index
            let _: () = conn
                .sadd(&nodes_index, node_id)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
        }

        Ok(())
    }

    async fn upsert_edge(
        &self,
        source: &str,
        target: &str,
        event_type: &str,
        correlation_id: &str,
        timestamp: &str,
    ) -> Result<()> {
        let mut conn = self.conn.clone();

        // Alphabetical ID for stable dedup
        let (id_a, id_b) = if source < target {
            (source, target)
        } else {
            (target, source)
        };
        let edge_id = format!("{}--{}", id_a, id_b);

        let edge_key = self.edge_key(&edge_id);
        let edges_index = self.edges_index_key();

        // Check if edge exists
        let exists: bool = conn
            .exists(&edge_key)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        if exists {
            // Increment event_count and update last_* fields
            let _: () = conn
                .hincr(&edge_key, "event_count", 1_i64)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
            let _: () = conn
                .hset_multiple(
                    &edge_key,
                    &[
                        ("last_correlation_id", correlation_id),
                        ("last_seen", timestamp),
                    ],
                )
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            // Add event_type to set (stored as separate key for set operations)
            let event_types_key = format!("{}:event_types", edge_key);
            let _: () = conn
                .sadd(&event_types_key, event_type)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
        } else {
            // Create new edge
            let _: () = conn
                .hset_multiple(
                    &edge_key,
                    &[
                        ("id", edge_id.as_str()),
                        ("source", source),
                        ("target", target),
                        ("edge_type", "event"),
                        ("event_count", "1"),
                        ("last_correlation_id", correlation_id),
                        ("last_seen", timestamp),
                        ("created_at", timestamp),
                    ],
                )
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            // Initialize event_types set
            let event_types_key = format!("{}:event_types", edge_key);
            let _: () = conn
                .sadd(&event_types_key, event_type)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            // Add to edges index
            let _: () = conn
                .sadd(&edges_index, &edge_id)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
        }

        Ok(())
    }

    async fn get_nodes(&self) -> Result<Vec<NodeRecord>> {
        let mut conn = self.conn.clone();

        let nodes_index = self.nodes_index_key();

        // Get all node IDs
        let node_ids: Vec<String> = conn
            .smembers(&nodes_index)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        let mut nodes = Vec::with_capacity(node_ids.len());

        for node_id in node_ids {
            let node_key = self.node_key(&node_id);

            let fields: Vec<String> = conn
                .hgetall(&node_key)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            // Parse fields (alternating key/value pairs)
            let mut map = std::collections::HashMap::new();
            for chunk in fields.chunks(2) {
                if chunk.len() == 2 {
                    map.insert(chunk[0].clone(), chunk[1].clone());
                }
            }

            nodes.push(NodeRecord {
                id: map.get("id").cloned().unwrap_or_default(),
                title: map.get("title").cloned().unwrap_or_default(),
                component_type: map.get("component_type").cloned().unwrap_or_default(),
                domain: map.get("domain").cloned().unwrap_or_default(),
                event_count: map
                    .get("event_count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                last_event_type: map.get("last_event_type").cloned().unwrap_or_default(),
                last_seen: map.get("last_seen").cloned().unwrap_or_default(),
                created_at: map.get("created_at").cloned().unwrap_or_default(),
            });
        }

        // Sort by id for consistent ordering
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(nodes)
    }

    async fn get_edges(&self) -> Result<Vec<EdgeRecord>> {
        let mut conn = self.conn.clone();

        let edges_index = self.edges_index_key();

        // Get all edge IDs
        let edge_ids: Vec<String> = conn
            .smembers(&edges_index)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        let mut edges = Vec::with_capacity(edge_ids.len());

        for edge_id in edge_ids {
            let edge_key = self.edge_key(&edge_id);

            let fields: Vec<String> = conn
                .hgetall(&edge_key)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            // Parse fields (alternating key/value pairs)
            let mut map = std::collections::HashMap::new();
            for chunk in fields.chunks(2) {
                if chunk.len() == 2 {
                    map.insert(chunk[0].clone(), chunk[1].clone());
                }
            }

            // Get event_types from separate set
            let event_types_key = format!("{}:event_types", edge_key);
            let event_types: Vec<String> = conn
                .smembers(&event_types_key)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
            let event_types_json =
                serde_json::to_string(&event_types).unwrap_or_else(|_| "[]".to_string());

            edges.push(EdgeRecord {
                id: map.get("id").cloned().unwrap_or_default(),
                source: map.get("source").cloned().unwrap_or_default(),
                target: map.get("target").cloned().unwrap_or_default(),
                edge_type: map.get("edge_type").cloned().unwrap_or_default(),
                event_count: map
                    .get("event_count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                event_types: event_types_json,
                last_correlation_id: map.get("last_correlation_id").cloned().unwrap_or_default(),
                last_seen: map.get("last_seen").cloned().unwrap_or_default(),
                created_at: map.get("created_at").cloned().unwrap_or_default(),
            });
        }

        // Sort by id for consistent ordering
        edges.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(edges)
    }

    async fn delete_node(&self, node_id: &str) -> Result<()> {
        let mut conn = self.conn.clone();

        // Get all edge IDs
        let edge_ids: Vec<String> = conn
            .smembers(&self.edges_index_key())
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Find and delete edges where this node is source or target
        for edge_id in edge_ids {
            let edge_key = self.edge_key(&edge_id);
            let source: Option<String> = conn
                .hget(&edge_key, "source")
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
            let target: Option<String> = conn
                .hget(&edge_key, "target")
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            if source.as_deref() == Some(node_id) || target.as_deref() == Some(node_id) {
                // Delete the edge hash
                conn.del::<_, ()>(&edge_key)
                    .await
                    .map_err(|e| TopologyError::Database(e.to_string()))?;
                // Remove from edges index
                conn.srem::<_, _, ()>(&self.edges_index_key(), &edge_id)
                    .await
                    .map_err(|e| TopologyError::Database(e.to_string()))?;
            }
        }

        // Delete the node hash
        let node_key = self.node_key(node_id);
        conn.del::<_, ()>(&node_key)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Remove from nodes index
        conn.srem::<_, _, ()>(&self.nodes_index_key(), node_id)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        Ok(())
    }

    async fn prune_correlations(&self, older_than: &str) -> Result<u64> {
        let mut conn = self.conn.clone();

        let correlations_index = self.correlations_index_key();

        // Get correlations older than timestamp using sorted set range
        let old_correlations: Vec<String> = conn
            .zrangebyscore(&correlations_index, "-inf", older_than)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        let count = old_correlations.len() as u64;

        // Delete each correlation's set and remove from index
        for correlation_id in &old_correlations {
            let correlation_key = self.correlation_key(correlation_id);
            let _: () = conn
                .del(&correlation_key)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
        }

        // Remove from sorted set index
        if !old_correlations.is_empty() {
            let _: () = conn
                .zrembyscore(&correlations_index, "-inf", older_than)
                .await
                .map_err(|e| TopologyError::Database(e.to_string()))?;
        }

        Ok(count)
    }
}
