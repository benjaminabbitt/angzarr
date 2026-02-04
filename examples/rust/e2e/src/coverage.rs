//! Integration tests verifying event flows through the minimal example set.

#[cfg(test)]
mod integration {
    use std::time::Duration;

    use prost::Message;
    use uuid::Uuid;

    use angzarr::proto::{CommandBook, CommandPage, Cover, Uuid as ProtoUuid};
    use common::proto as examples_proto;

    use crate::backend::{self, BackendWithProjectors};

    fn cmd(
        domain: &str,
        root: Uuid,
        correlation: &str,
        sequence: u32,
        type_url: &str,
        payload: &impl Message,
    ) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: correlation.to_string(),
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence,
                command: Some(prost_types::Any {
                    type_url: format!("type.examples/{}", type_url),
                    value: payload.encode_to_vec(),
                }),
            }],
            saga_origin: None,
        }
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    async fn exec(bp: &BackendWithProjectors, command: CommandBook) {
        bp.backend
            .execute(command)
            .await
            .expect("Command execution failed");
    }

    fn has_event(events: &[angzarr::proto::EventPage], event_type: &str) -> bool {
        events.iter().any(|page| {
            page.event
                .as_ref()
                .map(|e| e.type_url.contains(event_type))
                .unwrap_or(false)
        })
    }

    #[tokio::test]
    async fn test_inventory_emits_low_stock_alert() {
        let bp = backend::create_backend().await;
        let root = common::identity::inventory_product_root("SKU-ALERT");

        exec(
            &bp,
            cmd(
                "inventory",
                root,
                "test-alert",
                0,
                "examples.InitializeStock",
                &examples_proto::InitializeStock {
                    product_id: "SKU-ALERT".to_string(),
                    quantity: 10,
                    low_stock_threshold: 8,
                },
            ),
        )
        .await;

        // Reserve enough to drop below threshold: 10 - 5 = 5 < 8
        exec(
            &bp,
            cmd(
                "inventory",
                root,
                "test-alert",
                1,
                "examples.ReserveStock",
                &examples_proto::ReserveStock {
                    quantity: 5,
                    order_id: "order-alert".to_string(),
                },
            ),
        )
        .await;
        settle().await;

        let events = bp
            .backend
            .query_events("inventory", root)
            .await
            .expect("query failed");

        assert!(
            has_event(&events, "LowStockAlert"),
            "LowStockAlert not emitted"
        );
    }

    #[tokio::test]
    async fn test_fulfillment_lifecycle() {
        let bp = backend::create_backend().await;
        let root = Uuid::new_v4();

        exec(
            &bp,
            cmd(
                "fulfillment",
                root,
                "test-ful",
                0,
                "examples.CreateShipment",
                &examples_proto::CreateShipment {
                    order_id: "order-ful".to_string(),
                    items: vec![],
                },
            ),
        )
        .await;
        exec(
            &bp,
            cmd(
                "fulfillment",
                root,
                "test-ful",
                1,
                "examples.MarkPicked",
                &examples_proto::MarkPicked {
                    picker_id: "picker-1".to_string(),
                },
            ),
        )
        .await;
        exec(
            &bp,
            cmd(
                "fulfillment",
                root,
                "test-ful",
                2,
                "examples.MarkPacked",
                &examples_proto::MarkPacked {
                    packer_id: "packer-1".to_string(),
                },
            ),
        )
        .await;
        exec(
            &bp,
            cmd(
                "fulfillment",
                root,
                "test-ful",
                3,
                "examples.Ship",
                &examples_proto::Ship {
                    carrier: "UPS".to_string(),
                    tracking_number: "1Z999".to_string(),
                },
            ),
        )
        .await;
        exec(
            &bp,
            cmd(
                "fulfillment",
                root,
                "test-ful",
                4,
                "examples.RecordDelivery",
                &examples_proto::RecordDelivery {
                    signature: "J. Doe".to_string(),
                },
            ),
        )
        .await;
        settle().await;

        let events = bp
            .backend
            .query_events("fulfillment", root)
            .await
            .expect("query failed");

        assert!(has_event(&events, "ShipmentCreated"), "ShipmentCreated missing");
        assert!(has_event(&events, "ItemsPicked"), "ItemsPicked missing");
        assert!(has_event(&events, "ItemsPacked"), "ItemsPacked missing");
        assert!(has_event(&events, "Shipped"), "Shipped missing");
        assert!(has_event(&events, "Delivered"), "Delivered missing");
    }

    #[tokio::test]
    async fn test_saga_commits_reservation_on_ship() {
        let bp = backend::create_backend().await;
        let order_id = "order-saga-test";
        let inv_root = common::identity::inventory_product_root("SKU-SAGA");
        let ful_root = Uuid::new_v4();

        // Setup: initialize inventory and reserve stock
        exec(
            &bp,
            cmd(
                "inventory",
                inv_root,
                "test-saga",
                0,
                "examples.InitializeStock",
                &examples_proto::InitializeStock {
                    product_id: "SKU-SAGA".to_string(),
                    quantity: 100,
                    low_stock_threshold: 10,
                },
            ),
        )
        .await;
        exec(
            &bp,
            cmd(
                "inventory",
                inv_root,
                "test-saga",
                1,
                "examples.ReserveStock",
                &examples_proto::ReserveStock {
                    quantity: 5,
                    order_id: order_id.to_string(),
                },
            ),
        )
        .await;

        // Create shipment and ship it - saga should trigger CommitReservation
        exec(
            &bp,
            cmd(
                "fulfillment",
                ful_root,
                "test-saga",
                0,
                "examples.CreateShipment",
                &examples_proto::CreateShipment {
                    order_id: order_id.to_string(),
                    items: vec![examples_proto::LineItem {
                        product_id: "SKU-SAGA".to_string(),
                        name: "Test Item".to_string(),
                        quantity: 1,
                        unit_price_cents: 1000,
                        ..Default::default()
                    }],
                },
            ),
        )
        .await;
        exec(
            &bp,
            cmd(
                "fulfillment",
                ful_root,
                "test-saga",
                1,
                "examples.MarkPicked",
                &examples_proto::MarkPicked {
                    picker_id: "p".to_string(),
                },
            ),
        )
        .await;
        exec(
            &bp,
            cmd(
                "fulfillment",
                ful_root,
                "test-saga",
                2,
                "examples.MarkPacked",
                &examples_proto::MarkPacked {
                    packer_id: "p".to_string(),
                },
            ),
        )
        .await;
        exec(
            &bp,
            cmd(
                "fulfillment",
                ful_root,
                "test-saga",
                3,
                "examples.Ship",
                &examples_proto::Ship {
                    carrier: "UPS".to_string(),
                    tracking_number: "1Z".to_string(),
                },
            ),
        )
        .await;
        settle().await;

        let inv_events = bp
            .backend
            .query_events("inventory", inv_root)
            .await
            .expect("query failed");

        assert!(
            has_event(&inv_events, "ReservationCommitted"),
            "saga-fulfillment-inventory did not trigger CommitReservation"
        );
    }
}
