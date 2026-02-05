//! Sea-query schema definitions for topology tables.

use sea_query::Iden;

/// Topology nodes table schema.
///
/// Each node represents a discovered runtime component (aggregate, saga,
/// process manager, projector). Discovered from `EventBook.cover.domain`.
#[derive(Iden)]
pub enum TopologyNodes {
    Table,
    #[iden = "id"]
    Id,
    #[iden = "title"]
    Title,
    #[iden = "component_type"]
    ComponentType,
    #[iden = "domain"]
    Domain,
    /// JSON array of output domain names (command targets from descriptor).
    #[iden = "outputs"]
    Outputs,
    #[iden = "event_count"]
    EventCount,
    #[iden = "last_event_type"]
    LastEventType,
    #[iden = "last_seen"]
    LastSeen,
    #[iden = "created_at"]
    CreatedAt,
}

/// Topology edges table schema.
///
/// Each edge represents a discovered command/event flow between two components.
/// Discovered from correlation_id chains spanning multiple domains.
#[derive(Iden)]
pub enum TopologyEdges {
    Table,
    #[iden = "id"]
    Id,
    #[iden = "source"]
    Source,
    #[iden = "target"]
    Target,
    #[iden = "edge_type"]
    EdgeType,
    #[iden = "event_count"]
    EventCount,
    #[iden = "event_types"]
    EventTypes,
    #[iden = "last_correlation_id"]
    LastCorrelationId,
    #[iden = "last_seen"]
    LastSeen,
    #[iden = "created_at"]
    CreatedAt,
}

/// Topology correlations table schema.
///
/// Tracks which domains a correlation_id has been seen in, enabling edge
/// discovery when the same correlation appears in multiple domains.
/// Pruned by TTL to manage table growth.
#[derive(Iden)]
pub enum TopologyCorrelations {
    Table,
    #[iden = "correlation_id"]
    CorrelationId,
    #[iden = "domain"]
    Domain,
    #[iden = "event_type"]
    EventType,
    #[iden = "seen_at"]
    SeenAt,
}
