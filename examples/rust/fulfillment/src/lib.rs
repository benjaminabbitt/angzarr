//! Fulfillment bounded context business logic.
//!
//! Manages shipment lifecycle: pending → picking → packing → shipped → delivered.

use prost::Message;

use common::{AggregateLogic, BusinessError, Result};
use angzarr::proto::{
    business_response, event_page::Sequence, BusinessResponse, CommandBook, ContextualCommand,
    EventBook, EventPage,
};
use common::next_sequence;
use common::proto::{
    CreateShipment, Delivered, FulfillmentState, ItemsPacked, ItemsPicked, MarkPacked, MarkPicked,
    RecordDelivery, Ship, ShipmentCreated, Shipped,
};

pub mod errmsg {
    pub const SHIPMENT_EXISTS: &str = "Shipment already exists";
    pub const SHIPMENT_NOT_FOUND: &str = "Shipment does not exist";
    pub const NOT_PENDING: &str = "Shipment is not pending";
    pub const NOT_PICKED: &str = "Shipment is not picked";
    pub const NOT_PACKED: &str = "Shipment is not packed";
    pub const NOT_SHIPPED: &str = "Shipment is not shipped";
    pub const ALREADY_DELIVERED: &str = "Shipment is already delivered";
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}

/// Business logic for Fulfillment aggregate.
pub struct FulfillmentLogic {
    domain: String,
}

impl FulfillmentLogic {
    pub const DOMAIN: &'static str = "fulfillment";

    pub fn new() -> Self {
        Self {
            domain: Self::DOMAIN.to_string(),
        }
    }

    /// Rebuild fulfillment state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> FulfillmentState {
        let mut state = FulfillmentState::default();

        let Some(book) = event_book else {
            return state;
        };

        // Start from snapshot if present
        if let Some(snapshot) = &book.snapshot {
            if let Some(snapshot_state) = &snapshot.state {
                if let Ok(s) = FulfillmentState::decode(snapshot_state.value.as_slice()) {
                    state = s;
                }
            }
        }

        // Apply events
        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            if event.type_url.ends_with("ShipmentCreated") {
                if let Ok(e) = ShipmentCreated::decode(event.value.as_slice()) {
                    state.order_id = e.order_id;
                    state.status = "pending".to_string();
                }
            } else if event.type_url.ends_with("ItemsPicked") {
                if let Ok(e) = ItemsPicked::decode(event.value.as_slice()) {
                    state.picker_id = e.picker_id;
                    state.status = "picking".to_string();
                }
            } else if event.type_url.ends_with("ItemsPacked") {
                if let Ok(e) = ItemsPacked::decode(event.value.as_slice()) {
                    state.packer_id = e.packer_id;
                    state.status = "packing".to_string();
                }
            } else if event.type_url.ends_with("Shipped") {
                if let Ok(e) = Shipped::decode(event.value.as_slice()) {
                    state.carrier = e.carrier;
                    state.tracking_number = e.tracking_number;
                    state.status = "shipped".to_string();
                }
            } else if event.type_url.ends_with("Delivered") {
                if let Ok(e) = Delivered::decode(event.value.as_slice()) {
                    state.signature = e.signature;
                    state.status = "delivered".to_string();
                }
            }
        }

        state
    }

    fn handle_create_shipment(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if !state.order_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::SHIPMENT_EXISTS.to_string()));
        }

        let cmd = CreateShipment::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = ShipmentCreated {
            order_id: cmd.order_id.clone(),
            status: "pending".to_string(),
            created_at: Some(now()),
        };

        let new_state = FulfillmentState {
            order_id: cmd.order_id,
            status: "pending".to_string(),
            tracking_number: String::new(),
            carrier: String::new(),
            picker_id: String::new(),
            packer_id: String::new(),
            signature: String::new(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ShipmentCreated".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.FulfillmentState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_mark_picked(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.order_id.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::SHIPMENT_NOT_FOUND.to_string(),
            ));
        }
        if state.status != "pending" {
            return Err(BusinessError::Rejected(errmsg::NOT_PENDING.to_string()));
        }

        let cmd =
            MarkPicked::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = ItemsPicked {
            picker_id: cmd.picker_id.clone(),
            picked_at: Some(now()),
        };

        let new_state = FulfillmentState {
            order_id: state.order_id.clone(),
            status: "picking".to_string(),
            tracking_number: state.tracking_number.clone(),
            carrier: state.carrier.clone(),
            picker_id: cmd.picker_id,
            packer_id: state.packer_id.clone(),
            signature: state.signature.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ItemsPicked".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.FulfillmentState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_mark_packed(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.order_id.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::SHIPMENT_NOT_FOUND.to_string(),
            ));
        }
        if state.status != "picking" {
            return Err(BusinessError::Rejected(errmsg::NOT_PICKED.to_string()));
        }

        let cmd =
            MarkPacked::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = ItemsPacked {
            packer_id: cmd.packer_id.clone(),
            packed_at: Some(now()),
        };

        let new_state = FulfillmentState {
            order_id: state.order_id.clone(),
            status: "packing".to_string(),
            tracking_number: state.tracking_number.clone(),
            carrier: state.carrier.clone(),
            picker_id: state.picker_id.clone(),
            packer_id: cmd.packer_id,
            signature: state.signature.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ItemsPacked".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.FulfillmentState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_ship(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.order_id.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::SHIPMENT_NOT_FOUND.to_string(),
            ));
        }
        if state.status != "packing" {
            return Err(BusinessError::Rejected(errmsg::NOT_PACKED.to_string()));
        }

        let cmd = Ship::decode(command_data).map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = Shipped {
            carrier: cmd.carrier.clone(),
            tracking_number: cmd.tracking_number.clone(),
            shipped_at: Some(now()),
        };

        let new_state = FulfillmentState {
            order_id: state.order_id.clone(),
            status: "shipped".to_string(),
            tracking_number: cmd.tracking_number,
            carrier: cmd.carrier,
            picker_id: state.picker_id.clone(),
            packer_id: state.packer_id.clone(),
            signature: state.signature.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.Shipped".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.FulfillmentState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_record_delivery(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.order_id.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::SHIPMENT_NOT_FOUND.to_string(),
            ));
        }
        if state.status == "delivered" {
            return Err(BusinessError::Rejected(
                errmsg::ALREADY_DELIVERED.to_string(),
            ));
        }
        if state.status != "shipped" {
            return Err(BusinessError::Rejected(errmsg::NOT_SHIPPED.to_string()));
        }

        let cmd = RecordDelivery::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        let event = Delivered {
            signature: cmd.signature.clone(),
            delivered_at: Some(now()),
        };

        let new_state = FulfillmentState {
            order_id: state.order_id.clone(),
            status: "delivered".to_string(),
            tracking_number: state.tracking_number.clone(),
            carrier: state.carrier.clone(),
            picker_id: state.picker_id.clone(),
            packer_id: state.packer_id.clone(),
            signature: cmd.signature,
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.Delivered".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.FulfillmentState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }
}

impl Default for FulfillmentLogic {
    fn default() -> Self {
        Self::new()
    }
}

// Public test methods for cucumber tests
impl FulfillmentLogic {
    pub fn rebuild_state_public(&self, event_book: Option<&EventBook>) -> FulfillmentState {
        self.rebuild_state(event_book)
    }

    pub fn handle_create_shipment_public(
        &self,
        command_book: &CommandBook,
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_create_shipment(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_mark_picked_public(
        &self,
        command_book: &CommandBook,
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_mark_picked(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_mark_packed_public(
        &self,
        command_book: &CommandBook,
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_mark_packed(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_ship_public(
        &self,
        command_book: &CommandBook,
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_ship(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_record_delivery_public(
        &self,
        command_book: &CommandBook,
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_record_delivery(command_book, &command_any.value, state, next_seq)
    }
}

#[tonic::async_trait]
impl AggregateLogic for FulfillmentLogic {
    async fn handle(&self, cmd: ContextualCommand) -> std::result::Result<BusinessResponse, tonic::Status> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = self.rebuild_state(prior_events);
        let next_seq = next_sequence(prior_events);

        let Some(cb) = command_book else {
            return Err(BusinessError::Rejected(
                errmsg::NO_COMMAND_PAGES.to_string(),
            ).into());
        };

        let command_page = cb
            .pages
            .first()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let command_any = command_page
            .command
            .as_ref()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let events = if command_any.type_url.ends_with("CreateShipment") {
            self.handle_create_shipment(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("MarkPicked") {
            self.handle_mark_picked(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("MarkPacked") {
            self.handle_mark_packed(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("Ship") {
            self.handle_ship(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("RecordDelivery") {
            self.handle_record_delivery(cb, &command_any.value, &state, next_seq)?
        } else {
            return Err(BusinessError::Rejected(format!(
                "{}: {}",
                errmsg::UNKNOWN_COMMAND,
                command_any.type_url
            )).into());
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
