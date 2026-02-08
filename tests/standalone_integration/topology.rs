//! Topology integration tests — verifies all component types appear in the graph.

use std::sync::Arc;

use axum::body::Body;
use http::Request;
use tower::ServiceExt;

use angzarr::handlers::projectors::topology::rest::router;
use angzarr::handlers::projectors::topology::store::TopologyStore;
use angzarr::handlers::projectors::topology::TopologyProjector;
use angzarr::proto::{
    CommandBook, ComponentDescriptor, Cover, EventBook, Projection, SagaResponse, Target,
};
use angzarr::standalone::{
    ProcessManagerConfig, ProcessManagerHandler, ProjectionMode, ProjectorConfig, ProjectorHandler,
    RuntimeBuilder, SagaConfig, SagaHandler,
};
use angzarr::storage::SqliteTopologyStore;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::common::*;

// ============================================================================
// Minimal test handlers
// ============================================================================

struct NoopProjector;

#[async_trait]
impl ProjectorHandler for NoopProjector {
    async fn handle(&self, _events: &EventBook, _mode: ProjectionMode) -> Result<Projection, Status> {
        Ok(Projection::default())
    }
}

struct NoopSaga;

#[async_trait]
impl SagaHandler for NoopSaga {
    async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
        Ok(vec![])
    }

    async fn execute(
        &self,
        _source: &EventBook,
        _destinations: &[EventBook],
    ) -> Result<SagaResponse, Status> {
        Ok(SagaResponse::default())
    }
}

struct NoopProcessManager;

impl ProcessManagerHandler for NoopProcessManager {
    fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor {
            name: "test-pm".into(),
            component_type: "process_manager".into(),
            inputs: vec![Target {
                domain: "orders".into(),
                types: vec![],
            }],
        }
    }

    fn prepare(&self, _trigger: &EventBook, _state: Option<&EventBook>) -> Vec<Cover> {
        vec![]
    }

    fn handle(
        &self,
        _trigger: &EventBook,
        _process_state: Option<&EventBook>,
        _destinations: &[EventBook],
    ) -> (Vec<CommandBook>, Option<EventBook>) {
        (vec![], None)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn test_topology_contains_all_component_types() {
    // Create topology projector with in-memory SQLite
    let opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("failed to create topology pool");
    let store = Arc::new(SqliteTopologyStore::new(pool));
    let topology = Arc::new(TopologyProjector::new(store.clone(), 0));

    // Build runtime with all 4 component types + topology projector
    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_aggregate("inventory", EchoAggregate::new())
        .register_projector(
            "web",
            NoopProjector,
            ProjectorConfig::async_().with_domains(vec!["orders".into()]),
        )
        .register_saga(
            "fulfillment-saga",
            NoopSaga,
            SagaConfig::new("orders", "inventory"),
        )
        .register_process_manager(
            "test-pm",
            NoopProcessManager,
            ProcessManagerConfig::new("test-pm"),
        )
        .register_topology(topology, ProjectorConfig::async_())
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    // Query the store directly to verify all nodes are registered
    let nodes = store.get_nodes().await.expect("get_nodes failed");

    let find_type = |name: &str| -> Option<String> {
        nodes.iter().find(|n| n.id == name).map(|n| n.component_type.clone())
    };

    assert_eq!(
        find_type("orders"),
        Some("aggregate".into()),
        "orders should be registered as aggregate. Nodes: {:?}",
        nodes.iter().map(|n| (&n.id, &n.component_type)).collect::<Vec<_>>()
    );
    assert_eq!(
        find_type("inventory"),
        Some("aggregate".into()),
        "inventory should be registered as aggregate"
    );
    assert_eq!(
        find_type("web"),
        Some("projector".into()),
        "web should be registered as projector. Nodes: {:?}",
        nodes.iter().map(|n| (&n.id, &n.component_type)).collect::<Vec<_>>()
    );
    assert_eq!(
        find_type("fulfillment-saga"),
        Some("saga".into()),
        "fulfillment-saga should be registered as saga. Nodes: {:?}",
        nodes.iter().map(|n| (&n.id, &n.component_type)).collect::<Vec<_>>()
    );
    assert_eq!(
        find_type("test-pm"),
        Some("process_manager".into()),
        "test-pm should be registered as process_manager. Nodes: {:?}",
        nodes.iter().map(|n| (&n.id, &n.component_type)).collect::<Vec<_>>()
    );

    // Verify subscription edges exist
    let edges = store.get_edges().await.expect("get_edges failed");
    let has_edge = |source: &str, target: &str| {
        edges.iter().any(|e| e.source == source && e.target == target)
    };

    assert!(
        has_edge("orders", "fulfillment-saga"),
        "Should have subscription edge orders -> fulfillment-saga. Edges: {:?}",
        edges.iter().map(|e| (&e.source, &e.target)).collect::<Vec<_>>()
    );
    assert!(
        has_edge("orders", "web"),
        "Should have subscription edge orders -> web"
    );
    assert!(
        has_edge("orders", "test-pm"),
        "Should have subscription edge orders -> test-pm"
    );
}

#[tokio::test]
async fn test_topology_rest_endpoint_returns_all_component_types() {
    // Create topology projector with in-memory SQLite
    let opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("failed to create topology pool");
    let store = Arc::new(SqliteTopologyStore::new(pool));
    let topology = Arc::new(TopologyProjector::new(store.clone(), 0));

    // Build runtime with all 4 component types
    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_projector(
            "accounting",
            NoopProjector,
            ProjectorConfig::async_().with_domains(vec!["orders".into()]),
        )
        .register_saga(
            "fulfillment-saga",
            NoopSaga,
            SagaConfig::new("orders", "inventory"),
        )
        .register_process_manager(
            "order-pm",
            NoopProcessManager,
            ProcessManagerConfig::new("order-pm"),
        )
        .register_topology(topology, ProjectorConfig::async_())
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    // Query the REST endpoint
    let app = router(store as Arc<dyn TopologyStore>);
    let req = Request::builder()
        .uri("/api/graph/data")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let nodes = json["nodes"].as_array().expect("nodes should be array");

    // Collect component types from the REST response
    let component_types: Vec<&str> = nodes
        .iter()
        .filter_map(|n| n["detail__component_type"].as_str())
        .collect();

    assert!(
        component_types.contains(&"aggregate"),
        "REST response should contain aggregate nodes. Got: {:?}",
        component_types
    );
    assert!(
        component_types.contains(&"projector"),
        "REST response should contain projector nodes. Got: {:?}",
        component_types
    );
    assert!(
        component_types.contains(&"saga"),
        "REST response should contain saga nodes. Got: {:?}",
        component_types
    );
    assert!(
        component_types.contains(&"process_manager"),
        "REST response should contain process_manager nodes. Got: {:?}",
        component_types
    );
}
