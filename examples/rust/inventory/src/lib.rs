//! Inventory bounded context business logic.
//!
//! Handles stock levels, reservations, and low stock alerts.

use std::collections::HashMap;

use async_trait::async_trait;
use prost::Message;

use angzarr::clients::{BusinessError, BusinessLogicClient, Result};
use angzarr::proto::{
    business_response, event_page::Sequence, BusinessResponse, CommandBook, ContextualCommand,
    EventBook, EventPage,
};
use common::next_sequence;
use common::proto::{
    CommitReservation, InitializeStock, InventoryState, LowStockAlert, ReceiveStock,
    ReleaseReservation, ReservationCommitted, ReservationReleased, ReserveStock, StockInitialized,
    StockReceived, StockReserved,
};

pub mod errmsg {
    pub const ALREADY_INITIALIZED: &str = "Inventory already initialized";
    pub const NOT_INITIALIZED: &str = "Inventory not initialized";
    pub const QUANTITY_POSITIVE: &str = "Quantity must be positive";
    pub const INSUFFICIENT_STOCK: &str = "Insufficient available stock";
    pub const RESERVATION_NOT_FOUND: &str = "Reservation not found";
    pub const UNKNOWN_COMMAND: &str = "Unknown command type";
    pub const NO_COMMAND_PAGES: &str = "CommandBook has no pages";
}

/// Business logic for Inventory aggregate.
pub struct InventoryLogic {
    domain: String,
}

impl InventoryLogic {
    pub const DOMAIN: &'static str = "inventory";

    pub fn new() -> Self {
        Self {
            domain: Self::DOMAIN.to_string(),
        }
    }

    /// Rebuild inventory state from events.
    fn rebuild_state(&self, event_book: Option<&EventBook>) -> InventoryState {
        let mut state = InventoryState::default();

        let Some(book) = event_book else {
            return state;
        };

        // Start from snapshot if present
        if let Some(snapshot) = &book.snapshot {
            if let Some(snapshot_state) = &snapshot.state {
                if let Ok(s) = InventoryState::decode(snapshot_state.value.as_slice()) {
                    state = s;
                }
            }
        }

        // Apply events
        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

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

        state
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
        if !state.product_id.is_empty() {
            return Err(BusinessError::Rejected(
                errmsg::ALREADY_INITIALIZED.to_string(),
            ));
        }

        let cmd = InitializeStock::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.quantity < 0 {
            return Err(BusinessError::Rejected(
                errmsg::QUANTITY_POSITIVE.to_string(),
            ));
        }

        let event = StockInitialized {
            product_id: cmd.product_id.clone(),
            quantity: cmd.quantity,
            low_stock_threshold: cmd.low_stock_threshold,
            initialized_at: Some(now()),
        };

        let new_state = InventoryState {
            product_id: cmd.product_id,
            on_hand: cmd.quantity,
            reserved: 0,
            low_stock_threshold: cmd.low_stock_threshold,
            reservations: HashMap::new(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.StockInitialized".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.InventoryState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_receive_stock(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.product_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::NOT_INITIALIZED.to_string()));
        }

        let cmd = ReceiveStock::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

        if cmd.quantity <= 0 {
            return Err(BusinessError::Rejected(
                errmsg::QUANTITY_POSITIVE.to_string(),
            ));
        }

        let new_on_hand = state.on_hand + cmd.quantity;

        let event = StockReceived {
            quantity: cmd.quantity,
            new_on_hand,
            reference: cmd.reference,
            received_at: Some(now()),
        };

        let new_state = InventoryState {
            product_id: state.product_id.clone(),
            on_hand: new_on_hand,
            reserved: state.reserved,
            low_stock_threshold: state.low_stock_threshold,
            reservations: state.reservations.clone(),
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.StockReceived".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.InventoryState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_reserve_stock(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.product_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::NOT_INITIALIZED.to_string()));
        }

        let cmd = ReserveStock::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

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
            new_reserved,           // Fact: total reserved after this event
            new_on_hand: state.on_hand, // Fact: on_hand unchanged by reserve
        };

        let mut new_reservations = state.reservations.clone();
        new_reservations.insert(cmd.order_id, cmd.quantity);

        let new_state = InventoryState {
            product_id: state.product_id.clone(),
            on_hand: state.on_hand,
            reserved: new_reserved,
            low_stock_threshold: state.low_stock_threshold,
            reservations: new_reservations,
        };

        // Build event pages - main event plus optional alert
        let mut seq = next_seq;
        let mut pages = vec![EventPage {
            sequence: Some(Sequence::Num(seq)),
            event: Some(prost_types::Any {
                type_url: "type.examples/examples.StockReserved".to_string(),
                value: event.encode_to_vec(),
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
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.InventoryState".to_string(),
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
        if state.product_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::NOT_INITIALIZED.to_string()));
        }

        let cmd = ReleaseReservation::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

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
            new_reserved,           // Fact: total reserved after this event
            new_on_hand: state.on_hand, // Fact: on_hand unchanged by release
        };

        let mut new_reservations = state.reservations.clone();
        new_reservations.remove(&cmd.order_id);

        let new_state = InventoryState {
            product_id: state.product_id.clone(),
            on_hand: state.on_hand,
            reserved: new_reserved,
            low_stock_threshold: state.low_stock_threshold,
            reservations: new_reservations,
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ReservationReleased".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.InventoryState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }

    fn handle_commit_reservation(
        &self,
        command_book: &CommandBook,
        command_data: &[u8],
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        if state.product_id.is_empty() {
            return Err(BusinessError::Rejected(errmsg::NOT_INITIALIZED.to_string()));
        }

        let cmd = CommitReservation::decode(command_data)
            .map_err(|e| BusinessError::Rejected(e.to_string()))?;

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
            new_reserved,           // Fact: total reserved after this event
        };

        let mut new_reservations = state.reservations.clone();
        new_reservations.remove(&cmd.order_id);

        let new_state = InventoryState {
            product_id: state.product_id.clone(),
            on_hand: new_on_hand,
            reserved: new_reserved,
            low_stock_threshold: state.low_stock_threshold,
            reservations: new_reservations,
        };

        Ok(EventBook {
            cover: command_book.cover.clone(),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                event: Some(prost_types::Any {
                    type_url: "type.examples/examples.ReservationCommitted".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: Some(now()),
            }],
            correlation_id: String::new(),
            snapshot_state: Some(prost_types::Any {
                type_url: "type.examples/examples.InventoryState".to_string(),
                value: new_state.encode_to_vec(),
            }),
        })
    }
}

impl Default for InventoryLogic {
    fn default() -> Self {
        Self::new()
    }
}

// Public test methods for cucumber tests
impl InventoryLogic {
    pub fn rebuild_state_public(&self, event_book: Option<&EventBook>) -> InventoryState {
        self.rebuild_state(event_book)
    }

    pub fn handle_initialize_stock_public(
        &self,
        command_book: &CommandBook,
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_initialize_stock(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_receive_stock_public(
        &self,
        command_book: &CommandBook,
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_receive_stock(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_reserve_stock_public(
        &self,
        command_book: &CommandBook,
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_reserve_stock(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_release_reservation_public(
        &self,
        command_book: &CommandBook,
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_release_reservation(command_book, &command_any.value, state, next_seq)
    }

    pub fn handle_commit_reservation_public(
        &self,
        command_book: &CommandBook,
        state: &InventoryState,
        next_seq: u32,
    ) -> Result<EventBook> {
        let command_any = command_book
            .pages
            .first()
            .and_then(|p| p.command.as_ref())
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;
        self.handle_commit_reservation(command_book, &command_any.value, state, next_seq)
    }
}

#[async_trait]
impl BusinessLogicClient for InventoryLogic {
    async fn handle(&self, _domain: &str, cmd: ContextualCommand) -> Result<BusinessResponse> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        let state = self.rebuild_state(prior_events);
        let next_seq = next_sequence(prior_events);

        let Some(cb) = command_book else {
            return Err(BusinessError::Rejected(
                errmsg::NO_COMMAND_PAGES.to_string(),
            ));
        };

        let command_page = cb
            .pages
            .first()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let command_any = command_page
            .command
            .as_ref()
            .ok_or_else(|| BusinessError::Rejected(errmsg::NO_COMMAND_PAGES.to_string()))?;

        let events = if command_any.type_url.ends_with("InitializeStock") {
            self.handle_initialize_stock(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ReceiveStock") {
            self.handle_receive_stock(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ReserveStock") {
            self.handle_reserve_stock(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("ReleaseReservation") {
            self.handle_release_reservation(cb, &command_any.value, &state, next_seq)?
        } else if command_any.type_url.ends_with("CommitReservation") {
            self.handle_commit_reservation(cb, &command_any.value, &state, next_seq)?
        } else {
            return Err(BusinessError::Rejected(format!(
                "{}: {}",
                errmsg::UNKNOWN_COMMAND,
                command_any.type_url
            )));
        };

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(events)),
        })
    }

    fn has_domain(&self, domain: &str) -> bool {
        domain == self.domain
    }

    fn domains(&self) -> Vec<String> {
        vec![self.domain.clone()]
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
            auto_resequence: false,
            fact: false,
        }
    }

    fn extract_events(response: BusinessResponse) -> EventBook {
        match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events in response"),
        }
    }

    #[tokio::test]
    async fn test_initialize_stock_success() {
        let logic = InventoryLogic::new();

        let cmd = InitializeStock {
            product_id: "SKU-001".to_string(),
            quantity: 100,
            low_stock_threshold: 10,
        };

        let command_book = make_command_book(
            "inventory",
            &[1; 16],
            "type.examples/examples.InitializeStock",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: None,
        };

        let response = logic.handle("inventory", ctx).await.unwrap();
        let result = extract_events(response);
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
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let cmd = ReserveStock {
            quantity: 10,
            order_id: "ORD-001".to_string(),
        };

        let command_book = make_command_book(
            "inventory",
            &[1; 16],
            "type.examples/examples.ReserveStock",
            cmd.encode_to_vec(),
        );

        let ctx = ContextualCommand {
            command: Some(command_book),
            events: Some(prior),
        };

        let response = logic.handle("inventory", ctx).await.unwrap();
        let result = extract_events(response);
        assert_eq!(result.pages.len(), 1);

        // Verify explicit sequence: prior event was seq 0, so new event is seq 1
        assert_eq!(result.pages[0].sequence, Some(Sequence::Num(1)));

        let event = StockReserved::decode(result.pages[0].event.as_ref().unwrap().value.as_slice())
            .unwrap();
        assert_eq!(event.quantity, 10);
        assert_eq!(event.new_available, 90);
    }
}
