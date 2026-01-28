//! Fulfillment bounded context business logic.
//!
//! Manages shipment lifecycle: pending -> picking -> packing -> shipped -> delivered.

use prost::Message;

use angzarr::proto::{BusinessResponse, CommandBook, ContextualCommand, EventBook};
use common::proto::{
    CreateShipment, Delivered, FulfillmentState, ItemsPacked, ItemsPicked, MarkPacked, MarkPicked,
    RecordDelivery, Ship, ShipmentCreated, Shipped,
};
use common::{decode_command, dispatch_aggregate, make_event_book, now, unknown_command};
use common::{
    rebuild_from_events, require_exists, require_not_exists, require_status, require_status_not,
};
use common::{AggregateLogic, Result};

pub mod errmsg {
    pub const SHIPMENT_EXISTS: &str = "Shipment already exists";
    pub const SHIPMENT_NOT_FOUND: &str = "Shipment does not exist";
    pub const NOT_PENDING: &str = "Shipment is not pending";
    pub const NOT_PICKED: &str = "Shipment is not picked";
    pub const NOT_PACKED: &str = "Shipment is not packed";
    pub const NOT_SHIPPED: &str = "Shipment is not shipped";
    pub const ALREADY_DELIVERED: &str = "Shipment is already delivered";
    pub use common::errmsg::*;
}

fn apply_event(state: &mut FulfillmentState, event: &prost_types::Any) {
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

/// Business logic for Fulfillment aggregate.
pub struct FulfillmentLogic;

common::define_aggregate!(FulfillmentLogic, "fulfillment");

common::expose_handlers!(methods, FulfillmentLogic, FulfillmentState, rebuild: rebuild_state, [
    (handle_create_shipment_public, handle_create_shipment),
    (handle_mark_picked_public, handle_mark_picked),
    (handle_mark_packed_public, handle_mark_packed),
    (handle_ship_public, handle_ship),
    (handle_record_delivery_public, handle_record_delivery),
]);

impl FulfillmentLogic {
    /// Rebuild fulfillment state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> FulfillmentState {
        rebuild_from_events(event_book, apply_event)
    }

    fn handle_create_shipment(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_not_exists(&state.order_id, errmsg::SHIPMENT_EXISTS)?;

        let cmd: CreateShipment = decode_command(command_data)?;

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

        Ok(make_event_book(
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.ShipmentCreated",
            event.encode_to_vec(),
            "type.examples/examples.FulfillmentState",
            new_state.encode_to_vec(),
        ))
    }

    fn handle_mark_picked(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_exists(&state.order_id, errmsg::SHIPMENT_NOT_FOUND)?;
        require_status(&state.status, "pending", errmsg::NOT_PENDING)?;

        let cmd: MarkPicked = decode_command(command_data)?;

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

        Ok(make_event_book(
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.ItemsPicked",
            event.encode_to_vec(),
            "type.examples/examples.FulfillmentState",
            new_state.encode_to_vec(),
        ))
    }

    fn handle_mark_packed(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_exists(&state.order_id, errmsg::SHIPMENT_NOT_FOUND)?;
        require_status(&state.status, "picking", errmsg::NOT_PICKED)?;

        let cmd: MarkPacked = decode_command(command_data)?;

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

        Ok(make_event_book(
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.ItemsPacked",
            event.encode_to_vec(),
            "type.examples/examples.FulfillmentState",
            new_state.encode_to_vec(),
        ))
    }

    fn handle_ship(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_exists(&state.order_id, errmsg::SHIPMENT_NOT_FOUND)?;
        require_status(&state.status, "packing", errmsg::NOT_PACKED)?;

        let cmd: Ship = decode_command(command_data)?;

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

        Ok(make_event_book(
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.Shipped",
            event.encode_to_vec(),
            "type.examples/examples.FulfillmentState",
            new_state.encode_to_vec(),
        ))
    }

    fn handle_record_delivery(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &FulfillmentState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_exists(&state.order_id, errmsg::SHIPMENT_NOT_FOUND)?;
        require_status_not(&state.status, "delivered", errmsg::ALREADY_DELIVERED)?;
        require_status(&state.status, "shipped", errmsg::NOT_SHIPPED)?;

        let cmd: RecordDelivery = decode_command(command_data)?;

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

        Ok(make_event_book(
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.Delivered",
            event.encode_to_vec(),
            "type.examples/examples.FulfillmentState",
            new_state.encode_to_vec(),
        ))
    }
}

#[tonic::async_trait]
impl AggregateLogic for FulfillmentLogic {
    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        dispatch_aggregate(
            cmd,
            |eb| self.rebuild_state(eb),
            |cb, command_any, state, next_seq| {
                if command_any.type_url.ends_with("CreateShipment") {
                    self.handle_create_shipment(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("MarkPicked") {
                    self.handle_mark_picked(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("MarkPacked") {
                    self.handle_mark_packed(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("Ship") {
                    self.handle_ship(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("RecordDelivery") {
                    self.handle_record_delivery(cb, &command_any.value, state, next_seq)
                } else {
                    Err(unknown_command(&command_any.type_url))
                }
            },
        )
    }
}
