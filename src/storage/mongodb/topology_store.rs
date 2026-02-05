//! MongoDB implementation of TopologyStore.

use async_trait::async_trait;
use mongodb::bson::doc;
use mongodb::options::{IndexOptions, UpdateOptions};
use mongodb::{Client, Collection, IndexModel};

use crate::handlers::projectors::topology::store::{
    EdgeRecord, NodeRecord, Result, TopologyError, TopologyStore,
};

use super::{TOPOLOGY_CORRELATIONS_COLLECTION, TOPOLOGY_EDGES_COLLECTION, TOPOLOGY_NODES_COLLECTION};

/// MongoDB-backed topology store.
pub struct MongoTopologyStore {
    nodes: Collection<mongodb::bson::Document>,
    edges: Collection<mongodb::bson::Document>,
    correlations: Collection<mongodb::bson::Document>,
}

impl MongoTopologyStore {
    /// Create a new MongoDB topology store.
    pub async fn new(client: &Client, database_name: &str) -> Result<Self> {
        let database = client.database(database_name);
        let nodes = database.collection(TOPOLOGY_NODES_COLLECTION);
        let edges = database.collection(TOPOLOGY_EDGES_COLLECTION);
        let correlations = database.collection(TOPOLOGY_CORRELATIONS_COLLECTION);

        Ok(Self {
            nodes,
            edges,
            correlations,
        })
    }
}

#[async_trait]
impl TopologyStore for MongoTopologyStore {
    async fn init_schema(&self) -> Result<()> {
        // Nodes: unique index on id
        let node_index = IndexModel::builder()
            .keys(doc! { "id": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();
        self.nodes
            .create_index(node_index)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Edges: unique index on id
        let edge_id_index = IndexModel::builder()
            .keys(doc! { "id": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();
        self.edges
            .create_index(edge_id_index)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Edges: indexes on source and target for FK-like queries
        let edge_source_index = IndexModel::builder()
            .keys(doc! { "source": 1 })
            .build();
        self.edges
            .create_index(edge_source_index)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        let edge_target_index = IndexModel::builder()
            .keys(doc! { "target": 1 })
            .build();
        self.edges
            .create_index(edge_target_index)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Correlations: compound unique index on (correlation_id, domain)
        let corr_index = IndexModel::builder()
            .keys(doc! { "correlation_id": 1, "domain": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();
        self.correlations
            .create_index(corr_index)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Correlations: index on seen_at for pruning
        let corr_seen_index = IndexModel::builder()
            .keys(doc! { "seen_at": 1 })
            .build();
        self.correlations
            .create_index(corr_seen_index)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        Ok(())
    }

    async fn record_correlation(
        &self,
        correlation_id: &str,
        domain: &str,
        event_type: &str,
        timestamp: &str,
    ) -> Result<Vec<String>> {
        // Upsert the correlation record
        let filter = doc! {
            "correlation_id": correlation_id,
            "domain": domain,
        };
        let update = doc! {
            "$set": {
                "correlation_id": correlation_id,
                "domain": domain,
                "event_type": event_type,
                "seen_at": timestamp,
            }
        };
        let options = UpdateOptions::builder().upsert(true).build();

        self.correlations
            .update_one(filter, update)
            .with_options(options)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Query all domains with this correlation_id
        let filter = doc! { "correlation_id": correlation_id };
        let mut cursor = self
            .correlations
            .find(filter)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        let mut domains = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?
        {
            let doc = cursor
                .deserialize_current()
                .map_err(|e| TopologyError::Database(e.to_string()))?;
            if let Ok(d) = doc.get_str("domain") {
                domains.push(d.to_string());
            }
        }

        Ok(domains)
    }

    async fn register_node(
        &self,
        node_id: &str,
        component_type: &str,
        domain: &str,
        outputs: &[String],
        timestamp: &str,
    ) -> Result<()> {
        let filter = doc! { "id": node_id };
        let outputs_vec: Vec<&str> = outputs.iter().map(|s| s.as_str()).collect();
        let update = doc! {
            "$set": {
                "component_type": component_type,
                "outputs": &outputs_vec,
            },
            "$setOnInsert": {
                "id": node_id,
                "title": node_id,
                "domain": domain,
                "event_count": 0_i64,
                "last_event_type": "registered",
                "last_seen": timestamp,
                "created_at": timestamp,
            }
        };
        let options = UpdateOptions::builder().upsert(true).build();

        self.nodes
            .update_one(filter, update)
            .with_options(options)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

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
        let filter = doc! { "id": node_id };
        let update = doc! {
            "$inc": { "event_count": 1_i64 },
            "$set": {
                "last_event_type": event_type,
                "last_seen": timestamp,
            },
            "$setOnInsert": {
                "id": node_id,
                "title": node_id,
                "component_type": component_type,
                "domain": domain,
                "created_at": timestamp,
            }
        };
        let options = UpdateOptions::builder().upsert(true).build();

        self.nodes
            .update_one(filter, update)
            .with_options(options)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

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
        // Alphabetical ID for stable dedup
        let (id_a, id_b) = if source < target {
            (source, target)
        } else {
            (target, source)
        };
        let edge_id = format!("{}--{}", id_a, id_b);

        let filter = doc! { "id": &edge_id };

        let update = doc! {
            "$inc": { "event_count": 1_i64 },
            "$set": {
                "last_correlation_id": correlation_id,
                "last_seen": timestamp,
            },
            "$addToSet": { "event_types": event_type },
            "$setOnInsert": {
                "id": &edge_id,
                "source": source,
                "target": target,
                "edge_type": "event",
                "created_at": timestamp,
            }
        };
        let options = UpdateOptions::builder().upsert(true).build();

        self.edges
            .update_one(filter, update)
            .with_options(options)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_nodes(&self) -> Result<Vec<NodeRecord>> {
        let mut cursor = self
            .nodes
            .find(doc! {})
            .sort(doc! { "id": 1 })
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        let mut nodes = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?
        {
            let doc = cursor
                .deserialize_current()
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            let outputs = doc
                .get_array("outputs")
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            nodes.push(NodeRecord {
                id: doc.get_str("id").unwrap_or_default().to_string(),
                title: doc.get_str("title").unwrap_or_default().to_string(),
                component_type: doc.get_str("component_type").unwrap_or_default().to_string(),
                domain: doc.get_str("domain").unwrap_or_default().to_string(),
                outputs,
                event_count: doc.get_i64("event_count").unwrap_or(0),
                last_event_type: doc.get_str("last_event_type").unwrap_or_default().to_string(),
                last_seen: doc.get_str("last_seen").unwrap_or_default().to_string(),
                created_at: doc.get_str("created_at").unwrap_or_default().to_string(),
            });
        }

        Ok(nodes)
    }

    async fn get_edges(&self) -> Result<Vec<EdgeRecord>> {
        let mut cursor = self
            .edges
            .find(doc! {})
            .sort(doc! { "id": 1 })
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        let mut edges = Vec::new();
        while cursor
            .advance()
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?
        {
            let doc = cursor
                .deserialize_current()
                .map_err(|e| TopologyError::Database(e.to_string()))?;

            // event_types stored as array in MongoDB, convert to JSON string for EdgeRecord
            let event_types = doc
                .get_array("event_types")
                .map(|arr| {
                    let types: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    serde_json::to_string(&types).unwrap_or_else(|_| "[]".to_string())
                })
                .unwrap_or_else(|_| "[]".to_string());

            edges.push(EdgeRecord {
                id: doc.get_str("id").unwrap_or_default().to_string(),
                source: doc.get_str("source").unwrap_or_default().to_string(),
                target: doc.get_str("target").unwrap_or_default().to_string(),
                edge_type: doc.get_str("edge_type").unwrap_or_default().to_string(),
                event_count: doc.get_i64("event_count").unwrap_or(0),
                event_types,
                last_correlation_id: doc
                    .get_str("last_correlation_id")
                    .unwrap_or_default()
                    .to_string(),
                last_seen: doc.get_str("last_seen").unwrap_or_default().to_string(),
                created_at: doc.get_str("created_at").unwrap_or_default().to_string(),
            });
        }

        Ok(edges)
    }

    async fn delete_node(&self, node_id: &str) -> Result<()> {
        // Delete edges where this node is source or target
        self.edges
            .delete_many(doc! {
                "$or": [
                    { "source": node_id },
                    { "target": node_id }
                ]
            })
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        // Delete the node
        self.nodes
            .delete_one(doc! { "id": node_id })
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        Ok(())
    }

    async fn prune_correlations(&self, older_than: &str) -> Result<u64> {
        let filter = doc! { "seen_at": { "$lt": older_than } };
        let result = self
            .correlations
            .delete_many(filter)
            .await
            .map_err(|e| TopologyError::Database(e.to_string()))?;

        Ok(result.deleted_count)
    }
}
