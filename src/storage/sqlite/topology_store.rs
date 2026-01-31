//! SQLite implementation of TopologyStore.

use async_trait::async_trait;
use sea_query::{Expr, OnConflict, Order, Query, SqliteQueryBuilder};
use sqlx::{Row, SqlitePool};

use crate::handlers::projectors::topology::schema::{
    TopologyCorrelations, TopologyEdges, TopologyNodes,
};
use crate::handlers::projectors::topology::store::{
    EdgeRecord, NodeRecord, Result, TopologyStore,
};

/// SQLite-backed topology store.
pub struct SqliteTopologyStore {
    pool: SqlitePool,
}

impl SqliteTopologyStore {
    /// Create a new SQLite topology store.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TopologyStore for SqliteTopologyStore {
    async fn init_schema(&self) -> Result<()> {
        // Enable foreign keys for this connection
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS topology_nodes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                component_type TEXT NOT NULL,
                domain TEXT NOT NULL,
                event_count INTEGER NOT NULL DEFAULT 0,
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
                event_count INTEGER NOT NULL DEFAULT 0,
                event_types TEXT NOT NULL DEFAULT '[]',
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
        // Upsert this correlation entry
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
            .to_string(SqliteQueryBuilder);

        sqlx::query(&upsert).execute(&self.pool).await?;

        // Fetch all domains for this correlation_id
        let select = Query::select()
            .column(TopologyCorrelations::Domain)
            .from(TopologyCorrelations::Table)
            .and_where(Expr::col(TopologyCorrelations::CorrelationId).eq(correlation_id))
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&select).fetch_all(&self.pool).await?;
        let domains: Vec<String> = rows.iter().map(|r| r.get("domain")).collect();

        Ok(domains)
    }

    async fn upsert_node(
        &self,
        node_id: &str,
        component_type: &str,
        domain: &str,
        event_type: &str,
        timestamp: &str,
    ) -> Result<()> {
        // Try insert first, update on conflict
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
            .to_string(SqliteQueryBuilder);

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
        let edge_id = format!("{}--{}", source, target);

        // Build initial event_types JSON array
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
                edge_id.into(),
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
                    .update_columns([
                        TopologyEdges::LastCorrelationId,
                        TopologyEdges::LastSeen,
                    ])
                    .to_owned(),
            )
            .to_string(SqliteQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;

        // Append event_type to the JSON array if not already present
        sqlx::query(
            "UPDATE topology_edges
             SET event_types = CASE
                 WHEN json_each.value IS NULL
                 THEN json_insert(event_types, '$[#]', ?1)
                 ELSE event_types
             END
             FROM (SELECT value FROM json_each(topology_edges.event_types) WHERE value = ?1) AS json_each
             WHERE topology_edges.id = ?2",
        )
        .bind(event_type)
        .bind(format!("{}--{}", source, target))
        .execute(&self.pool)
        .await
        .ok(); // Best-effort append; the ON CONFLICT already handles the core upsert

        // Simpler approach: just update if the type isn't in the array
        sqlx::query(
            "UPDATE topology_edges
             SET event_types = json_insert(event_types, '$[#]', ?1)
             WHERE id = ?2
             AND NOT EXISTS (
                 SELECT 1 FROM json_each(topology_edges.event_types) WHERE value = ?1
             )",
        )
        .bind(event_type)
        .bind(format!("{}--{}", source, target))
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
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let nodes = rows
            .iter()
            .map(|r| NodeRecord {
                id: r.get("id"),
                title: r.get("title"),
                component_type: r.get("component_type"),
                domain: r.get("domain"),
                event_count: r.get("event_count"),
                last_event_type: r.get("last_event_type"),
                last_seen: r.get("last_seen"),
                created_at: r.get("created_at"),
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
                TopologyEdges::EventTypes,
                TopologyEdges::LastCorrelationId,
                TopologyEdges::LastSeen,
                TopologyEdges::CreatedAt,
            ])
            .from(TopologyEdges::Table)
            .order_by(TopologyEdges::Id, Order::Asc)
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let edges = rows
            .iter()
            .map(|r| EdgeRecord {
                id: r.get("id"),
                source: r.get("source"),
                target: r.get("target"),
                edge_type: r.get("edge_type"),
                event_count: r.get("event_count"),
                event_types: r.get("event_types"),
                last_correlation_id: r.get("last_correlation_id"),
                last_seen: r.get("last_seen"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(edges)
    }

    async fn prune_correlations(&self, older_than: &str) -> Result<u64> {
        let query = Query::delete()
            .from_table(TopologyCorrelations::Table)
            .and_where(Expr::col(TopologyCorrelations::SeenAt).lt(older_than))
            .to_string(SqliteQueryBuilder);

        let result = sqlx::query(&query).execute(&self.pool).await?;
        Ok(result.rows_affected())
    }
}
