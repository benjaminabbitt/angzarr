//! Unit tests for the topology projector (SQLite in-memory).

#[cfg(feature = "sqlite")]
mod sqlite_tests {
    use std::sync::Arc;

    use prost_types::Any;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use crate::handlers::projectors::topology::store::TopologyStore;
    use crate::handlers::projectors::topology::TopologyProjector;
    use crate::proto::{Cover, EventBook, EventPage};
    use crate::storage::sqlite::SqliteTopologyStore;

    async fn test_store() -> Arc<SqliteTopologyStore> {
        let opts = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("failed to create in-memory pool");

        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("failed to enable foreign keys");

        let store = Arc::new(SqliteTopologyStore::new(pool));
        store.init_schema().await.expect("failed to init schema");
        store
    }

    fn make_event_book(domain: &str, correlation_id: &str, event_types: &[&str]) -> EventBook {
        let pages = event_types
            .iter()
            .map(|t| EventPage {
                event: Some(Any {
                    type_url: format!("type.googleapis.com/test.{}", t),
                    value: vec![],
                }),
                ..Default::default()
            })
            .collect();

        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                correlation_id: correlation_id.to_string(),
                ..Default::default()
            }),
            pages,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_handle_event_creates_node() {
        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book = make_event_book("orders", "", &["OrderPlaced"]);
        projector
            .process_event(&book)
            .await
            .expect("process_event failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "orders");
        assert_eq!(nodes[0].component_type, "aggregate");
        assert_eq!(nodes[0].domain, "orders");
    }

    #[tokio::test]
    async fn test_handle_event_increments_count() {
        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book1 = make_event_book("orders", "", &["OrderPlaced"]);
        let book2 = make_event_book("orders", "", &["OrderConfirmed"]);

        projector.process_event(&book1).await.expect("process_event failed");
        projector.process_event(&book2).await.expect("process_event failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].event_count, 2);
        assert_eq!(nodes[0].last_event_type, "OrderConfirmed");
    }

    #[tokio::test]
    async fn test_correlation_discovers_edge() {
        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book1 = make_event_book("orders", "corr-1", &["OrderPlaced"]);
        let book2 = make_event_book("fulfillment", "corr-1", &["ShipmentCreated"]);

        projector.process_event(&book1).await.expect("process_event failed");
        projector.process_event(&book2).await.expect("process_event failed");

        let edges = store.get_edges().await.expect("get_edges failed");
        assert_eq!(edges.len(), 1);
        // Alphabetical: fulfillment < orders
        assert_eq!(edges[0].source, "fulfillment");
        assert_eq!(edges[0].target, "orders");
    }

    #[tokio::test]
    async fn test_no_edge_without_correlation() {
        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book1 = make_event_book("orders", "", &["OrderPlaced"]);
        let book2 = make_event_book("fulfillment", "", &["ShipmentCreated"]);

        projector.process_event(&book1).await.expect("process_event failed");
        projector.process_event(&book2).await.expect("process_event failed");

        let edges = store.get_edges().await.expect("get_edges failed");
        assert!(edges.is_empty());
    }

    #[tokio::test]
    async fn test_speculate_mode_noop() {
        use crate::standalone::{ProjectionMode, ProjectorHandler};

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book = make_event_book("orders", "corr-1", &["OrderPlaced"]);
        projector
            .handle(&book, ProjectionMode::Speculate)
            .await
            .expect("handle failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn test_projection_domain_detected() {
        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book = make_event_book("_projection.web.order", "", &["OrderView"]);
        projector.process_event(&book).await.expect("process_event failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].component_type, "projector");
    }

    #[tokio::test]
    async fn test_prune_correlations() {
        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book = make_event_book("orders", "old-corr", &["OrderPlaced"]);
        projector.process_event(&book).await.expect("process_event failed");

        let pruned = store
            .prune_correlations("2099-01-01T00:00:00Z")
            .await
            .expect("prune failed");
        assert_eq!(pruned, 1);

        // Verify correlations are gone
        let domains = store
            .record_correlation("old-corr", "orders", "OrderPlaced", "2099-01-01T00:00:00Z")
            .await
            .expect("record failed");
        assert_eq!(domains.len(), 1);
    }

    #[cfg(feature = "topology")]
    #[tokio::test]
    async fn test_rest_health() {
        use axum::body::Body;
        use http::Request;
        use tower::ServiceExt;

        let store = test_store().await;
        let app = super::super::rest::router(store as Arc<dyn TopologyStore>);

        let req = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
    }

    #[cfg(feature = "topology")]
    #[tokio::test]
    async fn test_rest_graph_data_format() {
        use axum::body::Body;
        use http::Request;
        use tower::ServiceExt;

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book1 = make_event_book("orders", "corr-1", &["OrderPlaced"]);
        let book2 = make_event_book("fulfillment", "corr-1", &["ShipmentCreated"]);
        projector.process_event(&book1).await.unwrap();
        projector.process_event(&book2).await.unwrap();

        let app = super::super::rest::router(store as Arc<dyn TopologyStore>);

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

        assert!(json["nodes"].is_array());
        assert!(json["edges"].is_array());
        assert_eq!(json["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(json["edges"].as_array().unwrap().len(), 1);

        let node = &json["nodes"][0];
        assert!(node["id"].is_string());
        assert!(node["title"].is_string());
        assert!(node["mainStat"].is_string());
        assert!(node["secondaryStat"].is_string());
        assert!(node["color"].is_string());
    }
}
