//! REST API for Grafana Node Graph API plugin.
//!
//! Serves topology data in the format expected by the `hamedkarbasi93-nodegraphapi-datasource`
//! Grafana plugin. Endpoints (both prefixes supported):
//! - `GET /api/health` or `GET /nodegraphds/api/health` — health check
//! - `GET /api/graph/fields` or `GET /nodegraphds/api/graph/fields` — schema for nodes and edges
//! - `GET /api/graph/data` or `GET /nodegraphds/api/graph/data` — current topology graph

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{Method, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

use super::store::TopologyStore;

/// Shared state for axum handlers.
type AppState = Arc<dyn TopologyStore>;

/// Start the REST server on the given port.
///
/// When `port` is 0, the OS assigns an ephemeral port. The actual bound
/// port is always logged so it can be discovered.
pub async fn serve(
    store: Arc<dyn TopologyStore>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = router(store);
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let actual_port = listener.local_addr()?.port();
    info!(port = actual_port, "topology REST API listening");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Build the axum router (separated for testing).
pub fn router(store: Arc<dyn TopologyStore>) -> Router {
    // CORS layer for Grafana Node Graph API plugin (direct mode)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::OPTIONS])
        .allow_headers(Any);

    // The Node Graph API plugin expects endpoints at /nodegraphds/api/...
    // We also support /api/... for direct access
    Router::new()
        .route("/api/health", get(health))
        .route("/api/graph/fields", get(graph_fields))
        .route("/api/graph/data", get(graph_data))
        .route("/nodegraphds/api/health", get(health))
        .route("/nodegraphds/api/graph/fields", get(graph_fields))
        .route("/nodegraphds/api/graph/data", get(graph_data))
        .layer(cors)
        .with_state(store)
}

// ============================================================================
// Handlers
// ============================================================================

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn graph_fields() -> Json<GraphFieldsResponse> {
    Json(GraphFieldsResponse {
        nodes_fields: vec![
            FieldDef::string("id", "ID"),
            FieldDef::string("title", "Title"),
            FieldDef::string("subTitle", "Domain"),
            FieldDef::string("mainStat", "Events"),
            FieldDef::string("secondaryStat", "Type"),
            FieldDef::string("color", "Color"),
            FieldDef::string("detail__component_type", "Component Type"),
            FieldDef::string("detail__domain", "Domain"),
            FieldDef::number("detail__event_count", "Event Count"),
            FieldDef::string("detail__last_event_type", "Last Event Type"),
            FieldDef::string("detail__last_seen", "Last Seen"),
        ],
        edges_fields: vec![
            FieldDef::string("id", "ID"),
            FieldDef::string("source", "Source"),
            FieldDef::string("target", "Target"),
            FieldDef::string("mainStat", "Events"),
            FieldDef::string("secondaryStat", "Event Types"),
            FieldDef::number("detail__event_count", "Event Count"),
            FieldDef::string("detail__event_types", "Event Types"),
            FieldDef::string("detail__last_correlation_id", "Last Correlation"),
            FieldDef::string("detail__last_seen", "Last Seen"),
            FieldDef::string("color", "Color"),
            FieldDef::number("thickness", "Thickness"),
        ],
    })
}

async fn graph_data(
    State(store): State<AppState>,
) -> Result<Json<GraphDataResponse>, StatusCode> {
    let nodes = store.get_nodes().await.map_err(|e| {
        error!(error = %e, "failed to get topology nodes");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let edges = store.get_edges().await.map_err(|e| {
        error!(error = %e, "failed to get topology edges");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Compute incoming message counts for all nodes
    let mut incoming_counts: HashMap<String, i64> = HashMap::new();
    for edge in &edges {
        *incoming_counts.entry(edge.target.clone()).or_insert(0) += edge.event_count;
    }

    let graph_nodes: Vec<GraphNode> = nodes
        .into_iter()
        .map(|n| {
            // For sagas/PMs, use first output domain color; for aggregates, use own domain
            let color_domain = if !n.outputs.is_empty() {
                &n.outputs[0]
            } else {
                &n.domain
            };
            let color = domain_color(color_domain);

            let processed_count = *incoming_counts.get(&n.id).unwrap_or(&0);

            GraphNode {
                id: n.id.clone(),
                title: n.id.clone(),
                subtitle: n.component_type.clone(),
                main_stat: format!("{} processed", processed_count),
                secondary_stat: n.last_event_type.clone(),
                color,
                detail_component_type: n.component_type,
                detail_domain: n.domain,
                detail_event_count: processed_count,
                detail_last_event_type: n.last_event_type,
                detail_last_seen: n.last_seen,
            }
        })
        .collect();

    let graph_edges: Vec<GraphEdge> = edges
        .into_iter()
        .map(|e| {
            let has_error = e.event_types.contains("Error") || e.event_types.contains("Failed");
            GraphEdge {
                id: e.id,
                source: e.source,
                target: e.target,
                main_stat: format!("{} events", e.event_count),
                secondary_stat: summarize_event_types(&e.event_types),
                detail_event_count: e.event_count,
                detail_event_types: e.event_types,
                detail_last_correlation_id: e.last_correlation_id,
                detail_last_seen: e.last_seen,
                color: if has_error { "red" } else { "green" }.to_string(),
                thickness: scale_thickness(e.event_count),
            }
        })
        .collect();

    Ok(Json(GraphDataResponse {
        nodes: graph_nodes,
        edges: graph_edges,
    }))
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Serialize)]
struct GraphFieldsResponse {
    nodes_fields: Vec<FieldDef>,
    edges_fields: Vec<FieldDef>,
}

#[derive(Serialize)]
struct FieldDef {
    field_name: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "type")]
    field_type: String,
}

impl FieldDef {
    fn string(name: &str, display: &str) -> Self {
        Self {
            field_name: name.to_string(),
            display_name: display.to_string(),
            field_type: "string".to_string(),
        }
    }

    fn number(name: &str, display: &str) -> Self {
        Self {
            field_name: name.to_string(),
            display_name: display.to_string(),
            field_type: "number".to_string(),
        }
    }
}

#[derive(Serialize)]
struct GraphDataResponse {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
struct GraphNode {
    id: String,
    title: String,
    #[serde(rename = "subTitle")]
    subtitle: String,
    #[serde(rename = "mainStat")]
    main_stat: String,
    #[serde(rename = "secondaryStat")]
    secondary_stat: String,
    color: String,
    #[serde(rename = "detail__component_type")]
    detail_component_type: String,
    #[serde(rename = "detail__domain")]
    detail_domain: String,
    #[serde(rename = "detail__event_count")]
    detail_event_count: i64,
    #[serde(rename = "detail__last_event_type")]
    detail_last_event_type: String,
    #[serde(rename = "detail__last_seen")]
    detail_last_seen: String,
}

#[derive(Serialize)]
struct GraphEdge {
    id: String,
    source: String,
    target: String,
    #[serde(rename = "mainStat")]
    main_stat: String,
    #[serde(rename = "secondaryStat")]
    secondary_stat: String,
    #[serde(rename = "detail__event_count")]
    detail_event_count: i64,
    #[serde(rename = "detail__event_types")]
    detail_event_types: String,
    #[serde(rename = "detail__last_correlation_id")]
    detail_last_correlation_id: String,
    #[serde(rename = "detail__last_seen")]
    detail_last_seen: String,
    color: String,
    thickness: f64,
}

// ============================================================================
// Helpers
// ============================================================================

/// Generate a consistent hex color for a domain name via hashing.
fn domain_color(domain: &str) -> String {
    let mut hasher = DefaultHasher::new();
    domain.hash(&mut hasher);
    let hash = hasher.finish();

    // Map hash to HSL hue (0-360), keep saturation/lightness fixed for readability
    let hue = (hash % 360) as f64;
    let (r, g, b) = hsl_to_rgb(hue, 0.65, 0.55);
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

/// Convert HSL to RGB (each channel 0-255).
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Summarize a JSON array of event types into a short display string.
fn summarize_event_types(json_types: &str) -> String {
    let types: Vec<String> = serde_json::from_str(json_types).unwrap_or_default();

    // Filter out wildcards and output indicators, keep actual event names
    let filtered: Vec<&str> = types
        .iter()
        .filter(|t| *t != "*" && !t.starts_with('→'))
        .map(String::as_str)
        .collect();

    match filtered.len() {
        0 => {
            // Only wildcards/outputs - show edge direction hint
            if types.iter().any(|t| t.starts_with('→')) {
                "commands".to_string()
            } else if types.contains(&"*".to_string()) {
                "events".to_string()
            } else {
                String::new()
            }
        }
        1 => filtered[0].to_string(),
        n => format!("{} (+{} more)", filtered[0], n - 1),
    }
}

/// Scale edge thickness by event count (log scale, clamped 1-5).
fn scale_thickness(event_count: i64) -> f64 {
    if event_count <= 0 {
        return 1.0;
    }
    let log_val = (event_count as f64).ln();
    (1.0 + log_val).clamp(1.0, 5.0)
}
