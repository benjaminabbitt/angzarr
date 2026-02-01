//! Inventory bounded context client logic.
//!
//! Handles stock levels, reservations, and low stock alerts.

use prost::Message;

use angzarr::proto::{
    event_page::Sequence, BusinessResponse, CommandBook, ContextualCommand, Cover, EventBook,
    EventPage,
};
use common::proto::{
    CommitReservation, InitializeStock, InventoryState, LowStockAlert, ReceiveStock,
    ReleaseReservation, ReservationCommitted, ReservationReleased, ReserveStock, StockInitialized,
    StockReceived, StockReserved,
};
use common::{decode_command, dispatch_aggregate, make_event_book, now, unknown_command};
use common::{
    rebuild_from_events, require_exists, require_non_negative, require_not_exists, require_positive,
};
use common::{AggregateLogic, BusinessError, Result};

pub mod errmsg {
    pub const ALREADY_INITIALIZED: &str = "Inventory already initialized";
    pub const NOT_INITIALIZED: &str = "Inventory not initialized";
    pub const QUANTITY_POSITIVE: &str = "Quantity must be positive";
    pub const INSUFFICIENT_STOCK: &str = "Insufficient available stock";
    pub const RESERVATION_NOT_FOUND: &str = "Reservation not found";
    pub use common::errmsg::*;
}

const STATE_TYPE_URL: &str = "type.examples/examples.InventoryState";

/// Apply a single event to the inventory state.
///
/// Single source of truth for all inventory state transitions.
pub fn apply_event(state: &mut InventoryState, event: &prost_types::Any) {
    if event.type_url.ends_with("StockInitialized") {
        if let Ok(e) = StockInitialized::decode(event.value.as_slice()) {
            state.product_id = e.product_id;
            state.on_hand = e.quantity;
            state.reserved = 0;
            state.low_stock_threshold = e.low_stock_threshold;
            state.reservations.clear();
        }
    } else if event.type_url.ends_with("StockReceived") {
        if let Ok(e) = StockReceived::decode(event.value.as_slice()) {
            state.on_hand = e.new_on_hand;
        }
    } else if event.type_url.ends_with("StockReserved") {
        if let Ok(e) = StockReserved::decode(event.value.as_slice()) {
            // Use facts (absolute values) for idempotent state reconstruction
            state.on_hand = e.new_on_hand;
            state.reserved = e.new_reserved;
            state.reservations.insert(e.order_id, e.quantity);
        }
    } else if event.type_url.ends_with("ReservationReleased") {
        if let Ok(e) = ReservationReleased::decode(event.value.as_slice()) {
            // Use facts (absolute values) for idempotent state reconstruction
            state.on_hand = e.new_on_hand;
            state.reserved = e.new_reserved;
            state.reservations.remove(&e.order_id);
        }
    } else if event.type_url.ends_with("ReservationCommitted") {
        if let Ok(e) = ReservationCommitted::decode(event.value.as_slice()) {
            // Use facts (absolute values) for idempotent state reconstruction
            state.on_hand = e.new_on_hand;
            state.reserved = e.new_reserved;
            state.reservations.remove(&e.order_id);
        }
    }
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
        STATE_TYPE_URL,
        new_state.encode_to_vec(),
    )
}

/// client logic for Inventory aggregate.
pub struct InventoryLogic;

common::define_aggregate!(InventoryLogic, "inventory");

common::expose_handlers!(methods, InventoryLogic, InventoryState, rebuild: rebuild_state, [
    (handle_initialize_stock_public, handle_initialize_stock),
    (handle_receive_stock_public, handle_receive_stock),
    (handle_reserve_stock_public, handle_reserve_stock),
    (handle_release_reservation_public, handle_release_reservation),
    (handle_commit_reservation_public, handle_commit_reservation),
]);

impl InventoryLogic {
    /// Rebuild inventory state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> InventoryState {
        rebuild_from_events(event_book, apply_event)
    }

    fn available(&self, state: &InventoryState) -> i32 {
        state.on_hand - state.reserved
    }

    fn handle_initialize_stock(
        &self,
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
            "type.examples/examples.StockInitialized",
            event,
        ))
    }

    fn handle_receive_stock(
        &self,
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
            "type.examples/examples.StockReceived",
            event,
        ))
    }

    fn handle_reserve_stock(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        require_exists(&state.product_id, errmsg::NOT_INITIALIZED)?;

        let cmd: ReserveStock = decode_command(command_data)?;

        let available = self.available(state);
        if cmd.quantity > available {
            return Err(BusinessError::Rejected(format!(
                "{}: have {}, need {}",
                errmsg::INSUFFICIENT_STOCK,
                available,
                cmd.quantity
            )));
        }

        let new_available = available - cmd.quantity;
        let new_reserved = state.reserved + cmd.quantity;

        let event = StockReserved {
            quantity: cmd.quantity,
            order_id: cmd.order_id.clone(),
            new_available,
            reserved_at: Some(now()),
            new_reserved,               // Fact: total reserved after this event
            new_on_hand: state.on_hand, // Fact: on_hand unchanged by reserve
        };

        // Derive state through apply_event (single source of truth)
        let event_bytes = event.encode_to_vec();
        let mut new_state = state.clone();
        apply_event(
            &mut new_state,
            &prost_types::Any {
                type_url: "type.examples/examples.StockReserved".to_string(),
                value: event_bytes.clone(),
            },
        );

        // Build event pages - main event plus optional alert
        let mut seq = next_seq;
        let mut pages = vec![EventPage {
            sequence: Some(Sequence::Num(seq)),
            event: Some(prost_types::Any {
                type_url: "type.examples/examples.StockReserved".to_string(),
                value: event_bytes,
            }),
            created_at: Some(now()),
        }];
        seq += 1;

        // Check for low stock alert
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
                    type_url: "type.examples/examples.LowStockAlert".to_string(),
                    value: alert.encode_to_vec(),
                }),
                created_at: Some(now()),
            });
        }

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages,
            snapshot_state: Some(prost_types::Any {
                type_url: STATE_TYPE_URL.to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_release_reservation(
        &self,
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

        let new_available = self.available(state) + quantity;
        let new_reserved = state.reserved - quantity;

        let event = ReservationReleased {
            order_id: cmd.order_id.clone(),
            quantity,
            new_available,
            released_at: Some(now()),
            new_reserved,               // Fact: total reserved after this event
            new_on_hand: state.on_hand, // Fact: on_hand unchanged by release
        };

        Ok(build_event_response(
            state,
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.ReservationReleased",
            event,
        ))
    }

    fn handle_commit_reservation(
        &self,
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
            new_reserved, // Fact: total reserved after this event
        };

        Ok(build_event_response(
            state,
            command_book.cover.clone(),
            next_seq,
            "type.examples/examples.ReservationCommitted",
            event,
        ))
    }
}

#[tonic::async_trait]
impl AggregateLogic for InventoryLogic {
    async fn handle(
        &self,
        cmd: ContextualCommand,
    ) -> std::result::Result<BusinessResponse, tonic::Status> {
        dispatch_aggregate(
            cmd,
            |eb| self.rebuild_state(eb),
            |cb, command_any, state, next_seq| {
                if command_any.type_url.ends_with("InitializeStock") {
                    self.handle_initialize_stock(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("ReceiveStock") {
                    self.handle_receive_stock(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("ReserveStock") {
                    self.handle_reserve_stock(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("ReleaseReservation") {
                    self.handle_release_reservation(cb, &command_any.value, state, next_seq)
                } else if command_any.type_url.ends_with("CommitReservation") {
                    self.handle_commit_reservation(cb, &command_any.value, state, next_seq)
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
            snapshot_state: None,
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
