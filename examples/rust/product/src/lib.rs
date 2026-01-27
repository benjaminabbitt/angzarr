//! Product bounded context business logic.
//!
//! Handles product catalog lifecycle and pricing.

use prost::Message;

use angzarr::proto::{
    business_response, event_page::Sequence, BusinessResponse, CommandBook, ContextualCommand,
    EventBook, EventPage,
};
use common::next_sequence;
use common::proto::{
    CreateProduct, Discontinue, PriceSet, ProductCreated, ProductDiscontinued, ProductState,
    ProductUpdated, SetPrice, UpdateProduct,
};
use common::{AggregateLogic, BusinessError, Result};

pub mod errmsg {
    pub const PRODUCT_EXISTS: &str = "Product already exists";
    pub const PRODUCT_NOT_FOUND: &str = "Product does not exist";
    pub const SKU_REQUIRED: &str = "Product sku is required";
    pub const NAME_REQUIRED: &str = "Product name is required";
    pub const PRICE_POSITIVE: &str = "Price must be positive";
    pub const PRODUCT_DISCONTINUED: &str = "Product is discontinued";
    pub const ALREADY_DISCONTINUED: &str = "Product is already discontinued";
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}

/// Business logic for Product aggregate.
pub struct ProductLogic {
    domain: String,
}

impl ProductLogic {
    pub const DOMAIN: &'static str = "product";

    pub fn new() -> Self {
        Self {
            domain: Self::DOMAIN.to_string(),
        }
    }

    /// Rebuild product state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> ProductState {
        let mut state = ProductState::default();

        let Some(book) = event_book else {
            return state;
        };

        // Start from snapshot if present
        if let Some(snapshot) = &book.snapshot {
            if let Some(snapshot_state) = &snapshot.state {
                if let Ok(s) = ProductState::decode(snapshot_state.value.as_slice()) {
                    state = s;
                }
            }
        }

        // Apply events
        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

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

        state
    }

    fn handle_create_product(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if !state.sku.is_empty() {
            return Err(BusinessError::Rejected(errmsg::PRODUCT_EXISTS.to_string()));
        }

        let cmd = CreateProduct::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.sku.is_empty() {
            return Err(BusinessError::Rejected(errmsg::SKU_REQUIRED.to_string()));
        }
        if cmd.name.is_empty() {
            return Err(BusinessError::Rejected(errmsg::NAME_REQUIRED.to_string()));
        }
        if cmd.price_cents <= 0 {
            return Err(BusinessError::Rejected(errmsg::PRICE_POSITIVE.to_string()));
        }

        let event = ProductCreated {
            sku: cmd.sku.clone(),
            name: cmd.name.clone(),
            description: cmd.description.clone(),
            price_cents: cmd.price_cents,
            created_at: Some(now()),
        };

        let new_state = ProductState {
            sku: cmd.sku,
            name: cmd.name,
            description: cmd.description,
            price_cents: cmd.price_cents,
            status: "active".to_string(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ProductCreated".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.ProductState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_update_product(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.sku.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::PRODUCT_NOT_FOUND.to_string(),
            ));
        }

        let cmd = UpdateProduct::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = ProductUpdated {
            name: cmd.name.clone(),
            description: cmd.description.clone(),
            updated_at: Some(now()),
        };

        let new_state = ProductState {
            sku: state.sku.clone(),
            name: cmd.name,
            description: cmd.description,
            price_cents: state.price_cents,
            status: state.status.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ProductUpdated".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.ProductState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_set_price(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.sku.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::PRODUCT_NOT_FOUND.to_string(),
            ));
        }
        if state.status == "discontinued" {
            return Err(BusinessError::Rejected(
                errmsg::PRODUCT_DISCONTINUED.to_string(),
            ));
        }

        let cmd =
            SetPrice::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.price_cents <= 0 {
            return Err(BusinessError::Rejected(errmsg::PRICE_POSITIVE.to_string()));
        }

        let event = PriceSet {
            price_cents: cmd.price_cents,
            previous_price_cents: state.price_cents,
            set_at: Some(now()),
        };

        let new_state = ProductState {
            sku: state.sku.clone(),
            name: state.name.clone(),
            description: state.description.clone(),
            price_cents: cmd.price_cents,
            status: state.status.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.PriceSet".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.ProductState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_discontinue(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.sku.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::PRODUCT_NOT_FOUND.to_string(),
            ));
        }
        if state.status == "discontinued" {
            return Err(BusinessError::Rejected(
                errmsg::ALREADY_DISCONTINUED.to_string(),
            ));
        }

        let cmd = Discontinue::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = ProductDiscontinued {
            reason: cmd.reason,
            discontinued_at: Some(now()),
        };

        let new_state = ProductState {
            sku: state.sku.clone(),
            name: state.name.clone(),
            description: state.description.clone(),
            price_cents: state.price_cents,
            status: "discontinued".to_string(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ProductDiscontinued".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.ProductState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }
}

impl Default for ProductLogic {
    fn default() -> Self {
        Self::new()
    }
}

// Public test methods for cucumber tests
impl ProductLogic {
    /// Public access to rebuild_state for testing.
    pub fn rebuild_state_public(&self, event_book: Option<&EventBook>) -> ProductState {
        self.rebuild_state(event_book)
    }

    /// Public access to handle_create_product for testing.
    pub fn handle_create_product_public(
        &self,
        command_book: &CommandBook,
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_create_product(command_book, &command_any.value, state, next_seq)
    }

    /// Public access to handle_update_product for testing.
    pub fn handle_update_product_public(
        &self,
        command_book: &CommandBook,
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_update_product(command_book, &command_any.value, state, next_seq)
    }

    /// Public access to handle_set_price for testing.
    pub fn handle_set_price_public(
        &self,
        command_book: &CommandBook,
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_set_price(command_book, &command_any.value, state, next_seq)
    }

    /// Public access to handle_discontinue for testing.
    pub fn handle_discontinue_public(
        &self,
        command_book: &CommandBook,
        state: &ProductState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_discontinue(command_book, &command_any.value, state, next_seq)
    }
}

#[tonic::async_trait]
impl AggregateLogic for ProductLogic {
    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = self.rebuild_state(prior_events);
        let next_seq = next_sequence(prior_events);

        let Some(cb) = command_book else {
            return Err(BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()).into());
        };

        let command_page = cb
            .pages
            .first()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let command_any = command_page
            .command
            .as_ref()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let events = if command_any.type_url.ends_with("CreateProduct") {
            self.handle_create_product(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("UpdateProduct") {
            self.handle_update_product(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("SetPrice") {
            self.handle_set_price(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("Discontinue") {
            self.handle_discontinue(cb, &command_any.value, &state, next_seq)?
        } else {
            return Err(BusinessError::Rejected(format!(
                "{}: {}",
                errmsg::UNKNOWN_COMMAND,
                command_any.type_url
            ))
            .into());
        };

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(events)),
        })
    }
}

fn now() -> prost_types::Timestamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    prost_types::Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{CommandPage, Cover, Uuid as ProtoUuid};

    fn make_command_book(domain: &str, root: &[u8], type_url: &str, value: Vec<u8>) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.to_vec(),
                }),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(prost_types::Any {
                    type_url: type_url.to_string(),
                    value,
                }),
            }],
            correlation_id: String::new(),
            saga_origin: None,
        }
    }

    fn extract_events(response: BusinessResponse) -> EventBook {
        match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events in response"),
        }
    }

    #[tokio::test]
    async fn test_create_product_success() {
        let logic = ProductLogic::new();

        let cmd = CreateProduct {
            sku: "SKU-001".to_string(),
            name: "Widget".to_string(),
            description: "A useful widget".to_string(),
            price_cents: 1999,
        };

        let command_book = make_command_book(
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
        let result = extract_events(response);
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
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let cmd = CreateProduct {
            sku: "SKU-002".to_string(),
            name: "New".to_string(),
            description: "".to_string(),
            price_cents: 2000,
        };

        let command_book = make_command_book(
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
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let cmd = SetPrice { price_cents: 1500 };

        let command_book = make_command_book(
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
        let result = extract_events(response);
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
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let cmd = Discontinue {
            reason: "End of life".to_string(),
        };

        let command_book = make_command_book(
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
        let result = extract_events(response);
        assert_eq!(result.pages.len(), 1);

        let event =
            ProductDiscontinued::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.reason, "End of life");
    }
}
