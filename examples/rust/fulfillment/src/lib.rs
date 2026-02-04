//! Fulfillment bounded context client logic.
//!
//! Manages shipment lifecycle: pending -> picking -> packing -> shipped -> delivered.

use prost::Message;

use angzarr::proto::{
    BusinessResponse, CommandBook, ComponentDescriptor, ContextualCommand, Cover, EventBook,
};
use common::proto::{
    CreateShipment, Delivered, FulfillmentState, ItemsPacked, ItemsPicked, MarkPacked, MarkPicked,
    RecordDelivery, Ship, ShipmentCreated, Shipped,
};
use common::{decode_command, make_event_book, now};
use common::{
    rebuild_from_events, require_exists, require_not_exists, require_status, require_status_not,
};
use common::{AggregateLogic, CommandRouter, Result};

const STATE_TYPE_URL: &str = "type.examples/examples.FulfillmentState";

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

pub fn apply_event(state: &mut FulfillmentState, event: &prost_types::Any) {
    if event.type_url.ends_with("ShipmentCreated") {
        if let Ok(e) = ShipmentCreated::decode(event.value.as_slice()) {
            state.order_id = e.order_id;
            state.status = "pending".to_string();
            state.items = e.items;
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

/// Apply an event and build an EventBook response with updated snapshot.
fn build_event_response(
    state: &FulfillmentState,
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

/// Client logic for Fulfillment aggregate.
pub struct FulfillmentLogic {
    router: CommandRouter<FulfillmentState>,
}

impl FulfillmentLogic {
    pub const DOMAIN: &'static str = "fulfillment";

    pub fn new() -> Self {
        Self {
            router: CommandRouter::new("fulfillment", rebuild_state)
                .on("CreateShipment", handle_create_shipment)
                .on("MarkPicked", handle_mark_picked)
                .on("MarkPacked", handle_mark_packed)
                .on("Ship", handle_ship)
                .on("RecordDelivery", handle_record_delivery),
        }
    }
}

impl Default for FulfillmentLogic {
    fn default() -> Self {
        Self::new()
    }
}

fn rebuild_state(event_book: Option<&EventBook>) -> FulfillmentState {
    rebuild_from_events(event_book, apply_event)
}

fn handle_create_shipment(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &FulfillmentState,
    next_seq: u32,
) -> Result<EventBook> {
    require_not_exists(&state.order_id, errmsg::SHIPMENT_EXISTS)?;

    let cmd: CreateShipment = decode_command(command_data)?;

    let event = ShipmentCreated {
        order_id: cmd.order_id,
        status: "pending".to_string(),
        created_at: Some(now()),
        items: cmd.items,
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.ShipmentCreated",
        event,
    ))
}

fn handle_mark_picked(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &FulfillmentState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.order_id, errmsg::SHIPMENT_NOT_FOUND)?;
    require_status(&state.status, "pending", errmsg::NOT_PENDING)?;

    let cmd: MarkPicked = decode_command(command_data)?;

    let event = ItemsPicked {
        picker_id: cmd.picker_id,
        picked_at: Some(now()),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.ItemsPicked",
        event,
    ))
}

fn handle_mark_packed(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &FulfillmentState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.order_id, errmsg::SHIPMENT_NOT_FOUND)?;
    require_status(&state.status, "picking", errmsg::NOT_PICKED)?;

    let cmd: MarkPacked = decode_command(command_data)?;

    let event = ItemsPacked {
        packer_id: cmd.packer_id,
        packed_at: Some(now()),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.ItemsPacked",
        event,
    ))
}

fn handle_ship(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &FulfillmentState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.order_id, errmsg::SHIPMENT_NOT_FOUND)?;
    require_status(&state.status, "packing", errmsg::NOT_PACKED)?;

    let cmd: Ship = decode_command(command_data)?;

    let event = Shipped {
        carrier: cmd.carrier,
        tracking_number: cmd.tracking_number,
        shipped_at: Some(now()),
        items: state.items.clone(),
        order_id: state.order_id.clone(),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.Shipped",
        event,
    ))
}

fn handle_record_delivery(
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
        signature: cmd.signature,
        delivered_at: Some(now()),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        "type.examples/examples.Delivered",
        event,
    ))
}

#[tonic::async_trait]
impl AggregateLogic for FulfillmentLogic {
    fn descriptor(&self) -> ComponentDescriptor {
        self.router.descriptor()
    }

    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        self.router.dispatch(cmd)
    }
}
