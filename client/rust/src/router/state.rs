//! State reconstruction from events.
//!
//! `StateRouter` provides fluent registration of event appliers for
//! rebuilding aggregate/PM state from event streams.

use prost_types::Any;

use crate::proto::{event_page, EventBook, EventPage};

/// Event applier function type.
///
/// Takes mutable state reference and event bytes (to be decoded by handler).
pub type EventApplier<S> = Box<dyn Fn(&mut S, &[u8]) + Send + Sync>;

/// Factory function type for creating initial state.
pub type StateFactory<S> = Box<dyn Fn() -> S + Send + Sync>;

/// Fluent state reconstruction router.
///
/// Provides a builder pattern for registering event appliers with auto-unpacking.
/// Register once at startup, call `with_events()` per rebuild.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::StateRouter;
///
/// fn apply_registered(state: &mut PlayerState, event: PlayerRegistered) {
///     state.player_id = format!("player_{}", event.email);
///     state.display_name = event.display_name;
///     state.exists = true;
/// }
///
/// fn apply_deposited(state: &mut PlayerState, event: FundsDeposited) {
///     if let Some(balance) = event.new_balance {
///         state.bankroll = balance.amount;
///     }
/// }
///
/// // Build router once
/// let player_router = StateRouter::<PlayerState>::new()
///     .on::<PlayerRegistered>(apply_registered)
///     .on::<FundsDeposited>(apply_deposited);
///
/// // Use per rebuild
/// fn rebuild_state(event_book: &EventBook) -> PlayerState {
///     player_router.with_event_book(event_book)
/// }
/// ```
pub struct StateRouter<S: Default> {
    handlers: Vec<(String, EventApplier<S>)>,
    factory: Option<StateFactory<S>>,
}

impl<S: Default + 'static> Default for StateRouter<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Default + 'static> StateRouter<S> {
    /// Create a new StateRouter using `S::default()` for state creation.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            factory: None,
        }
    }

    /// Create a StateRouter with a custom state factory.
    ///
    /// Use this when your state needs non-default initialization.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn new_hand_state() -> HandState {
    ///     HandState {
    ///         pots: vec![PotState { pot_type: "main".to_string(), ..Default::default() }],
    ///         ..Default::default()
    ///     }
    /// }
    ///
    /// let router = StateRouter::with_factory(new_hand_state)
    ///     .on::<CardsDealt>(apply_cards_dealt);
    /// ```
    pub fn with_factory(factory: fn() -> S) -> Self {
        Self {
            handlers: Vec::new(),
            factory: Some(Box::new(factory)),
        }
    }

    /// Create a new state instance using factory or Default.
    fn create_state(&self) -> S {
        match &self.factory {
            Some(factory) => factory(),
            None => S::default(),
        }
    }

    /// Register an event applier for the given protobuf event type.
    ///
    /// The handler receives typed events (auto-decoded from protobuf).
    /// Type name is extracted via reflection using `prost::Name::full_name()`.
    ///
    /// # Type Parameters
    ///
    /// - `E`: The protobuf event type (must implement `prost::Message + Default + prost::Name`)
    ///
    /// # Arguments
    ///
    /// - `handler`: Function that takes `(&mut S, E)` and mutates state
    pub fn on<E>(mut self, handler: fn(&mut S, E)) -> Self
    where
        E: prost::Message + Default + prost::Name + 'static,
    {
        let type_name = E::full_name();
        let boxed: EventApplier<S> = Box::new(move |state, bytes| {
            if let Ok(event) = E::decode(bytes) {
                handler(state, event);
            }
        });
        self.handlers.push((type_name, boxed));
        self
    }

    /// Create fresh state and apply all events from pages.
    ///
    /// This is the terminal operation for standalone usage.
    pub fn with_events(&self, pages: &[EventPage]) -> S {
        let mut state = self.create_state();
        for page in pages {
            if let Some(event_page::Payload::Event(event)) = &page.payload {
                self.apply_single(&mut state, event);
            }
        }
        state
    }

    /// Create fresh state and apply all events from an EventBook.
    pub fn with_event_book(&self, event_book: &EventBook) -> S {
        self.with_events(&event_book.pages)
    }

    /// Apply a single event to existing state.
    ///
    /// Matches using fully qualified type name from `prost::Name`.
    pub fn apply_single(&self, state: &mut S, event_any: &Any) {
        let type_url = &event_any.type_url;
        for (type_name, handler) in &self.handlers {
            if Self::type_matches(type_url, type_name) {
                handler(state, &event_any.value);
                return;
            }
        }
        // Unknown event type — silently ignore (forward compatibility)
    }

    /// Check if type_url exactly matches the given fully qualified type name.
    ///
    /// type_name should be fully qualified (e.g., "examples.CardsDealt").
    /// Compares type_url == "type.googleapis.com/" + type_name.
    fn type_matches(type_url: &str, type_name: &str) -> bool {
        type_url == format!("type.googleapis.com/{}", type_name)
    }

    /// Convert to a rebuilder closure for use with Router.
    ///
    /// Returns a closure that can be passed to Router constructors.
    pub fn into_rebuilder(self) -> impl Fn(&EventBook) -> S + Send + Sync {
        move |event_book| self.with_event_book(event_book)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_matches_requires_fully_qualified_name() {
        assert!(StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CardsDealt",
            "examples.CardsDealt"
        ));
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CardsDealt",
            "CardsDealt"
        ));
    }

    #[test]
    fn type_matches_rejects_partial_names() {
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CommunityCardsDealt",
            "examples.CardsDealt"
        ));
        assert!(StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CommunityCardsDealt",
            "examples.CommunityCardsDealt"
        ));
    }

    #[test]
    fn type_matches_rejects_wrong_package() {
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.CardsDealt",
            "other.CardsDealt"
        ));
    }

    #[test]
    fn type_matches_handles_edge_cases() {
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.Test",
            ""
        ));
        assert!(!StateRouter::<()>::type_matches(
            "type.googleapis.com/examples.Other",
            "examples.CardsDealt"
        ));
    }

    #[test]
    fn state_router_default() {
        let router: StateRouter<String> = StateRouter::default();
        let state = router.with_events(&[]);
        assert_eq!(state, String::default());
    }
}
