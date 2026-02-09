//! Generic state rebuild from event-sourced event books.
//!
//! Provides two approaches:
//! - `rebuild_from_events`: Takes a closure for event application
//! - `StateBuilder`: Declarative registration of named event handlers
//!
//! `StateBuilder` is the preferred approach as it mirrors `Aggregate`'s
//! pattern of registering handlers by type suffix.

use angzarr::proto::EventBook;
use prost::Message;

// ============================================================================
// StateBuilder: Declarative event handler registration
// ============================================================================

/// Function pointer that applies a raw event (Any) to state.
///
/// Each handler is responsible for decoding the event and type-checking.
/// This matches the CommandHandler pattern where handlers decode commands.
pub type StateApplier<S> = fn(&mut S, &prost_types::Any);

/// Builder for state reconstruction from events.
///
/// Registers event handlers via function pointers (no closures).
/// Each handler receives the raw `Any` and handles its own decoding.
///
/// # Example
///
/// ```ignore
/// use once_cell::sync::Lazy;
///
/// static STATE_BUILDER: Lazy<StateBuilder<OrderState>> = Lazy::new(|| {
///     StateBuilder::new()
///         .on("OrderCreated", apply_order_created)
///         .on("OrderCompleted", apply_order_completed)
/// });
///
/// fn apply_order_created(state: &mut OrderState, event: &prost_types::Any) {
///     if let Ok(e) = OrderCreated::decode(event.value.as_slice()) {
///         state.customer_id = e.customer_id;
///         // ...
///     }
/// }
///
/// pub fn rebuild_state(book: Option<&EventBook>) -> OrderState {
///     STATE_BUILDER.rebuild(book)
/// }
/// ```
pub struct StateBuilder<S> {
    appliers: Vec<(&'static str, StateApplier<S>)>,
}

impl<S: Message + Default> StateBuilder<S> {
    /// Create a new StateBuilder with no registered handlers.
    pub fn new() -> Self {
        Self {
            appliers: Vec::new(),
        }
    }

    /// Register an event applier for a type_url suffix.
    ///
    /// The applier function is responsible for decoding the event.
    /// This matches Aggregate's pattern where handlers decode commands.
    pub fn on(mut self, type_suffix: &'static str, apply: StateApplier<S>) -> Self {
        self.appliers.push((type_suffix, apply));
        self
    }

    /// Apply a single event to state using registered handlers.
    ///
    /// Useful for applying newly-created events to current state
    /// without going through full EventBook reconstruction.
    pub fn apply(&self, state: &mut S, event: &prost_types::Any) {
        for (suffix, apply) in &self.appliers {
            if event.type_url.ends_with(suffix) {
                apply(state, event);
                break;
            }
        }
    }

    /// Rebuild state from an EventBook.
    ///
    /// Handles snapshots first, then applies events via registered handlers.
    /// Unknown event types are silently ignored.
    pub fn rebuild(&self, event_book: Option<&EventBook>) -> S {
        let mut state = S::default();

        let Some(book) = event_book else {
            return state;
        };

        // Load snapshot if present
        if let Some(snapshot) = &book.snapshot {
            if let Some(snapshot_state) = &snapshot.state {
                if let Ok(s) = S::decode(snapshot_state.value.as_slice()) {
                    state = s;
                }
            }
        }

        // Apply events via registered handlers
        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };
            self.apply(&mut state, event);
        }

        state
    }
}

impl<S: Message + Default> Default for StateBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// rebuild_from_events: Closure-based approach (legacy)
// ============================================================================

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
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.test/test.CountIncremented".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot: None,
            ..Default::default()
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
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(6)),
                event: Some(prost_types::Any {
                    type_url: "type.test/test.CountIncremented".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot: Some(Snapshot {
                sequence: 5,
                state: Some(prost_types::Any {
                    type_url: "type.test/test.TestState".to_string(),
                    value: snapshot_state.encode_to_vec(),
                }),
            }),
            ..Default::default()
        };

        let state: TestState = rebuild_from_events(Some(&book), apply_test_event);
        assert_eq!(state.name, "snapshotted");
        assert_eq!(state.count, 15);
    }

    #[test]
    fn test_rebuild_empty_event_book() {
        let book = EventBook {
            cover: None,
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };

        let state: TestState = rebuild_from_events(Some(&book), apply_test_event);
        assert!(state.name.is_empty());
        assert_eq!(state.count, 0);
    }

    // ========================================================================
    // StateBuilder tests
    // ========================================================================

    /// Named event applier for StateBuilder tests.
    fn apply_count_incremented(state: &mut TestState, event: &prost_types::Any) {
        if let Ok(e) = CountIncremented::decode(event.value.as_slice()) {
            state.count = e.new_count;
        }
    }

    /// Second event type for multiple-handler tests.
    #[derive(Clone, PartialEq, prost::Message)]
    struct NameChanged {
        #[prost(string, tag = "1")]
        pub new_name: String,
    }

    fn apply_name_changed(state: &mut TestState, event: &prost_types::Any) {
        if let Ok(e) = NameChanged::decode(event.value.as_slice()) {
            state.name = e.new_name;
        }
    }

    #[test]
    fn test_state_builder_none_returns_default() {
        let builder: StateBuilder<TestState> =
            StateBuilder::new().on("CountIncremented", apply_count_incremented);

        let state = builder.rebuild(None);
        assert!(state.name.is_empty());
        assert_eq!(state.count, 0);
    }

    #[test]
    fn test_state_builder_applies_events() {
        let builder: StateBuilder<TestState> =
            StateBuilder::new().on("CountIncremented", apply_count_incremented);

        let event = CountIncremented { new_count: 42 };
        let book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.test/test.CountIncremented".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot: None,
            ..Default::default()
        };

        let state = builder.rebuild(Some(&book));
        assert_eq!(state.count, 42);
    }

    #[test]
    fn test_state_builder_multiple_handlers() {
        let builder: StateBuilder<TestState> = StateBuilder::new()
            .on("CountIncremented", apply_count_incremented)
            .on("NameChanged", apply_name_changed);

        let event1 = CountIncremented { new_count: 10 };
        let event2 = NameChanged {
            new_name: "updated".to_string(),
        };
        let book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages: vec![
                EventPage {
                    sequence: Some(Sequence::Num(0)),
                    event: Some(prost_types::Any {
                        type_url: "type.test/test.CountIncremented".to_string(),
                        value: event1.encode_to_vec(),
                    }),
                    created_at: None,
                },
                EventPage {
                    sequence: Some(Sequence::Num(1)),
                    event: Some(prost_types::Any {
                        type_url: "type.test/test.NameChanged".to_string(),
                        value: event2.encode_to_vec(),
                    }),
                    created_at: None,
                },
            ],
            snapshot: None,
            ..Default::default()
        };

        let state = builder.rebuild(Some(&book));
        assert_eq!(state.count, 10);
        assert_eq!(state.name, "updated");
    }

    #[test]
    fn test_state_builder_snapshot_plus_events() {
        let builder: StateBuilder<TestState> =
            StateBuilder::new().on("CountIncremented", apply_count_incremented);

        let snapshot_state = TestState {
            name: "from_snapshot".to_string(),
            count: 100,
        };

        let event = CountIncremented { new_count: 150 };
        let book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
                correlation_id: String::new(),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(6)),
                event: Some(prost_types::Any {
                    type_url: "type.test/test.CountIncremented".to_string(),
                    value: event.encode_to_vec(),
                }),
                created_at: None,
            }],
            snapshot: Some(Snapshot {
                sequence: 5,
                state: Some(prost_types::Any {
                    type_url: "type.test/test.TestState".to_string(),
                    value: snapshot_state.encode_to_vec(),
                }),
            }),
            ..Default::default()
        };

        let state = builder.rebuild(Some(&book));
        assert_eq!(state.name, "from_snapshot");
        assert_eq!(state.count, 150);
    }

    #[test]
    fn test_state_builder_ignores_unknown_events() {
        let builder: StateBuilder<TestState> =
            StateBuilder::new().on("CountIncremented", apply_count_incremented);

        let book = EventBook {
            cover: None,
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(0)),
                event: Some(prost_types::Any {
                    type_url: "type.test/test.UnknownEvent".to_string(),
                    value: vec![1, 2, 3],
                }),
                created_at: None,
            }],
            snapshot: None,
            ..Default::default()
        };

        let state = builder.rebuild(Some(&book));
        assert_eq!(state.count, 0); // Unchanged from default
    }
}
