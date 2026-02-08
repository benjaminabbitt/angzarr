//! Inventory bounded context client logic.
//!
//! Handles stock levels, reservations, and low stock alerts.

use prost::Message;

use angzarr::proto::{
    event_page::Sequence, BusinessResponse, CommandBook, ComponentDescriptor, ContextualCommand,
    Cover, EventBook, EventPage, Snapshot,
};
use common::proto::{
    CommitReservation, InitializeStock, InventoryState, LowStockAlert, ReceiveStock,
    ReleaseReservation, ReservationCommitted, ReservationReleased, ReserveStock, StockInitialized,
    StockReceived, StockReserved,
};
use common::{decode_command, make_event_book, now, ProtoTypeName};
use common::{require_exists, require_non_negative, require_not_exists, require_positive};
use common::{Aggregate, AggregateLogic, BusinessError, Result, StateBuilder};

pub mod errmsg {
    pub const ALREADY_INITIALIZED: &str = "Inventory already initialized";
    pub const NOT_INITIALIZED: &str = "Inventory not initialized";
    pub const QUANTITY_POSITIVE: &str = "Quantity must be positive";
    pub const INSUFFICIENT_STOCK: &str = "Insufficient available stock";
    pub const RESERVATION_NOT_FOUND: &str = "Reservation not found";
    pub use common::errmsg::*;
}

// ============================================================================
// Named event appliers
// ============================================================================

fn apply_stock_initialized(state: &mut InventoryState, event: &prost_types::Any) {
    if let Ok(e) = StockInitialized::decode(event.value.as_slice()) {
        state.product_id = e.product_id;
        state.on_hand = e.quantity;
        state.reserved = 0;
        state.low_stock_threshold = e.low_stock_threshold;
        state.reservations.clear();
    }
}

fn apply_stock_received(state: &mut InventoryState, event: &prost_types::Any) {
    if let Ok(e) = StockReceived::decode(event.value.as_slice()) {
        state.on_hand = e.new_on_hand;
    }
}

fn apply_stock_reserved(state: &mut InventoryState, event: &prost_types::Any) {
    if let Ok(e) = StockReserved::decode(event.value.as_slice()) {
        // Use facts (absolute values) for idempotent state reconstruction
        state.on_hand = e.new_on_hand;
        state.reserved = e.new_reserved;
        state.reservations.insert(e.order_id, e.quantity);
    }
}

fn apply_reservation_released(state: &mut InventoryState, event: &prost_types::Any) {
    if let Ok(e) = ReservationReleased::decode(event.value.as_slice()) {
        // Use facts (absolute values) for idempotent state reconstruction
        state.on_hand = e.new_on_hand;
        state.reserved = e.new_reserved;
        state.reservations.remove(&e.order_id);
    }
}

fn apply_reservation_committed(state: &mut InventoryState, event: &prost_types::Any) {
    if let Ok(e) = ReservationCommitted::decode(event.value.as_slice()) {
        // Use facts (absolute values) for idempotent state reconstruction
        state.on_hand = e.new_on_hand;
        state.reserved = e.new_reserved;
        state.reservations.remove(&e.order_id);
    }
}

// ============================================================================
// State rebuilding
// ============================================================================

/// Create the StateBuilder with all registered event handlers.
fn state_builder() -> StateBuilder<InventoryState> {
    StateBuilder::new()
        .on(StockInitialized::TYPE_NAME, apply_stock_initialized)
        .on(StockReceived::TYPE_NAME, apply_stock_received)
        .on(StockReserved::TYPE_NAME, apply_stock_reserved)
        .on(ReservationReleased::TYPE_NAME, apply_reservation_released)
        .on(ReservationCommitted::TYPE_NAME, apply_reservation_committed)
}

fn rebuild_state(event_book: Option<&EventBook>) -> InventoryState {
    state_builder().rebuild(event_book)
}

/// Apply a single event to the inventory state.
pub fn apply_event(state: &mut InventoryState, event: &prost_types::Any) {
    state_builder().apply(state, event);
}

/// Apply an event and build an EventBook response with updated snapshot.
fn build_event_response(
    state: &InventoryState,
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
        &InventoryState::type_url(),
        new_state.encode_to_vec(),
    )
}

/// Client logic for Inventory aggregate.
pub struct InventoryLogic {
    aggregate: Aggregate<InventoryState>,
}

impl InventoryLogic {
    pub const DOMAIN: &'static str = "inventory";

    pub fn new() -> Self {
        Self {
            aggregate: Aggregate::new("inventory", rebuild_state)
                .on(InitializeStock::TYPE_NAME, handle_initialize_stock)
                .on(ReceiveStock::TYPE_NAME, handle_receive_stock)
                .on(ReserveStock::TYPE_NAME, handle_reserve_stock)
                .on(ReleaseReservation::TYPE_NAME, handle_release_reservation)
                .on(CommitReservation::TYPE_NAME, handle_commit_reservation),
        }
    }
}

impl Default for InventoryLogic {
    fn default() -> Self {
        Self::new()
    }
}

fn available(state: &InventoryState) -> i32 {
    state.on_hand - state.reserved
}

fn handle_initialize_stock(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &InventoryState,
    next_seq: u32,
) -> Result<EventBook> {
    require_not_exists(&state.product_id, errmsg::ALREADY_INITIALIZED)?;

    let cmd: InitializeStock = decode_command(command_data)?;

    require_non_negative(cmd.quantity, errmsg::QUANTITY_POSITIVE)?;

    let event = StockInitialized {
        product_id: cmd.product_id.clone(),
        quantity: cmd.quantity,
        low_stock_threshold: cmd.low_stock_threshold,
        initialized_at: Some(now()),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        &StockInitialized::type_url(),
        event,
    ))
}

fn handle_receive_stock(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &InventoryState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.product_id, errmsg::NOT_INITIALIZED)?;

    let cmd: ReceiveStock = decode_command(command_data)?;

    require_positive(cmd.quantity, errmsg::QUANTITY_POSITIVE)?;

    let new_on_hand = state.on_hand + cmd.quantity;

    let event = StockReceived {
        quantity: cmd.quantity,
        new_on_hand,
        reference: cmd.reference,
        received_at: Some(now()),
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        &StockReceived::type_url(),
        event,
    ))
}

fn handle_reserve_stock(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &InventoryState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.product_id, errmsg::NOT_INITIALIZED)?;

    let cmd: ReserveStock = decode_command(command_data)?;

    let avail = available(state);
    if cmd.quantity > avail {
        return Err(BusinessError::Rejected(format!(
            "{}: have {}, need {}",
            errmsg::INSUFFICIENT_STOCK, avail, cmd.quantity
        )));
    }

    let new_available = avail - cmd.quantity;
    let new_reserved = state.reserved + cmd.quantity;

    let event = StockReserved {
        quantity: cmd.quantity,
        order_id: cmd.order_id.clone(),
        new_available,
        reserved_at: Some(now()),
        new_reserved,
        new_on_hand: state.on_hand,
    };

    let event_bytes = event.encode_to_vec();
    let mut new_state = state.clone();
    apply_event(
        &mut new_state,
        &prost_types::Any {
            type_url: StockReserved::type_url(),
            value: event_bytes.clone(),
        },
    );

    let mut seq = next_seq;
    let mut pages = vec![EventPage {
        sequence: Some(Sequence::Num(seq)),
        event: Some(prost_types::Any {
            type_url: StockReserved::type_url(),
            value: event_bytes,
        }),
        created_at: Some(now()),
    }];
    seq += 1;

    if state.low_stock_threshold > 0 && new_available < state.low_stock_threshold {
        let alert = LowStockAlert {
            product_id: state.product_id.clone(),
            available: new_available,
            threshold: state.low_stock_threshold,
            alerted_at: Some(now()),
        };
        pages.push(EventPage {
            sequence: Some(Sequence::Num(seq)),
            event: Some(prost_types::Any {
                type_url: LowStockAlert::type_url(),
                value: alert.encode_to_vec(),
            }),
            created_at: Some(now()),
        });
    }

    Ok(EventBook {
        cover: command_book.cover.clone(),
        snapshot: Some(Snapshot {
            sequence: 0, // Framework computes from pages
            state: Some(prost_types::Any {
                type_url: InventoryState::type_url(),
                value: new_state.encode_to_vec(),
            }),
        }),
        pages,
    })
}

fn handle_release_reservation(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &InventoryState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.product_id, errmsg::NOT_INITIALIZED)?;

    let cmd: ReleaseReservation = decode_command(command_data)?;

    let quantity = state
        .reservations
        .get(&cmd.order_id)
        .copied()
        .ok_or_else(|| BusinessError::Rejected(errmsg::RESERVATION_NOT_FOUND.to_string()))?;

    let new_available = available(state) + quantity;
    let new_reserved = state.reserved - quantity;

    let event = ReservationReleased {
        order_id: cmd.order_id.clone(),
        quantity,
        new_available,
        released_at: Some(now()),
        new_reserved,
        new_on_hand: state.on_hand,
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        &ReservationReleased::type_url(),
        event,
    ))
}

fn handle_commit_reservation(
    command_book: &CommandBook,
    command_data: &[u8],
    state: &InventoryState,
    next_seq: u32,
) -> Result<EventBook> {
    require_exists(&state.product_id, errmsg::NOT_INITIALIZED)?;

    let cmd: CommitReservation = decode_command(command_data)?;

    let quantity = state
        .reservations
        .get(&cmd.order_id)
        .copied()
        .ok_or_else(|| BusinessError::Rejected(errmsg::RESERVATION_NOT_FOUND.to_string()))?;

    let new_on_hand = state.on_hand - quantity;
    let new_reserved = state.reserved - quantity;

    let event = ReservationCommitted {
        order_id: cmd.order_id.clone(),
        quantity,
        new_on_hand,
        committed_at: Some(now()),
        new_reserved,
    };

    Ok(build_event_response(
        state,
        command_book.cover.clone(),
        next_seq,
        &ReservationCommitted::type_url(),
        event,
    ))
}

#[tonic::async_trait]
impl AggregateLogic for InventoryLogic {
    fn descriptor(&self) -> ComponentDescriptor {
        self.aggregate.descriptor()
    }

    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        self.aggregate.dispatch(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{event_page::Sequence, Cover, EventPage, Uuid as ProtoUuid};
    use common::testing::{extract_response_events, make_test_command_book};

    #[tokio::test]
    async fn test_initialize_stock_success() {
        let logic = InventoryLogic::new();

        let cmd = InitializeStock {
            product_id: "SKU-001".to_string(),
            quantity: 100,
            low_stock_threshold: 10,
        };

        let command_book = make_test_command_book(
            "inventory",
            &[1; 16],
            "type.examples/examples.InitializeStock",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let response = logic.handle(ctx).await.unwrap();
        let result = extract_response_events(response);
        assert_eq!(result.pages.len(), 1);

        // Verify explicit sequence assignment
        assert_eq!(result.pages[0].sequence, Some(Sequence::Num(0)));

        let event =
            StockInitialized::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
                .unwrap();
        assert_eq!(event.product_id, "SKU-001");
        assert_eq!(event.quantity, 100);
    }

    #[tokio::test]
    async fn test_reserve_stock_success() {
        let logic = InventoryLogic::new();

        let prior = EventBook {
            cover: Some(Cover {
                domain: "inventory".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.StockInitialized".to_string(),
                    value: StockInitialized {
                        product_id: "SKU-001".to_string(),
                        quantity: 100,
                        low_stock_threshold: 0,
                        initialized_at: None,
                    }
                    .encode_to_vec(),
                }),
                created_at: None,
            }],
        };

        let cmd = ReserveStock {
            quantity: 10,
            order_id: "ORD-001".to_string(),
        };

        let command_book = make_test_command_book(
            "inventory",
            &[1; 16],
            "type.examples/examples.ReserveStock",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let response = logic.handle(ctx).await.unwrap();
        let result = extract_response_events(response);
        assert_eq!(result.pages.len(), 1);

        // Verify explicit sequence: prior event was seq 0, so new event is seq 1
        assert_eq!(result.pages[0].sequence, Some(Sequence::Num(1)));

        let event = StockReserved::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
            .unwrap();
        assert_eq!(event.quantity, 10);
        assert_eq!(event.new_available, 90);
    }
}
