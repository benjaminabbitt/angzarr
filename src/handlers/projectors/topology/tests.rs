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

        projector
            .process_event(&book1)
            .await
            .expect("process_event failed");
        projector
            .process_event(&book2)
            .await
            .expect("process_event failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].event_count, 2);
        assert_eq!(nodes[0].last_event_type, "OrderConfirmed");
    }

    #[tokio::test]
    async fn test_declarative_edges_from_inputs() {
        use crate::proto::{ComponentDescriptor, Target};

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        // Edges come from descriptor inputs, not correlation
        let descriptors = vec![
            ComponentDescriptor {
                name: "orders".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "order-fulfillment".into(),
                component_type: "saga".into(),
                inputs: vec![Target {
                    domain: "orders".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
        ];
        projector
            .register_components(&descriptors)
            .await
            .expect("register failed");

        let edges = store.get_edges().await.expect("get_edges failed");
        // Edges: orders -> order-fulfillment (subscription)
        assert_eq!(edges.len(), 1);
    }

    #[tokio::test]
    async fn test_no_edge_between_aggregates_without_intermediary() {
        use crate::proto::ComponentDescriptor;

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        // Register both domains as aggregates — no intermediary saga
        let descriptors = vec![
            ComponentDescriptor {
                name: "orders".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "fulfillment".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
        ];
        projector
            .register_components(&descriptors)
            .await
            .expect("register failed");

        // Correlated events between two aggregates — NO edge created (declarative only)
        let book1 = make_event_book("orders", "corr-1", &["OrderPlaced"]);
        let book2 = make_event_book("fulfillment", "corr-1", &["ShipmentCreated"]);

        projector
            .process_event(&book1)
            .await
            .expect("process_event failed");
        projector
            .process_event(&book2)
            .await
            .expect("process_event failed");

        // No edges — edges come only from descriptors now
        let edges = store.get_edges().await.expect("get_edges failed");
        assert!(
            edges.is_empty(),
            "Aggregates should not have direct edges without saga intermediary"
        );
    }

    #[tokio::test]
    async fn test_subscription_edge_from_aggregate_to_saga() {
        use crate::proto::{ComponentDescriptor, Target};

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let descriptors = vec![
            ComponentDescriptor {
                name: "orders".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "fulfillment-saga".into(),
                component_type: "saga".into(),
                inputs: vec![Target {
                    domain: "orders".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
        ];
        projector
            .register_components(&descriptors)
            .await
            .expect("register failed");

        // Edges come from descriptors: orders -> fulfillment-saga (subscription)
        let edges = store.get_edges().await.expect("get_edges failed");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source, "orders");
        assert_eq!(edges[0].target, "fulfillment-saga");
        // Note: edge_type is "subscription" in register_components, verified by edge existence
    }

    #[tokio::test]
    async fn test_no_edge_without_correlation() {
        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book1 = make_event_book("orders", "", &["OrderPlaced"]);
        let book2 = make_event_book("fulfillment", "", &["ShipmentCreated"]);

        projector
            .process_event(&book1)
            .await
            .expect("process_event failed");
        projector
            .process_event(&book2)
            .await
            .expect("process_event failed");

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
        projector
            .process_event(&book)
            .await
            .expect("process_event failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].component_type, "projector");
    }

    #[tokio::test]
    async fn test_prune_correlations() {
        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let book = make_event_book("orders", "old-corr", &["OrderPlaced"]);
        projector
            .process_event(&book)
            .await
            .expect("process_event failed");

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

    #[tokio::test]
    async fn test_register_components_creates_nodes_and_edges() {
        use crate::proto::{ComponentDescriptor, Target};

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let descriptors = vec![
            ComponentDescriptor {
                name: "orders".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "inventory".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "fulfillment-saga".into(),
                component_type: "saga".into(),
                inputs: vec![Target {
                    domain: "orders".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
            ComponentDescriptor {
                name: "accounting".into(),
                component_type: "projector".into(),
                inputs: vec![
                    Target {
                        domain: "orders".into(),
                        types: vec![],
                    },
                    Target {
                        domain: "inventory".into(),
                        types: vec![],
                    },
                ],
                ..Default::default()
            },
        ];

        projector
            .register_components(&descriptors)
            .await
            .expect("register_components failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 4);

        let saga_node = nodes
            .iter()
            .find(|n| n.id == "fulfillment-saga")
            .expect("saga node missing");
        assert_eq!(saga_node.component_type, "saga");

        let projector_node = nodes
            .iter()
            .find(|n| n.id == "accounting")
            .expect("projector node missing");
        assert_eq!(projector_node.component_type, "projector");

        let aggregate_node = nodes
            .iter()
            .find(|n| n.id == "orders")
            .expect("aggregate node missing");
        assert_eq!(aggregate_node.component_type, "aggregate");

        // Subscription edges: orders->fulfillment-saga, orders->accounting, inventory->accounting
        let edges = store.get_edges().await.expect("get_edges failed");
        assert_eq!(edges.len(), 3);

        let has_edge = |source: &str, target: &str| {
            edges
                .iter()
                .any(|e| e.source == source && e.target == target)
        };
        assert!(has_edge("orders", "fulfillment-saga"));
        assert!(has_edge("orders", "accounting"));
        assert!(has_edge("inventory", "accounting"));
    }

    #[tokio::test]
    async fn test_register_components_input_domain_not_in_batch() {
        use crate::proto::{ComponentDescriptor, Target};

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        // Saga subscribes to "orders" but "orders" is NOT in the descriptor batch.
        // The edge source node doesn't exist — must not FK-fail.
        let descriptors = vec![ComponentDescriptor {
            name: "fulfillment-saga".into(),
            component_type: "saga".into(),
            inputs: vec![Target {
                domain: "orders".into(),
                types: vec![],
            }],
            ..Default::default()
        }];

        projector
            .register_components(&descriptors)
            .await
            .expect("register_components failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "fulfillment-saga");

        // Edge should be skipped (source node doesn't exist)
        let edges = store.get_edges().await.expect("get_edges failed");
        assert!(edges.is_empty());
    }

    #[tokio::test]
    async fn test_register_components_skips_empty_names() {
        use crate::proto::ComponentDescriptor;

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        let descriptors = vec![
            ComponentDescriptor {
                name: String::new(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "orders".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
        ];

        projector
            .register_components(&descriptors)
            .await
            .expect("register_components failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "orders");
    }

    #[tokio::test]
    async fn test_descriptor_publish_roundtrip_preserves_component_type() {
        use crate::proto::{ComponentDescriptor, Target};

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        // Build descriptors with correct component types.
        // All referenced input domains must have corresponding nodes.
        let descriptors = vec![
            ComponentDescriptor {
                name: "order".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "fulfillment".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "fulfillment-saga".into(),
                component_type: "saga".into(),
                inputs: vec![Target {
                    domain: "order".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
            ComponentDescriptor {
                name: "web".into(),
                component_type: "projector".into(),
                inputs: vec![Target {
                    domain: "order".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
            ComponentDescriptor {
                name: "order-fulfillment".into(),
                component_type: "process_manager".into(),
                inputs: vec![
                    Target {
                        domain: "order".into(),
                        types: vec![],
                    },
                    Target {
                        domain: "fulfillment".into(),
                        types: vec![],
                    },
                ],
                ..Default::default()
            },
        ];

        // Simulate publish_descriptors: encode to EventBook
        use crate::proto::ComponentRegistered;
        use prost::Message;
        let pages: Vec<EventPage> = descriptors
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let registered = ComponentRegistered {
                    descriptor: Some(d.clone()),
                    ..Default::default()
                };
                EventPage {
                    sequence: Some(crate::proto::event_page::Sequence::Num(i as u32)),
                    event: Some(Any {
                        type_url: crate::proto_ext::COMPONENT_REGISTERED_TYPE_URL.to_string(),
                        value: registered.encode_to_vec(),
                    }),
                    created_at: None,
                }
            })
            .collect();

        let meta_book = EventBook {
            cover: Some(Cover {
                domain: crate::proto_ext::META_ANGZARR_DOMAIN.to_string(),
                correlation_id: String::new(),
                ..Default::default()
            }),
            pages,
            ..Default::default()
        };

        // Process the meta-event (as topology projector would receive from bus)
        projector
            .process_event(&meta_book)
            .await
            .expect("process_event failed for meta-event");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 5);

        let saga_node = nodes
            .iter()
            .find(|n| n.id == "fulfillment-saga")
            .expect("saga node missing");
        assert_eq!(saga_node.component_type, "saga");

        let projector_node = nodes
            .iter()
            .find(|n| n.id == "web")
            .expect("projector node missing");
        assert_eq!(projector_node.component_type, "projector");

        let pm_node = nodes
            .iter()
            .find(|n| n.id == "order-fulfillment")
            .expect("PM node missing");
        assert_eq!(pm_node.component_type, "process_manager");

        let agg_node = nodes
            .iter()
            .find(|n| n.id == "order")
            .expect("aggregate node missing");
        assert_eq!(agg_node.component_type, "aggregate");

        // Now process domain events — verify registered types are NOT overwritten
        let domain_book = make_event_book("order", "", &["OrderPlaced"]);
        projector
            .process_event(&domain_book)
            .await
            .expect("process_event failed for domain event");

        let nodes = store.get_nodes().await.expect("get_nodes failed");
        let agg_node = nodes
            .iter()
            .find(|n| n.id == "order")
            .expect("aggregate node missing after event");
        assert_eq!(agg_node.component_type, "aggregate");
        assert_eq!(agg_node.event_count, 1); // 0 from register + 1 from event

        // Saga/projector/PM types still preserved
        let saga_node = nodes
            .iter()
            .find(|n| n.id == "fulfillment-saga")
            .expect("saga node missing after event");
        assert_eq!(saga_node.component_type, "saga");
    }

    #[tokio::test]
    async fn test_register_node_overwrites_event_inferred_type() {
        use crate::proto::{ComponentDescriptor, Target};

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        // Events arrive FIRST — all nodes created as "aggregate"
        let book1 = make_event_book("order", "", &["OrderPlaced"]);
        let book2 = make_event_book("fulfillment-saga", "", &["SagaStarted"]);
        let book3 = make_event_book("web", "", &["ViewUpdated"]);
        let book4 = make_event_book("order-fulfillment", "", &["ProcessStarted"]);

        projector
            .process_event(&book1)
            .await
            .expect("process_event failed");
        projector
            .process_event(&book2)
            .await
            .expect("process_event failed");
        projector
            .process_event(&book3)
            .await
            .expect("process_event failed");
        projector
            .process_event(&book4)
            .await
            .expect("process_event failed");

        // Verify all nodes initially have type "aggregate"
        let nodes = store.get_nodes().await.expect("get_nodes failed");
        assert_eq!(nodes.len(), 4);
        for node in &nodes {
            assert_eq!(
                node.component_type, "aggregate",
                "node {} should be aggregate before registration",
                node.id
            );
        }

        // Descriptors arrive AFTER — register_node must overwrite component_type
        let descriptors = vec![
            ComponentDescriptor {
                name: "order".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "fulfillment-saga".into(),
                component_type: "saga".into(),
                inputs: vec![Target {
                    domain: "order".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
            ComponentDescriptor {
                name: "web".into(),
                component_type: "projector".into(),
                inputs: vec![Target {
                    domain: "order".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
            ComponentDescriptor {
                name: "order-fulfillment".into(),
                component_type: "process_manager".into(),
                inputs: vec![Target {
                    domain: "order".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
        ];

        projector
            .register_components(&descriptors)
            .await
            .expect("register_components failed");

        let nodes = store.get_nodes().await.expect("get_nodes failed");

        let find = |id: &str| {
            nodes
                .iter()
                .find(|n| n.id == id)
                .expect(&format!("node {} missing", id))
        };

        assert_eq!(find("order").component_type, "aggregate");
        assert_eq!(find("fulfillment-saga").component_type, "saga");
        assert_eq!(find("web").component_type, "projector");
        assert_eq!(find("order-fulfillment").component_type, "process_manager");

        // Event counts preserved from initial process_event calls
        assert_eq!(find("order").event_count, 1);
        assert_eq!(find("fulfillment-saga").event_count, 1);
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
        use crate::proto::{ComponentDescriptor, Target};
        use axum::body::Body;
        use http::Request;
        use tower::ServiceExt;

        let store = test_store().await;
        let projector = TopologyProjector::new(store.clone(), 0);

        // Register components with declarative edges
        let descriptors = vec![
            ComponentDescriptor {
                name: "orders".into(),
                component_type: "aggregate".into(),
                ..Default::default()
            },
            ComponentDescriptor {
                name: "order-fulfillment".into(),
                component_type: "saga".into(),
                inputs: vec![Target {
                    domain: "orders".into(),
                    types: vec![],
                }],
                ..Default::default()
            },
        ];
        projector.register_components(&descriptors).await.unwrap();

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
        // 2 nodes: orders, order-fulfillment
        assert_eq!(json["nodes"].as_array().unwrap().len(), 2);
        // 1 edge: orders->order-fulfillment
        assert_eq!(json["edges"].as_array().unwrap().len(), 1);

        let node = &json["nodes"][0];
        assert!(node["id"].is_string());
        assert!(node["title"].is_string());
        assert!(node["mainStat"].is_string());
        assert!(node["secondaryStat"].is_string());
        assert!(node["color"].is_string());
    }
}
