//! PostgreSQL implementation of TopologyStore.

use async_trait::async_trait;
use sea_query::{Expr, OnConflict, Order, PostgresQueryBuilder, Query};
use sqlx::{PgPool, Row};

use crate::handlers::projectors::topology::schema::{
    TopologyCorrelations, TopologyEdges, TopologyNodes,
};
use crate::handlers::projectors::topology::store::{
    EdgeRecord, NodeRecord, Result, TopologyStore,
};

/// PostgreSQL-backed topology store.
pub struct PostgresTopologyStore {
    pool: PgPool,
}

impl PostgresTopologyStore {
    /// Create a new PostgreSQL topology store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TopologyStore for PostgresTopologyStore {
    async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS topology_nodes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                component_type TEXT NOT NULL,
                domain TEXT NOT NULL,
                event_count BIGINT NOT NULL DEFAULT 0,
                last_event_type TEXT NOT NULL DEFAULT '',
                last_seen TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS topology_edges (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL REFERENCES topology_nodes(id) ON DELETE CASCADE,
                target TEXT NOT NULL REFERENCES topology_nodes(id) ON DELETE CASCADE,
                edge_type TEXT NOT NULL DEFAULT 'event',
                event_count BIGINT NOT NULL DEFAULT 0,
                event_types JSONB NOT NULL DEFAULT '[]'::jsonb,
                last_correlation_id TEXT NOT NULL DEFAULT '',
                last_seen TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_topology_edges_source ON topology_edges(source)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_topology_edges_target ON topology_edges(target)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS topology_correlations (
                correlation_id TEXT NOT NULL,
                domain TEXT NOT NULL REFERENCES topology_nodes(id) ON DELETE CASCADE,
                event_type TEXT NOT NULL DEFAULT '',
                seen_at TEXT NOT NULL,
                PRIMARY KEY (correlation_id, domain)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_topology_corr_id ON topology_correlations(correlation_id)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_topology_corr_seen ON topology_correlations(seen_at)",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn record_correlation(
        &self,
        correlation_id: &str,
        domain: &str,
        event_type: &str,
        timestamp: &str,
    ) -> Result<Vec<String>> {
        let upsert = Query::insert()
            .into_table(TopologyCorrelations::Table)
            .columns([
                TopologyCorrelations::CorrelationId,
                TopologyCorrelations::Domain,
                TopologyCorrelations::EventType,
                TopologyCorrelations::SeenAt,
            ])
            .values_panic([
                correlation_id.into(),
                domain.into(),
                event_type.into(),
                timestamp.into(),
            ])
            .on_conflict(
                OnConflict::columns([
                    TopologyCorrelations::CorrelationId,
                    TopologyCorrelations::Domain,
                ])
                .update_columns([TopologyCorrelations::EventType, TopologyCorrelations::SeenAt])
                .to_owned(),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&upsert).execute(&self.pool).await?;

        let select = Query::select()
            .column(TopologyCorrelations::Domain)
            .from(TopologyCorrelations::Table)
            .and_where(Expr::col(TopologyCorrelations::CorrelationId).eq(correlation_id))
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::query(&select).fetch_all(&self.pool).await?;
        let domains: Vec<String> = rows.iter().map(|r| r.get("domain")).collect();

        Ok(domains)
    }

    async fn register_node(
        &self,
        node_id: &str,
        component_type: &str,
        domain: &str,
        timestamp: &str,
    ) -> Result<()> {
        let query = Query::insert()
            .into_table(TopologyNodes::Table)
            .columns([
                TopologyNodes::Id,
                TopologyNodes::Title,
                TopologyNodes::ComponentType,
                TopologyNodes::Domain,
                TopologyNodes::EventCount,
                TopologyNodes::LastEventType,
                TopologyNodes::LastSeen,
                TopologyNodes::CreatedAt,
            ])
            .values_panic([
                node_id.into(),
                node_id.into(),
                component_type.into(),
                domain.into(),
                0_i64.into(),
                "registered".into(),
                timestamp.into(),
                timestamp.into(),
            ])
            .on_conflict(
                OnConflict::column(TopologyNodes::Id)
                    .update_columns([TopologyNodes::ComponentType])
                    .to_owned(),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
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
        let query = Query::insert()
            .into_table(TopologyNodes::Table)
            .columns([
                TopologyNodes::Id,
                TopologyNodes::Title,
                TopologyNodes::ComponentType,
                TopologyNodes::Domain,
                TopologyNodes::EventCount,
                TopologyNodes::LastEventType,
                TopologyNodes::LastSeen,
                TopologyNodes::CreatedAt,
            ])
            .values_panic([
                node_id.into(),
                node_id.into(),
                component_type.into(),
                domain.into(),
                1_i64.into(),
                event_type.into(),
                timestamp.into(),
                timestamp.into(),
            ])
            .on_conflict(
                OnConflict::column(TopologyNodes::Id)
                    .value(
                        TopologyNodes::EventCount,
                        Expr::col(TopologyNodes::EventCount).add(1),
                    )
                    .update_columns([TopologyNodes::LastEventType, TopologyNodes::LastSeen])
                    .to_owned(),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
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
        // Alphabetical ID for stable dedup — same pair always gets same ID
        // regardless of which domain's event triggered the edge discovery.
        let (id_a, id_b) = if source < target {
            (source, target)
        } else {
            (target, source)
        };
        let edge_id = format!("{}--{}", id_a, id_b);

        let initial_types =
            serde_json::to_string(&vec![event_type]).unwrap_or_else(|_| "[]".to_string());

        let query = Query::insert()
            .into_table(TopologyEdges::Table)
            .columns([
                TopologyEdges::Id,
                TopologyEdges::Source,
                TopologyEdges::Target,
                TopologyEdges::EdgeType,
                TopologyEdges::EventCount,
                TopologyEdges::EventTypes,
                TopologyEdges::LastCorrelationId,
                TopologyEdges::LastSeen,
                TopologyEdges::CreatedAt,
            ])
            .values_panic([
                edge_id.clone().into(),
                source.into(),
                target.into(),
                "event".into(),
                1_i64.into(),
                initial_types.into(),
                correlation_id.into(),
                timestamp.into(),
                timestamp.into(),
            ])
            .on_conflict(
                OnConflict::column(TopologyEdges::Id)
                    .value(
                        TopologyEdges::EventCount,
                        Expr::col(TopologyEdges::EventCount).add(1),
                    )
                    .update_columns([TopologyEdges::LastCorrelationId, TopologyEdges::LastSeen])
                    .to_owned(),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;

        // Append event_type to JSONB array if not already present (PostgreSQL native)
        sqlx::query(
            "UPDATE topology_edges
             SET event_types = event_types || to_jsonb($1::text)
             WHERE id = $2
             AND NOT event_types ? $1",
        )
        .bind(event_type)
        .bind(&edge_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_nodes(&self) -> Result<Vec<NodeRecord>> {
        let query = Query::select()
            .columns([
                TopologyNodes::Id,
                TopologyNodes::Title,
                TopologyNodes::ComponentType,
                TopologyNodes::Domain,
                TopologyNodes::EventCount,
                TopologyNodes::LastEventType,
                TopologyNodes::LastSeen,
                TopologyNodes::CreatedAt,
            ])
            .from(TopologyNodes::Table)
            .order_by(TopologyNodes::Id, Order::Asc)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let nodes = rows
            .iter()
            .map(|r| {
                NodeRecord {
                    id: r.get("id"),
                    title: r.get("title"),
                    component_type: r.get("component_type"),
                    domain: r.get("domain"),
                    event_count: r.get("event_count"),
                    last_event_type: r.get("last_event_type"),
                    last_seen: r.get("last_seen"),
                    created_at: r.get("created_at"),
                }
            })
            .collect();

        Ok(nodes)
    }

    async fn get_edges(&self) -> Result<Vec<EdgeRecord>> {
        let query = Query::select()
            .columns([
                TopologyEdges::Id,
                TopologyEdges::Source,
                TopologyEdges::Target,
                TopologyEdges::EdgeType,
                TopologyEdges::EventCount,
                TopologyEdges::LastCorrelationId,
                TopologyEdges::LastSeen,
                TopologyEdges::CreatedAt,
            ])
            .from(TopologyEdges::Table)
            .order_by(TopologyEdges::Id, Order::Asc)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let edges = rows
            .iter()
            .map(|r| {
                // PostgreSQL stores event_types as JSONB; extract as string for EdgeRecord
                let event_types: serde_json::Value =
                    r.try_get("event_types").unwrap_or(serde_json::Value::Array(vec![]));
                EdgeRecord {
                    id: r.get("id"),
                    source: r.get("source"),
                    target: r.get("target"),
                    edge_type: r.get("edge_type"),
                    event_count: r.get("event_count"),
                    event_types: event_types.to_string(),
                    last_correlation_id: r.get("last_correlation_id"),
                    last_seen: r.get("last_seen"),
                    created_at: r.get("created_at"),
                }
            })
            .collect();

        Ok(edges)
    }

    async fn delete_node(&self, node_id: &str) -> Result<()> {
        // Delete edges where this node is source or target
        let delete_edges = Query::delete()
            .from_table(TopologyEdges::Table)
            .and_where(
                Expr::col(TopologyEdges::Source)
                    .eq(node_id)
                    .or(Expr::col(TopologyEdges::Target).eq(node_id)),
            )
            .to_string(PostgresQueryBuilder);
        sqlx::query(&delete_edges).execute(&self.pool).await?;

        // Delete the node
        let delete_node = Query::delete()
            .from_table(TopologyNodes::Table)
            .and_where(Expr::col(TopologyNodes::Id).eq(node_id))
            .to_string(PostgresQueryBuilder);
        sqlx::query(&delete_node).execute(&self.pool).await?;

        Ok(())
    }

    async fn prune_correlations(&self, older_than: &str) -> Result<u64> {
        let query = Query::delete()
            .from_table(TopologyCorrelations::Table)
            .and_where(Expr::col(TopologyCorrelations::SeenAt).lt(older_than))
            .to_string(PostgresQueryBuilder);

        let result = sqlx::query(&query).execute(&self.pool).await?;
        Ok(result.rows_affected())
    }
}
