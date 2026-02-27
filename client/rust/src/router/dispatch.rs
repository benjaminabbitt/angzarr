//! Dispatch helpers for routing events/commands by type URL.

/// Helper macro for dispatching events by type URL suffix.
///
/// Simplifies the common pattern of matching event type URLs and delegating
/// to handler methods.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::dispatch_event;
///
/// impl SagaDomainHandler for OrderSagaHandler {
///     fn execute(
///         &self,
///         source: &EventBook,
///         event: &Any,
///         destinations: &[EventBook],
///     ) -> CommandResult<Vec<CommandBook>> {
///         dispatch_event!(event, source, destinations, {
///             "OrderCompleted" => self.handle_completed,
///             "OrderCancelled" => self.handle_cancelled,
///         })
///     }
/// }
/// ```
///
/// # Variants
///
/// ## For saga execute (source + destinations)
/// ```rust,ignore
/// dispatch_event!(event, source, destinations, {
///     "Suffix" => handler_method,
/// })
/// ```
///
/// ## For saga prepare (source only, returns Vec<Cover>)
/// ```rust,ignore
/// dispatch_event!(event, source, {
///     "Suffix" => prepare_method,
/// })
/// ```
#[macro_export]
macro_rules! dispatch_event {
    // Saga execute variant: (event, source, destinations, handlers)
    ($event:expr, $source:expr, $destinations:expr, { $($suffix:literal => $handler:expr),* $(,)? }) => {{
        let type_url = &$event.type_url;
        $(
            if type_url.ends_with($suffix) {
                return $handler($source, $event, $destinations);
            }
        )*
        Ok(vec![])
    }};

    // Saga prepare variant: (event, source, handlers) -> Vec<Cover>
    ($event:expr, $source:expr, { $($suffix:literal => $handler:expr),* $(,)? }) => {{
        let type_url = &$event.type_url;
        $(
            if type_url.ends_with($suffix) {
                return $handler($source, $event);
            }
        )*
        vec![]
    }};
}

/// Helper macro for dispatching commands by type URL suffix.
///
/// Similar to `dispatch_event!` but for command handler handlers.
///
/// # Example
///
/// ```rust,ignore
/// use angzarr_client::dispatch_command;
///
/// impl CommandHandlerDomainHandler for PlayerHandler {
///     fn handle(
///         &self,
///         cmd: &CommandBook,
///         payload: &Any,
///         state: &PlayerState,
///         seq: u32,
///     ) -> CommandResult<EventBook> {
///         dispatch_command!(payload, cmd, state, seq, {
///             "RegisterPlayer" => self.handle_register,
///             "DepositFunds" => self.handle_deposit,
///         })
///     }
/// }
/// ```
#[macro_export]
macro_rules! dispatch_command {
    ($payload:expr, $cmd:expr, $state:expr, $seq:expr, { $($suffix:literal => $handler:expr),* $(,)? }) => {{
        let type_url = &$payload.type_url;
        $(
            if type_url.ends_with($suffix) {
                return $handler($cmd, $payload, $state, $seq);
            }
        )*
        Err($crate::CommandRejectedError::new(format!("Unknown command type: {}", type_url)))
    }};
}

// Macros are exported at crate level via #[macro_export]
// Re-exported from router module for documentation purposes
