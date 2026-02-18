//! Procedural macros for angzarr OO-style component definitions.
//!
//! # Aggregate Example
//!
//! ```rust,ignore
//! use angzarr_macros::{aggregate, handles, applies, rejected};
//!
//! #[aggregate(domain = "player")]
//! impl PlayerAggregate {
//!     type State = PlayerState;
//!
//!     #[applies(PlayerRegistered)]
//!     fn apply_registered(state: &mut PlayerState, event: PlayerRegistered) {
//!         state.player_id = format!("player_{}", event.email);
//!         state.display_name = event.display_name;
//!         state.exists = true;
//!     }
//!
//!     #[applies(FundsDeposited)]
//!     fn apply_deposited(state: &mut PlayerState, event: FundsDeposited) {
//!         if let Some(balance) = event.new_balance {
//!             state.bankroll = balance.amount;
//!         }
//!     }
//!
//!     #[handles(RegisterPlayer)]
//!     fn register(&self, cb: &CommandBook, cmd: RegisterPlayer, state: &PlayerState, seq: u32)
//!         -> CommandResult<EventBook> {
//!         // ...
//!     }
//!
//!     #[rejected(domain = "payment", command = "ProcessPayment")]
//!     fn handle_payment_rejected(&self, notification: &Notification, state: &PlayerState)
//!         -> CommandResult<BusinessResponse> {
//!         // ...
//!     }
//! }
//! ```
//!
//! # Saga Example
//!
//! ```rust,ignore
//! use angzarr_macros::{saga, prepares, reacts_to};
//!
//! #[saga(name = "saga-order-fulfillment", input = "order", output = "fulfillment")]
//! impl OrderFulfillmentSaga {
//!     #[prepares(OrderCompleted)]
//!     fn prepare_order(&self, event: &OrderCompleted) -> Vec<Cover> {
//!         // ...
//!     }
//!
//!     #[reacts_to(OrderCompleted)]
//!     fn handle_completed(&self, event: OrderCompleted, destinations: &[EventBook])
//!         -> CommandResult<Vec<CommandBook>> {
//!         // ...
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Ident, ImplItem, ItemImpl, Meta, Token,
};

/// Marks an impl block as an aggregate with command handlers.
///
/// # Attributes
/// - `domain = "name"` - The aggregate's domain name (required)
///
/// # Example
/// ```rust,ignore
/// #[aggregate(domain = "player")]
/// impl PlayerAggregate {
///     #[handles(RegisterPlayer)]
///     fn register(&self, cmd: RegisterPlayer, state: &PlayerState, seq: u32)
///         -> CommandResult<EventBook> {
///         // ...
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn aggregate(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AggregateArgs);
    let input = parse_macro_input!(item as ItemImpl);

    let expanded = expand_aggregate(args, input);
    TokenStream::from(expanded)
}

struct AggregateArgs {
    domain: String,
    state: Ident,
}

impl syn::parse::Parse for AggregateArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut domain = None;
        let mut state = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match ident.to_string().as_str() {
                "domain" => {
                    let value: syn::LitStr = input.parse()?;
                    domain = Some(value.value());
                }
                "state" => {
                    let value: Ident = input.parse()?;
                    state = Some(value);
                }
                _ => return Err(syn::Error::new(ident.span(), "unknown attribute")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(AggregateArgs {
            domain: domain.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "domain is required")
            })?,
            state: state.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "state is required")
            })?,
        })
    }
}

fn expand_aggregate(args: AggregateArgs, mut input: ItemImpl) -> TokenStream2 {
    let domain = &args.domain;
    let state_ty = &args.state;
    let self_ty = &input.self_ty;

    // Collect handler methods
    let mut handlers = Vec::new();
    let mut rejection_handlers = Vec::new();
    let mut appliers = Vec::new();

    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            for attr in &method.attrs {
                if attr.path().is_ident("handles") {
                    if let Ok(command_type) = get_attr_ident(attr) {
                        handlers.push((method.sig.ident.clone(), command_type));
                    }
                } else if attr.path().is_ident("rejected") {
                    if let Ok((domain, command)) = get_rejected_args(attr) {
                        rejection_handlers.push((method.sig.ident.clone(), domain, command));
                    }
                } else if attr.path().is_ident("applies") {
                    if let Ok(event_type) = get_attr_ident(attr) {
                        appliers.push((method.sig.ident.clone(), event_type));
                    }
                }
            }
        }
    }

    // Generate handler registrations with command decoding
    // Each handler needs its own clone of the Arc<Self>
    let handler_registrations: Vec<_> = handlers
        .iter()
        .map(|(method, cmd_type)| {
            let cmd_str = cmd_type.to_string();
            quote! {
                .on(#cmd_str, {
                    let agg = agg.clone();
                    move |cb, cmd_any, state, seq| {
                        let cmd = <#cmd_type as prost::Message>::decode(cmd_any.value.as_slice())
                            .map_err(|e| angzarr_client::CommandRejectedError::new(format!("Failed to decode {}: {}", #cmd_str, e)))?;
                        agg.#method(cb, cmd, state, seq)
                    }
                })
            }
        })
        .collect();

    let rejection_registrations: Vec<_> = rejection_handlers
        .iter()
        .map(|(method, domain, command)| {
            quote! {
                .on_rejected(#domain, #command, {
                    let agg = agg.clone();
                    move |notification, state| agg.#method(notification, state)
                })
            }
        })
        .collect();

    // Generate apply_event dispatch arms
    let apply_arms: Vec<_> = appliers
        .iter()
        .map(|(method, event_type)| {
            let suffix = event_type.to_string();
            quote! {
                if event_any.type_url.ends_with(#suffix) {
                    if let Ok(event) = <#event_type as prost::Message>::decode(event_any.value.as_slice()) {
                        Self::#method(state, event);
                        return;
                    }
                }
            }
        })
        .collect();

    // Remove our attributes from methods (they're not real Rust attributes)
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|attr| {
                !attr.path().is_ident("handles")
                    && !attr.path().is_ident("rejected")
                    && !attr.path().is_ident("applies")
            });
        }
    }

    // Generate apply_event and rebuild functions if appliers exist
    let apply_event_fn = if !appliers.is_empty() {
        quote! {
            /// Apply a single event to state. Auto-generated from #[applies] methods.
            pub fn apply_event(state: &mut #state_ty, event_any: &prost_types::Any) {
                #(#apply_arms)*
                // Unknown event type - silently ignore (forward compatibility)
            }

            /// Rebuild state from event book. Auto-generated.
            pub fn rebuild(events: &angzarr_client::proto::EventBook) -> #state_ty {
                let mut state = #state_ty::default();
                for page in &events.pages {
                    if let Some(event) = &page.event {
                        Self::apply_event(&mut state, event);
                    }
                }
                state
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #input

        impl #self_ty {
            #apply_event_fn

            /// Creates a CommandRouter from this aggregate's annotated methods.
            pub fn into_router(self) -> angzarr_client::CommandRouter<#state_ty>
            where
                Self: Send + Sync + 'static,
            {
                let agg = std::sync::Arc::new(self);
                angzarr_client::CommandRouter::new(#domain, Self::rebuild)
                    #(#handler_registrations)*
                    #(#rejection_registrations)*
            }
        }
    }
}

/// Marks a method as a command handler.
///
/// # Example
/// ```rust,ignore
/// #[handles(RegisterPlayer)]
/// fn register(&self, cmd: RegisterPlayer, state: &PlayerState, seq: u32)
///     -> CommandResult<EventBook> {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn handles(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // The actual work is done by the #[aggregate] macro
    // This is just a marker attribute
    item
}

/// Marks a method as a rejection handler.
///
/// # Attributes
/// - `domain = "name"` - The domain of the rejected command
/// - `command = "name"` - The type of the rejected command
///
/// # Example
/// ```rust,ignore
/// #[rejected(domain = "payment", command = "ProcessPayment")]
/// fn handle_payment_rejected(&self, notification: &Notification, state: &PlayerState)
///     -> CommandResult<BusinessResponse> {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn rejected(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // The actual work is done by the #[aggregate] or #[process_manager] macro
    // This is just a marker attribute
    item
}

/// Marks a method as an event applier for state reconstruction.
///
/// The method must be a static function with signature:
/// `fn(state: &mut State, event: EventType)`
///
/// The #[aggregate] macro collects these and generates:
/// - `apply_event(state, event_any)` - dispatches to the right applier
/// - `rebuild(events)` - reconstructs state from event book
///
/// # Example
/// ```rust,ignore
/// #[applies(PlayerRegistered)]
/// fn apply_registered(state: &mut PlayerState, event: PlayerRegistered) {
///     state.player_id = format!("player_{}", event.email);
///     state.display_name = event.display_name;
///     state.exists = true;
/// }
///
/// #[applies(FundsDeposited)]
/// fn apply_deposited(state: &mut PlayerState, event: FundsDeposited) {
///     if let Some(balance) = event.new_balance {
///         state.bankroll = balance.amount;
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn applies(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // The actual work is done by the #[aggregate] macro
    // This is just a marker attribute
    item
}

/// Marks an impl block as a saga with event handlers.
///
/// # Attributes
/// - `name = "saga-name"` - The saga's name (required)
/// - `input = "domain"` - Input domain to listen to (required)
/// - `output = "domain"` - Output domain for commands (required)
///
/// # Example
/// ```rust,ignore
/// #[saga(name = "saga-order-fulfillment", input = "order", output = "fulfillment")]
/// impl OrderFulfillmentSaga {
///     #[reacts_to(OrderCompleted)]
///     fn handle_completed(&self, event: OrderCompleted, destinations: &[EventBook])
///         -> CommandResult<Vec<CommandBook>> {
///         // ...
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn saga(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SagaArgs);
    let input = parse_macro_input!(item as ItemImpl);

    let expanded = expand_saga(args, input);
    TokenStream::from(expanded)
}

struct SagaArgs {
    name: String,
    input: String,
    output: String,
}

impl syn::parse::Parse for SagaArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut input_domain = None;
        let mut output = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: syn::LitStr = input.parse()?;

            match ident.to_string().as_str() {
                "name" => name = Some(value.value()),
                "input" => input_domain = Some(value.value()),
                "output" => output = Some(value.value()),
                _ => return Err(syn::Error::new(ident.span(), "unknown attribute")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(SagaArgs {
            name: name.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "name is required")
            })?,
            input: input_domain.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "input is required")
            })?,
            output: output.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "output is required")
            })?,
        })
    }
}

fn expand_saga(args: SagaArgs, mut input: ItemImpl) -> TokenStream2 {
    let name = &args.name;
    let input_domain = &args.input;
    let output_domain = &args.output;
    let self_ty = &input.self_ty;

    // Collect handler methods
    let mut prepare_handlers = Vec::new();
    let mut event_handlers = Vec::new();

    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            for attr in &method.attrs {
                if attr.path().is_ident("prepares") {
                    if let Ok(event_type) = get_attr_ident(attr) {
                        prepare_handlers.push((method.sig.ident.clone(), event_type));
                    }
                } else if attr.path().is_ident("reacts_to") {
                    if let Ok(event_type) = get_attr_ident(attr) {
                        event_handlers.push((method.sig.ident.clone(), event_type));
                    }
                }
            }
        }
    }

    // Generate prepare handler registrations with event decoding
    let prepare_registrations: Vec<_> = prepare_handlers
        .iter()
        .map(|(method, event_type)| {
            let event_str = event_type.to_string();
            quote! {
                .prepare_fn(#event_str, {
                    let saga = saga.clone();
                    move |_source, event_any| {
                        if let Ok(event) = <#event_type as prost::Message>::decode(event_any.value.as_slice()) {
                            saga.#method(&event)
                        } else {
                            vec![]
                        }
                    }
                })
            }
        })
        .collect();

    // Generate event handler registrations with event decoding
    let handler_registrations: Vec<_> = event_handlers
        .iter()
        .map(|(method, event_type)| {
            let event_str = event_type.to_string();
            quote! {
                .on_many_fn(#event_str, {
                    let saga = saga.clone();
                    move |_source, event_any, destinations| {
                        let event = <#event_type as prost::Message>::decode(event_any.value.as_slice())
                            .map_err(|e| angzarr_client::CommandRejectedError::new(format!("Failed to decode {}: {}", #event_str, e)))?;
                        saga.#method(event, destinations)
                    }
                })
            }
        })
        .collect();

    // Remove our attributes from methods
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|attr| {
                !attr.path().is_ident("prepares") && !attr.path().is_ident("reacts_to")
            });
        }
    }

    quote! {
        #input

        impl #self_ty {
            /// Creates an EventRouter from this saga's annotated methods.
            pub fn into_router(self) -> angzarr_client::EventRouter
            where
                Self: Send + Sync + 'static,
            {
                let saga = std::sync::Arc::new(self);
                angzarr_client::EventRouter::new(#name, #input_domain)
                    .sends_domain(#output_domain)
                    #(#prepare_registrations)*
                    #(#handler_registrations)*
            }
        }
    }
}

/// Marks a method as a prepare handler for destination declaration.
///
/// # Example
/// ```rust,ignore
/// #[prepares(OrderCompleted)]
/// fn prepare_order(&self, event: &OrderCompleted) -> Vec<Cover> {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn prepares(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a method as an event handler.
///
/// # Example
/// ```rust,ignore
/// #[reacts_to(OrderCompleted)]
/// fn handle_completed(&self, event: OrderCompleted, destinations: &[EventBook])
///     -> CommandResult<Vec<CommandBook>> {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn reacts_to(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks an impl block as a process manager with event handlers.
///
/// # Attributes
/// - `name = "pm-name"` - The PM's name (required)
/// - `domain = "pm-domain"` - The PM's own domain for state (required)
/// - `state = StateType` - The PM's state type (required)
/// - `inputs = ["domain1", "domain2"]` - Input domains to subscribe to (required)
///
/// # Example
/// ```rust,ignore
/// #[process_manager(name = "hand-flow", domain = "hand-flow", state = PMState, inputs = ["table", "hand"])]
/// impl HandFlowPM {
///     #[applies(PMStateUpdated)]
///     fn apply_state(state: &mut PMState, event: PMStateUpdated) {
///         // ...
///     }
///
///     #[prepares(HandStarted)]
///     fn prepare_hand(&self, trigger: &EventBook, state: &PMState, event: &HandStarted) -> Vec<Cover> {
///         // ...
///     }
///
///     #[handles(HandStarted)]
///     fn handle_hand(&self, trigger: &EventBook, state: &PMState, event: HandStarted, destinations: &[EventBook])
///         -> CommandResult<ProcessManagerResponse> {
///         // ...
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn process_manager(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ProcessManagerArgs);
    let input = parse_macro_input!(item as ItemImpl);

    let expanded = expand_process_manager(args, input);
    TokenStream::from(expanded)
}

struct ProcessManagerArgs {
    name: String,
    domain: String,
    state: Ident,
    inputs: Vec<String>,
}

impl syn::parse::Parse for ProcessManagerArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut domain = None;
        let mut state = None;
        let mut inputs = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match ident.to_string().as_str() {
                "name" => {
                    let value: syn::LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "domain" => {
                    let value: syn::LitStr = input.parse()?;
                    domain = Some(value.value());
                }
                "state" => {
                    let value: Ident = input.parse()?;
                    state = Some(value);
                }
                "inputs" => {
                    let content;
                    syn::bracketed!(content in input);
                    let mut domains = Vec::new();
                    while !content.is_empty() {
                        let lit: syn::LitStr = content.parse()?;
                        domains.push(lit.value());
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    inputs = Some(domains);
                }
                _ => return Err(syn::Error::new(ident.span(), "unknown attribute")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(ProcessManagerArgs {
            name: name.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "name is required")
            })?,
            domain: domain.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "domain is required")
            })?,
            state: state.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "state is required")
            })?,
            inputs: inputs.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "inputs is required")
            })?,
        })
    }
}

fn expand_process_manager(args: ProcessManagerArgs, mut input: ItemImpl) -> TokenStream2 {
    let name = &args.name;
    let pm_domain = &args.domain;
    let state_ty = &args.state;
    let inputs = &args.inputs;
    let self_ty = &input.self_ty;

    // Collect handler methods
    let mut prepare_handlers = Vec::new();
    let mut event_handlers = Vec::new();
    let mut appliers = Vec::new();

    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            for attr in &method.attrs {
                if attr.path().is_ident("prepares") {
                    if let Ok(event_type) = get_attr_ident(attr) {
                        prepare_handlers.push((method.sig.ident.clone(), event_type));
                    }
                } else if attr.path().is_ident("handles") {
                    if let Ok(event_type) = get_attr_ident(attr) {
                        event_handlers.push((method.sig.ident.clone(), event_type));
                    }
                } else if attr.path().is_ident("applies") {
                    if let Ok(event_type) = get_attr_ident(attr) {
                        appliers.push((method.sig.ident.clone(), event_type));
                    }
                }
            }
        }
    }

    // Generate prepare handler registrations with event decoding
    let prepare_registrations: Vec<_> = prepare_handlers
        .iter()
        .map(|(method, event_type)| {
            let event_str = event_type.to_string();
            quote! {
                .prepare_fn(#event_str, {
                    let pm = pm.clone();
                    move |trigger, state, event_any| {
                        if let Ok(event) = <#event_type as prost::Message>::decode(event_any.value.as_slice()) {
                            pm.#method(trigger, state, &event)
                        } else {
                            vec![]
                        }
                    }
                })
            }
        })
        .collect();

    // Generate event handler registrations with event decoding
    let handler_registrations: Vec<_> = event_handlers
        .iter()
        .map(|(method, event_type)| {
            let event_str = event_type.to_string();
            quote! {
                .on_fn(#event_str, {
                    let pm = pm.clone();
                    move |trigger, state, event_any, destinations| {
                        let event = <#event_type as prost::Message>::decode(event_any.value.as_slice())
                            .map_err(|e| angzarr_client::CommandRejectedError::new(format!("Failed to decode {}: {}", #event_str, e)))?;
                        pm.#method(trigger, state, event, destinations)
                    }
                })
            }
        })
        .collect();

    // Generate subscribes registrations
    let subscribes_registrations: Vec<_> = inputs
        .iter()
        .map(|domain| {
            quote! {
                .subscribes(#domain)
            }
        })
        .collect();

    // Generate apply_event dispatch arms
    let apply_arms: Vec<_> = appliers
        .iter()
        .map(|(method, event_type)| {
            let suffix = event_type.to_string();
            quote! {
                if event_any.type_url.ends_with(#suffix) {
                    if let Ok(event) = <#event_type as prost::Message>::decode(event_any.value.as_slice()) {
                        Self::#method(state, event);
                        return;
                    }
                }
            }
        })
        .collect();

    // Remove our attributes from methods
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|attr| {
                !attr.path().is_ident("prepares")
                    && !attr.path().is_ident("handles")
                    && !attr.path().is_ident("applies")
            });
        }
    }

    // Generate apply_event and rebuild functions if appliers exist
    let apply_event_fn = if !appliers.is_empty() {
        quote! {
            /// Apply a single event to state. Auto-generated from #[applies] methods.
            pub fn apply_event(state: &mut #state_ty, event_any: &prost_types::Any) {
                #(#apply_arms)*
                // Unknown event type - silently ignore (forward compatibility)
            }

            /// Rebuild state from event book. Auto-generated.
            pub fn rebuild(events: &angzarr_client::proto::EventBook) -> #state_ty {
                let mut state = #state_ty::default();
                for page in &events.pages {
                    if let Some(event) = &page.event {
                        Self::apply_event(&mut state, event);
                    }
                }
                state
            }
        }
    } else {
        quote! {
            /// Rebuild state from event book. Returns default state (no #[applies] methods).
            pub fn rebuild(_events: &angzarr_client::proto::EventBook) -> #state_ty {
                #state_ty::default()
            }
        }
    };

    quote! {
        #input

        impl #self_ty {
            #apply_event_fn

            /// Creates a ProcessManagerRouter from this PM's annotated methods.
            pub fn into_router(self) -> angzarr_client::ProcessManagerRouter<#state_ty>
            where
                Self: Send + Sync + 'static,
            {
                let pm = std::sync::Arc::new(self);
                angzarr_client::ProcessManagerRouter::new_with_rebuild_fn(#name, #pm_domain, Self::rebuild)
                    #(#subscribes_registrations)*
                    #(#prepare_registrations)*
                    #(#handler_registrations)*
            }
        }
    }
}

/// Marks a method as a projector event handler.
///
/// # Example
/// ```rust,ignore
/// #[projects(PlayerRegistered)]
/// fn project_registered(&self, event: PlayerRegistered) -> Projection {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn projects(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks an impl block as a projector with event handlers.
///
/// # Attributes
/// - `name = "projector-name"` - The projector's name (required)
/// - `inputs = ["domain1", "domain2"]` - Input domains to subscribe to (required)
///
/// # Example
/// ```rust,ignore
/// #[projector(name = "output", inputs = ["player", "table", "hand"])]
/// impl OutputProjector {
///     #[projects(PlayerRegistered)]
///     fn project_registered(&self, event: PlayerRegistered) -> Projection {
///         // ...
///     }
///
///     #[projects(HandComplete)]
///     fn project_hand_complete(&self, event: HandComplete) -> Projection {
///         // ...
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn projector(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ProjectorArgs);
    let input = parse_macro_input!(item as ItemImpl);

    let expanded = expand_projector(args, input);
    TokenStream::from(expanded)
}

struct ProjectorArgs {
    name: String,
    inputs: Vec<String>,
}

impl syn::parse::Parse for ProjectorArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut inputs = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match ident.to_string().as_str() {
                "name" => {
                    let value: syn::LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "inputs" => {
                    let content;
                    syn::bracketed!(content in input);
                    let mut domains = Vec::new();
                    while !content.is_empty() {
                        let lit: syn::LitStr = content.parse()?;
                        domains.push(lit.value());
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    inputs = Some(domains);
                }
                _ => return Err(syn::Error::new(ident.span(), "unknown attribute")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(ProjectorArgs {
            name: name.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "name is required")
            })?,
            inputs: inputs.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "inputs is required")
            })?,
        })
    }
}

fn expand_projector(args: ProjectorArgs, mut input: ItemImpl) -> TokenStream2 {
    let name = &args.name;
    let inputs = &args.inputs;
    let self_ty = &input.self_ty;

    // Collect handler methods
    let mut event_handlers = Vec::new();

    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            for attr in &method.attrs {
                if attr.path().is_ident("projects") {
                    if let Ok(event_type) = get_attr_ident(attr) {
                        event_handlers.push((method.sig.ident.clone(), event_type));
                    }
                }
            }
        }
    }

    // Generate event dispatch arms
    let handler_arms: Vec<_> = event_handlers
        .iter()
        .map(|(method, event_type)| {
            let suffix = event_type.to_string();
            quote! {
                if type_url.ends_with(#suffix) {
                    if let Ok(event) = <#event_type as prost::Message>::decode(event_any.value.as_slice()) {
                        return Some(self.#method(event));
                    }
                }
            }
        })
        .collect();

    // Generate the handle_event dispatch function
    let dispatch_fn = if !event_handlers.is_empty() {
        quote! {
            /// Dispatch a single event to the appropriate handler.
            fn handle_event(&self, event_any: &prost_types::Any) -> Option<angzarr_client::proto::Projection> {
                let type_url = &event_any.type_url;
                #(#handler_arms)*
                None
            }
        }
    } else {
        quote! {
            fn handle_event(&self, _event_any: &prost_types::Any) -> Option<angzarr_client::proto::Projection> {
                None
            }
        }
    };

    // Remove our attributes from methods
    for item in &mut input.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|attr| !attr.path().is_ident("projects"));
        }
    }

    // Build inputs vec
    let inputs_vec: Vec<_> = inputs.iter().map(|d| quote! { #d.to_string() }).collect();

    quote! {
        #input

        impl #self_ty {
            #dispatch_fn

            /// Handle an EventBook by dispatching each event to handlers.
            pub fn handle(&self, events: &angzarr_client::proto::EventBook) -> angzarr_client::proto::Projection {
                let cover = events.cover.as_ref();
                let mut last_seq = 0u32;

                for page in &events.pages {
                    if let Some(event_any) = &page.event {
                        if let Some(projection) = self.handle_event(event_any) {
                            return projection;
                        }
                    }
                    if let Some(angzarr_client::proto::event_page::Sequence::Num(n)) = &page.sequence {
                        last_seq = *n;
                    }
                }

                // Default projection if no handler matched
                angzarr_client::proto::Projection {
                    cover: cover.cloned(),
                    projector: #name.to_string(),
                    sequence: last_seq,
                    projection: None,
                }
            }

            /// Creates a ProjectorHandler from this projector.
            pub fn into_handler(self) -> angzarr_client::ProjectorHandler
            where
                Self: Send + Sync + 'static,
            {
                let projector = std::sync::Arc::new(self);
                angzarr_client::ProjectorHandler::new(
                    #name,
                    vec![#(#inputs_vec),*],
                ).with_handle_fn(move |events| {
                    Ok(projector.handle(events))
                })
            }
        }
    }
}

// Helper functions

fn get_attr_ident(attr: &Attribute) -> syn::Result<Ident> {
    let meta = attr.meta.clone();
    match meta {
        Meta::List(list) => {
            let ident: Ident = syn::parse2(list.tokens)?;
            Ok(ident)
        }
        _ => Err(syn::Error::new_spanned(attr, "expected #[attr(Type)]")),
    }
}

fn get_rejected_args(attr: &Attribute) -> syn::Result<(String, String)> {
    let meta = attr.meta.clone();
    match meta {
        Meta::List(list) => {
            let args: RejectedArgs = syn::parse2(list.tokens)?;
            Ok((args.domain, args.command))
        }
        _ => Err(syn::Error::new_spanned(
            attr,
            "expected #[rejected(domain = \"...\", command = \"...\")]",
        )),
    }
}

struct RejectedArgs {
    domain: String,
    command: String,
}

impl syn::parse::Parse for RejectedArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut domain = None;
        let mut command = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: syn::LitStr = input.parse()?;

            match ident.to_string().as_str() {
                "domain" => domain = Some(value.value()),
                "command" => command = Some(value.value()),
                _ => return Err(syn::Error::new(ident.span(), "unknown attribute")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(RejectedArgs {
            domain: domain.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "domain is required")
            })?,
            command: command.ok_or_else(|| {
                syn::Error::new(proc_macro2::Span::call_site(), "command is required")
            })?,
        })
    }
}
