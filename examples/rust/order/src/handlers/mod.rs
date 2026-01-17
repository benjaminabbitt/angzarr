//! Command handlers for the Order aggregate.

mod apply_loyalty_discount;
mod cancel_order;
mod confirm_payment;
mod create_order;
mod submit_payment;

pub use apply_loyalty_discount::handle_apply_loyalty_discount;
pub use cancel_order::handle_cancel_order;
pub use confirm_payment::handle_confirm_payment;
pub use create_order::handle_create_order;
pub use submit_payment::handle_submit_payment;

use angzarr::proto::EventBook;

/// Helper to get the current timestamp.
pub fn now() -> prost_types::Timestamp {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    prost_types::Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}

/// Helper to create a single-page EventBook with snapshot state.
pub fn make_event_book(
    cover: Option<angzarr::proto::Cover>,
    sequence: u32,
    type_url: &str,
    event_value: Vec<u8>,
    state_value: Vec<u8>,
) -> EventBook {
    use angzarr::proto::{event_page::Sequence, EventPage};

    EventBook {
        cover,
        snapshot: None,
        pages: vec![EventPage {
            sequence: Some(Sequence::Num(sequence)),
            event: Some(prost_types::Any {
                type_url: type_url.to_string(),
                value: event_value,
            }),
            created_at: Some(now()),
            synchronous: false,
        }],
        correlation_id: String::new(),
        snapshot_state: Some(prost_types::Any {
            type_url: "type.examples/examples.OrderState".to_string(),
            value: state_value,
        }),
    }
}
