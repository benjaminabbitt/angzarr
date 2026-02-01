//! Product bounded context client logic.
//!
//! Handles product catalog lifecycle and pricing.

use prost::Message;

use angzarr::proto::{BusinessResponse, CommandBook, ContextualCommand, Cover, EventBook};
use common::proto::{
    CreateProduct, Discontinue, PriceSet, ProductCreated, ProductDiscontinued, ProductState,
    ProductUpdated, SetPrice, UpdateProduct,
};
use common::{decode_command, dispatch_aggregate, make_event_book, now, unknown_command};
use common::{
    rebuild_from_events, require_exists, require_not_exists, require_positive, require_status_not,
};
use common::{AggregateLogic, Result};

const STATE_TYPE_URL: &str = "type.examples/examples.ProductState";

pub mod errmsg {
    pub const PRODUCT_EXISTS: &str = "Product already exists";
    pub const PRODUCT_NOT_FOUND: &str = "Product does not exist";
    pub const SKU_REQUIRED: &str = "Product sku is required";
    pub const NAME_REQUIRED: &str = "Product name is required";
    pub const PRICE_POSITIVE: &str = "Price must be positive";
    pub const PRODUCT_DISCONTINUED: &str = "Product is discontinued";
    pub const ALREADY_DISCONTINUED: &str = "Product is already discontinued";
    pub use common::errmsg::*;
}

pub fn apply_event(state: &mut ProductState, event: &prost_types::Any) {
    if event.type_url.ends_with("ProductCreated") {
        if let Ok(e) = ProductCreated::decode(event.value.as_slice()) {
            state.sku = e.sku;
            state.name = e.name;
            state.description = e.description;
            state.price_cents = e.price_cents;
            state.status = "active".to_string();
        }
    } else if event.type_url.ends_with("ProductUpdated") {
        if let Ok(e) = ProductUpdated::decode(event.value.as_slice()) {
            state.name = e.name;
            state.description = e.description;
        }
    } else if event.type_url.ends_with("PriceSet") {
        if let Ok(e) = PriceSet::decode(event.value.as_slice()) {
            state.price_cents = e.price_cents;
        }
    } else if event.type_url.ends_with("ProductDiscontinued") {
        state.status = "discontinued".to_string();
    }
}

/// Apply an event and build an EventBook response with updated snapshot.
fn build_event_response(
    state: &ProductState,
    cover: Option<Cover>,
    next_seq: u32,
    event_type_url: &str,
    event: impl Message,
) -> EventBook {
    let event_bytes = event.encode_to_vec();
    let any = prost_types::Any {
        type_url: event_type_url.to_string(),
        value: event_bytes.clone(),
    };
    let mut new_state = state.clone();
    apply_event(&mut new_state, &any);

    make_event_book(
        cover,
        next_seq,
        event_type_url,
        event_bytes,
        STATE_TYPE_URL,
        new_state.encode_to_vec(),
    )
}

/// client logic for Product aggregate.
pub struct ProductLogic;

common::define_aggregate!(ProductLogic, "product");

common::expose_handlers!(methods, ProductLogic, ProductState, rebuild: rebuild_state, [
    (handle_create_product_public, handle_create_product),
    (handle_update_product_public, handle_update_product),
    (handle_set_price_public, handle_set_price),
    (handle_discontinue_public, handle_discontinue),
]);

impl ProductLogic {
    /// Rebuild product state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> ProductState {
        rebuild_from_events(event_book, apply_event)
    }

    fn handle_create_product(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_not_exists(&state.sku, errmsg::PRODUCT_EXISTS)?;

        let cmd: CreateProduct = decode_command(command_data)?;

        require_exists(&cmd.sku, errmsg::SKU_REQUIRED)?;
        require_exists(&cmd.name, errmsg::NAME_REQUIRED)?;
        require_positive(cmd.price_cents, errmsg::PRICE_POSITIVE)?;

        let event = ProductCreated {
            sku: cmd.sku,
            name: cmd.name,
            description: cmd.description,
            price_cents: cmd.price_cents,
            created_at: Some(now()),
        };

        Ok(build_event_response(
            state,
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.ProductCreated",
            event,
        ))
    }

    fn handle_update_product(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_exists(&state.sku, errmsg::PRODUCT_NOT_FOUND)?;

        let cmd: UpdateProduct = decode_command(command_data)?;

        let event = ProductUpdated {
            name: cmd.name,
            description: cmd.description,
            updated_at: Some(now()),
        };

        Ok(build_event_response(
            state,
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.ProductUpdated",
            event,
        ))
    }

    fn handle_set_price(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_exists(&state.sku, errmsg::PRODUCT_NOT_FOUND)?;
        require_status_not(&state.status, "discontinued", errmsg::PRODUCT_DISCONTINUED)?;

        let cmd: SetPrice = decode_command(command_data)?;

        require_positive(cmd.price_cents, errmsg::PRICE_POSITIVE)?;

        let event = PriceSet {
            price_cents: cmd.price_cents,
            previous_price_cents: state.price_cents,
            set_at: Some(now()),
        };

        Ok(build_event_response(
            state,
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.PriceSet",
            event,
        ))
    }

    fn handle_discontinue(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_exists(&state.sku, errmsg::PRODUCT_NOT_FOUND)?;
        require_status_not(&state.status, "discontinued", errmsg::ALREADY_DISCONTINUED)?;

        let cmd: Discontinue = decode_command(command_data)?;

        let event = ProductDiscontinued {
            reason: cmd.reason,
            discontinued_at: Some(now()),
        };

        Ok(build_event_response(
            state,
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.ProductDiscontinued",
            event,
        ))
    }
}

#[tonic::async_trait]
impl AggregateLogic for ProductLogic {
    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        dispatch_aggregate(
            cmd,
            |eb| self.rebuild_state(eb),
            |cb, command_any, state, next_seq| {
                if command_any.type_url.ends_with("CreateProduct") {
                    self.handle_create_product(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("UpdateProduct") {
                    self.handle_update_product(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("SetPrice") {
                    self.handle_set_price(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("Discontinue") {
                    self.handle_discontinue(cb, &command_any.value, state, next_seq)
                } else {
                    Err(unknown_command(&command_any.type_url))
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{event_page::Sequence, Cover, EventPage, Uuid as ProtoUuid};
    use common::testing::{extract_response_events, make_test_command_book};

    #[tokio::test]
    async fn test_create_product_success() {
        let logic = ProductLogic::new();

        let cmd = CreateProduct {
            sku: "SKU-001".to_string(),
            name: "Widget".to_string(),
            description: "A useful widget".to_string(),
            price_cents: 1999,
        };

        let command_book = make_test_command_book(
            "product",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            "type.examples/examples.CreateProduct",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let response = logic.handle(ctx).await.unwrap();
        let result = extract_response_events(response);
        assert_eq!(result.pages.len(), 1);

        let event =
            ProductCreated::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.sku, "SKU-001");
        assert_eq!(event.name, "Widget");
        assert_eq!(event.price_cents, 1999);
    }

    #[tokio::test]
    async fn test_create_product_already_exists() {
        let logic = ProductLogic::new();

        let prior = EventBook {
            cover: Some(Cover {
                domain: "product".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ProductCreated".to_string(),
                    value: ProductCreated {
                        sku: "SKU-001".to_string(),
                        name: "Existing".to_string(),
                        description: "".to_string(),
                        price_cents: 1000,
                        created_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot_state: None,
        };

        let cmd = CreateProduct {
            sku: "SKU-002".to_string(),
            name: "New".to_string(),
            description: "".to_string(),
            price_cents: 2000,
        };

        let command_book = make_test_command_book(
            "product",
            &[1; 16],
            "type.examples/examples.CreateProduct",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let result = logic.handle(ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_set_price_success() {
        let logic = ProductLogic::new();

        let prior = EventBook {
            cover: Some(Cover {
                domain: "product".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ProductCreated".to_string(),
                    value: ProductCreated {
                        sku: "SKU-001".to_string(),
                        name: "Widget".to_string(),
                        description: "".to_string(),
                        price_cents: 1000,
                        created_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot_state: None,
        };

        let cmd = SetPrice { price_cents: 1500 };

        let command_book = make_test_command_book(
            "product",
            &[1; 16],
            "type.examples/examples.SetPrice",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let response = logic.handle(ctx).await.unwrap();
        let result = extract_response_events(response);
        assert_eq!(result.pages.len(), 1);

        let event =
            PriceSet::decode(result.pages[0].event.as_ref().unwrap().value.as_slice()).unwrap();
        assert_eq!(event.price_cents, 1500);
        assert_eq!(event.previous_price_cents, 1000);
    }

    #[tokio::test]
    async fn test_discontinue_product() {
        let logic = ProductLogic::new();

        let prior = EventBook {
            cover: Some(Cover {
                domain: "product".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ProductCreated".to_string(),
                    value: ProductCreated {
                        sku: "SKU-001".to_string(),
                        name: "Widget".to_string(),
                        description: "".to_string(),
                        price_cents: 1000,
                        created_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot_state: None,
        };

        let cmd = Discontinue {
            reason: "End of life".to_string(),
        };

        let command_book = make_test_command_book(
            "product",
            &[1; 16],
            "type.examples/examples.Discontinue",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let response = logic.handle(ctx).await.unwrap();
        let result = extract_response_events(response);
        assert_eq!(result.pages.len(), 1);

        let event =
            ProductDiscontinued::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.reason, "End of life");
    }
}
