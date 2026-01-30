//! Generic state rebuild from event-sourced event books.
//!
//! Eliminates the repeated snapshot-loading + event-iteration boilerplate
//! found in every aggregate's `rebuild_state` function.

use angzarr::proto::EventBook;
use prost::Message;

/// Rebuild aggregate state from an EventBook using a generic apply function.
///
/// Handles the common pattern:
/// 1. Start from default state
/// 2. Load snapshot if present
/// 3. Apply each event page via the provided closure
///
/// Each aggregate provides its own `apply_event` implementation.
pub fn rebuild_from_events<S: Message + Default>(
    event_book: Option<&EventBook>,
    mut apply: impl FnMut(&mut S, &prost_types::Any),
) -> S {
    let mut state = S::default();

    let Some(book) = event_book else {
        return state;
    };

    // Start from snapshot if present
    if let Some(snapshot) = &book.snapshot {
        if let Some(snapshot_state) = &snapshot.state {
            if let Ok(s) = S::decode(snapshot_state.value.as_slice()) {
                state = s;
            }
        }
    }

    // Apply events
    for page in &book.pages {
        let Some(event) = &page.event else {
            continue;
        };

        apply(&mut state, event);
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr::proto::{event_page::Sequence, Cover, EventPage, Snapshot, Uuid as ProtoUuid};

    /// Minimal protobuf state for testing.
    #[derive(Clone, PartialEq, prost::Message)]
    struct TestState {
        #[prost(string, tag = "1")]
        pub name: String,
        #[prost(int32, tag = "2")]
        pub count: i32,
    }

    /// Minimal protobuf event for testing.
    #[derive(Clone, PartialEq, prost::Message)]
    struct CountIncremented {
        #[prost(int32, tag = "1")]
        pub new_count: i32,
    }

    fn apply_test_event(state: &mut TestState, event: &prost_types::Any) {
        if event.type_url.ends_with("CountIncremented") {
            if let Ok(e) = CountIncremented::decode(event.value.as_slice()) {
                state.count = e.new_count;
            }
        }
    }

    #[test]
    fn test_rebuild_from_none_returns_default() {
        let state: TestState = rebuild_from_events(None, apply_test_event);
        assert!(state.name.is_empty());
        assert_eq!(state.count, 0);
    }

    #[test]
    fn test_rebuild_from_events_applies_events() {
        let event = CountIncremented { new_count: 5 };
        let book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.test/test.CountIncremented".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot_state: None,
        };

        let state: TestState = rebuild_from_events(Some(&book), apply_test_event);
        assert_eq!(state.count, 5);
    }

    #[test]
    fn test_rebuild_from_snapshot_plus_events() {
        let snapshot_state = TestState {
            name: "snapshotted".to_string(),
            count: 10,
        };

        let event = CountIncremented { new_count: 15 };
        let book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            snapshot: Some(Snapshot {
                sequence: 5,
                state: Some(prost_types::Any {
                    type_url: "type.test/test.TestState".to_string(),
                    value: snapshot_state.encode_to_vec(),
                }),
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(6)),
                event: Some(prost_types::Any {
                    type_url: "type.test/test.CountIncremented".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot_state: None,
        };

        let state: TestState = rebuild_from_events(Some(&book), apply_test_event);
        assert_eq!(state.name, "snapshotted");
        assert_eq!(state.count, 15);
    }

    #[test]
    fn test_rebuild_empty_event_book() {
        let book = EventBook {
            cover: None,
            snapshot: None,
            pages: vec![],
            snapshot_state: None,
        };

        let state: TestState = rebuild_from_events(Some(&book), apply_test_event);
        assert!(state.name.is_empty());
        assert_eq!(state.count, 0);
    }
}
