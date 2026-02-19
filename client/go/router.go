// Package angzarr provides DRY dispatch via router types.
//
// CommandRouter replaces manual switch statements in aggregate handlers.
// EventRouter replaces manual switch statements in saga event handlers.
// Both auto-derive descriptors from their On() registrations.
package angzarr

import (
	"fmt"
	"reflect"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// Error constants.
const (
	ErrMsgUnknownCommand = "unknown command type"
	ErrMsgNoCommandPages = "no command pages"
)

// CommandHandler handles a command and returns events.
// Parameters:
//   - cb: The full CommandBook
//   - cmd: The unpacked command Any
//   - state: Rebuilt state from prior events
//   - seq: Next event sequence number
//
// Returns: EventBook containing produced events
type CommandHandler[S any] func(cb *pb.CommandBook, cmd *anypb.Any, state S, seq uint32) (*pb.EventBook, error)

// StateRebuilder reconstructs state from prior events.
type StateRebuilder[S any] func(events *pb.EventBook) S

// RevocationHandler handles saga compensation requests.
// Called when a saga command targeting this aggregate's events is rejected.
//
// Parameters:
//   - notification: Notification containing RejectionNotification payload
//   - state: Current aggregate state
//
// Returns: BusinessResponse with events or RevocationResponse
//
// To access rejection details:
//
//	rejection := &pb.RejectionNotification{}
//	notification.Payload.UnmarshalTo(rejection)
type RevocationHandler[S any] func(notification *pb.Notification, state S) *pb.BusinessResponse

// CommandRouter dispatches commands to handlers by type_url suffix.
//
// Example:
//
//	router := NewCommandRouter("cart", rebuildState).
//	    On("CreateCart", handleCreateCart).
//	    On("AddItem", handleAddItem).
//	    OnRejected("payment", "ProcessPayment", handlePaymentRejected)
//
//	// In Handle():
//	response, err := router.Dispatch(request)
type CommandRouter[S any] struct {
	domain            string
	rebuild           StateRebuilder[S]
	handlers          []commandRegistration[S]
	rejectionHandlers map[string]RevocationHandler[S] // Key: "domain/command"
}

type commandRegistration[S any] struct {
	suffix  string
	handler CommandHandler[S]
}

// NewCommandRouter creates a new router for the given domain.
func NewCommandRouter[S any](domain string, rebuild StateRebuilder[S]) *CommandRouter[S] {
	return &CommandRouter[S]{
		domain:            domain,
		rebuild:           rebuild,
		handlers:          make([]commandRegistration[S], 0),
		rejectionHandlers: make(map[string]RevocationHandler[S]),
	}
}

// On registers a handler for a command type_url suffix.
func (r *CommandRouter[S]) On(suffix string, handler CommandHandler[S]) *CommandRouter[S] {
	r.handlers = append(r.handlers, commandRegistration[S]{suffix: suffix, handler: handler})
	return r
}

// OnRejected registers a handler for rejected commands.
//
// Called when a saga/PM command targeting the specified domain and command
// type is rejected by the target aggregate. The handler should decide whether to:
// 1. Emit compensation events (return with Events)
// 2. Delegate to framework (return with RevocationResponse)
//
// If no handler matches, revocations delegate to framework by default.
//
// Example:
//
//	router.OnRejected("payment", "ProcessPayment", handlePaymentRejected)
func (r *CommandRouter[S]) OnRejected(domain, command string, handler RevocationHandler[S]) *CommandRouter[S] {
	key := domain + "/" + command
	r.rejectionHandlers[key] = handler
	return r
}

// Dispatch routes a ContextualCommand to the matching handler.
//
// Extracts command + prior events, rebuilds state, matches type_url suffix,
// and calls the registered handler. Detects Notification and routes
// to the rejection handler.
func (r *CommandRouter[S]) Dispatch(cmd *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	commandBook := cmd.Command
	priorEvents := cmd.Events

	state := r.rebuild(priorEvents)
	seq := NextSequence(priorEvents)

	if len(commandBook.Pages) == 0 {
		return nil, fmt.Errorf("%s", ErrMsgNoCommandPages)
	}

	commandAny := commandBook.Pages[0].GetCommand()
	if commandAny == nil || commandAny.TypeUrl == "" {
		return nil, fmt.Errorf("%s", ErrMsgNoCommandPages)
	}

	typeURL := commandAny.TypeUrl

	// Check for Notification (rejection/compensation)
	if strings.HasSuffix(typeURL, "Notification") {
		notification := &pb.Notification{}
		if err := commandAny.UnmarshalTo(notification); err != nil {
			return nil, fmt.Errorf("failed to unmarshal Notification: %w", err)
		}
		return r.dispatchRejection(notification, state)
	}

	// Normal command dispatch
	for _, reg := range r.handlers {
		if strings.HasSuffix(typeURL, reg.suffix) {
			events, err := reg.handler(commandBook, commandAny, state, seq)
			if err != nil {
				return nil, err
			}
			return &pb.BusinessResponse{
				Result: &pb.BusinessResponse_Events{Events: events},
			}, nil
		}
	}

	return nil, fmt.Errorf("%s: %s", ErrMsgUnknownCommand, typeURL)
}

// dispatchRejection routes a rejection Notification to the matching handler.
func (r *CommandRouter[S]) dispatchRejection(notification *pb.Notification, state S) (*pb.BusinessResponse, error) {
	// Unpack rejection details from notification payload
	rejection := &pb.RejectionNotification{}
	if notification.Payload != nil {
		if err := notification.Payload.UnmarshalTo(rejection); err != nil {
			return nil, fmt.Errorf("failed to unmarshal RejectionNotification: %w", err)
		}
	}

	// Extract domain and command type from rejected_command
	var domain, cmdSuffix string
	if rejection.RejectedCommand != nil && len(rejection.RejectedCommand.Pages) > 0 {
		if rejection.RejectedCommand.Cover != nil {
			domain = rejection.RejectedCommand.Cover.Domain
		}
		if cmd := rejection.RejectedCommand.Pages[0].GetCommand(); cmd != nil {
			cmdTypeURL := cmd.TypeUrl
			if idx := strings.LastIndex(cmdTypeURL, "/"); idx >= 0 {
				cmdSuffix = cmdTypeURL[idx+1:]
			} else {
				cmdSuffix = cmdTypeURL
			}
		}
	}

	// Build dispatch key and look up handler
	key := domain + "/" + cmdSuffix
	if handler, ok := r.rejectionHandlers[key]; ok {
		return handler(notification, state), nil
	}

	return DelegateToFramework(
		fmt.Sprintf("Aggregate %s has no custom compensation for %s", r.domain, key),
	), nil
}

// RebuildState reconstructs state from an EventBook using the registered rebuilder.
//
// This is used by the Replay RPC to compute state from events.
func (r *CommandRouter[S]) RebuildState(events *pb.EventBook) S {
	return r.rebuild(events)
}

// EventHandler handles an event and returns commands for other aggregates.
// Parameters:
//   - source: The source EventBook
//   - event: The event Any from the EventPage
//   - destinations: EventBooks for destinations declared in Prepare
//
// Returns: List of CommandBooks to execute on other aggregates
type EventHandler func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error)

// PrepareHandler declares which destinations are needed for an event type.
// Parameters:
//   - source: The source EventBook
//   - event: The event Any from the EventPage
//
// Returns: List of Covers for destinations to fetch
type PrepareHandler func(source *pb.EventBook, event *anypb.Any) []*pb.Cover

// EventRouter dispatches events to handlers by type_url suffix.
// Unified router for sagas, process managers, and projectors.
// Uses fluent .Domain().On() pattern to register handlers with domain context.
//
// Example (Saga - single domain):
//
//	router := NewEventRouter("saga-table-hand").
//	    Domain("table").
//	    On("HandStarted", handleStarted)
//
// Example (Process Manager - multi-domain):
//
//	router := NewEventRouter("pmg-order-flow").
//	    Domain("order").
//	    On("OrderCreated", handleCreated).
//	    Domain("inventory").
//	    On("StockReserved", handleReserved)
//
// Example (Projector - multi-domain):
//
//	router := NewEventRouter("prj-output").
//	    Domain("player").
//	    On("PlayerRegistered", handleRegistered).
//	    Domain("hand").
//	    On("CardsDealt", handleDealt)
type EventRouter struct {
	name            string
	currentDomain   string
	handlers        map[string][]eventRegistration  // domain -> handlers
	prepareHandlers map[string][]prepareRegistration // domain -> prepare handlers
}

type eventRegistration struct {
	suffix  string
	handler EventHandler
}

type prepareRegistration struct {
	suffix  string
	handler PrepareHandler
}

// NewEventRouter creates a new router for the given component name.
// For single-domain routers, you can pass an optional inputDomain as the second argument
// (backwards compatibility). For multi-domain routers, use Domain() instead.
func NewEventRouter(name string, inputDomain ...string) *EventRouter {
	router := &EventRouter{
		name:            name,
		handlers:        make(map[string][]eventRegistration),
		prepareHandlers: make(map[string][]prepareRegistration),
	}
	// Backwards compatibility: if inputDomain provided, set it as current context
	if len(inputDomain) > 0 && inputDomain[0] != "" {
		router.Domain(inputDomain[0])
	}
	return router
}

// Domain sets the current domain context for subsequent On() calls.
func (r *EventRouter) Domain(name string) *EventRouter {
	r.currentDomain = name
	if _, ok := r.handlers[name]; !ok {
		r.handlers[name] = make([]eventRegistration, 0)
	}
	if _, ok := r.prepareHandlers[name]; !ok {
		r.prepareHandlers[name] = make([]prepareRegistration, 0)
	}
	return r
}

// Prepare registers a prepare handler for an event type_url suffix.
// The prepare handler declares which destinations are needed before Execute.
// Must be called after Domain() to set context.
func (r *EventRouter) Prepare(suffix string, handler PrepareHandler) *EventRouter {
	if r.currentDomain == "" {
		panic("Must call Domain() before Prepare()")
	}
	r.prepareHandlers[r.currentDomain] = append(
		r.prepareHandlers[r.currentDomain],
		prepareRegistration{suffix: suffix, handler: handler},
	)
	return r
}

// On registers a handler for an event type_url suffix in current domain.
// Must be called after Domain() to set context.
func (r *EventRouter) On(suffix string, handler EventHandler) *EventRouter {
	if r.currentDomain == "" {
		panic("Must call Domain() before On()")
	}
	r.handlers[r.currentDomain] = append(
		r.handlers[r.currentDomain],
		eventRegistration{suffix: suffix, handler: handler},
	)
	return r
}

// Subscriptions auto-derives subscriptions from registered handlers.
// Returns list of (domain, event_types) pairs.
func (r *EventRouter) Subscriptions() map[string][]string {
	result := make(map[string][]string)
	for domain, handlers := range r.handlers {
		if len(handlers) > 0 {
			types := make([]string, len(handlers))
			for i, reg := range handlers {
				types[i] = reg.suffix
			}
			result[domain] = types
		}
	}
	return result
}

// PrepareDestinations returns the destination covers needed for the given source.
// Routes based on source domain.
func (r *EventRouter) PrepareDestinations(source *pb.EventBook) []*pb.Cover {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	sourceDomain := ""
	if source.Cover != nil {
		sourceDomain = source.Cover.Domain
	}

	domainHandlers, ok := r.prepareHandlers[sourceDomain]
	if !ok {
		return nil
	}

	// Use the last event page
	page := source.Pages[len(source.Pages)-1]
	event := page.GetEvent()
	if event == nil {
		return nil
	}

	for _, reg := range domainHandlers {
		if strings.HasSuffix(event.TypeUrl, reg.suffix) {
			return reg.handler(source, event)
		}
	}
	return nil
}

// Dispatch routes all events in an EventBook to registered handlers.
// Routes based on source domain and event type suffix.
func (r *EventRouter) Dispatch(source *pb.EventBook, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
	if source == nil {
		return nil, nil
	}

	sourceDomain := ""
	if source.Cover != nil {
		sourceDomain = source.Cover.Domain
	}

	domainHandlers, ok := r.handlers[sourceDomain]
	if !ok {
		return nil, nil
	}

	var commands []*pb.CommandBook
	for _, page := range source.Pages {
		event := page.GetEvent()
		if event == nil {
			continue
		}
		for _, reg := range domainHandlers {
			if strings.HasSuffix(event.TypeUrl, reg.suffix) {
				cmds, err := reg.handler(source, event, destinations)
				if err != nil {
					return nil, err
				}
				commands = append(commands, cmds...)
				break
			}
		}
	}
	return commands, nil
}

// InputDomain returns the first registered domain (for backwards compatibility).
// Deprecated: Use Subscriptions() instead.
func (r *EventRouter) InputDomain() string {
	for domain := range r.handlers {
		return domain
	}
	return ""
}

// ============================================================================
// StateRouter - fluent state reconstruction
// ============================================================================

// StateFactory creates a new zero-value state instance.
type StateFactory[S any] func() S

// EventApplier applies an event to state.
// The handler receives raw bytes and is responsible for unmarshaling.
type EventApplier[S any] func(state *S, value []byte)

// stateRegistration holds a suffix and its handler.
type stateRegistration[S any] struct {
	suffix  string
	applier EventApplier[S]
}

// StateRouter provides fluent state reconstruction from events.
//
// Register once at startup, call WithEvents() per rebuild.
// Creates fresh state on each WithEvents() call.
//
// Example:
//
//	func applyRegistered(state *PlayerState, event *examples.PlayerRegistered) {
//	    state.PlayerID = "player_" + event.Email
//	    state.DisplayName = event.DisplayName
//	}
//
//	func applyDeposited(state *PlayerState, event *examples.FundsDeposited) {
//	    if event.NewBalance != nil {
//	        state.Bankroll = event.NewBalance.Amount
//	    }
//	}
//
//	var playerRouter = NewStateRouter(NewPlayerState).
//	    On(applyRegistered).
//	    On(applyDeposited)
//
//	func RebuildState(eventBook *pb.EventBook) PlayerState {
//	    return playerRouter.WithEvents(eventBook.Pages)
//	}
type StateRouter[S any] struct {
	factory  StateFactory[S]
	handlers []stateRegistration[S]
}

// NewStateRouter creates a new StateRouter with the given state factory.
//
// The factory is called on each WithEvents() to create a fresh state instance.
func NewStateRouter[S any](factory StateFactory[S]) *StateRouter[S] {
	return &StateRouter[S]{
		factory:  factory,
		handlers: make([]stateRegistration[S], 0),
	}
}

// On registers an event applier handler.
//
// The handler function must have signature: func(*S, *EventType)
// The event type is derived via reflection from the handler.
//
// Example:
//
//	router.On(applyRegistered)  // applyRegistered is func(*PlayerState, *PlayerRegistered)
func (r *StateRouter[S]) On(handler any) *StateRouter[S] {
	// Use reflection to extract proto type from handler signature
	suffix, applier := makeEventApplier[S](handler)
	r.handlers = append(r.handlers, stateRegistration[S]{
		suffix:  suffix,
		applier: applier,
	})
	return r
}

// WithEvents creates fresh state and applies all events.
//
// This is the terminal operation for rebuilding state.
func (r *StateRouter[S]) WithEvents(pages []*pb.EventPage) S {
	state := r.factory()
	for _, page := range pages {
		if event := page.GetEvent(); event != nil {
			r.ApplySingle(&state, event)
		}
	}
	return state
}

// WithEventBook creates fresh state from an EventBook.
func (r *StateRouter[S]) WithEventBook(eventBook *pb.EventBook) S {
	if eventBook == nil {
		return r.factory()
	}
	return r.WithEvents(eventBook.Pages)
}

// ApplySingle applies a single event to existing state.
func (r *StateRouter[S]) ApplySingle(state *S, eventAny *anypb.Any) {
	typeURL := eventAny.TypeUrl
	for _, reg := range r.handlers {
		if strings.HasSuffix(typeURL, reg.suffix) {
			reg.applier(state, eventAny.Value)
			return
		}
	}
	// Unknown event type - silently ignore (forward compatibility)
}

// ToRebuilder converts the StateRouter to a StateRebuilder function.
//
// This allows using StateRouter with CommandRouter:
//
//	playerRouter := NewStateRouter(NewPlayerState).On(...)
//	cmdRouter := NewCommandRouter("player", playerRouter.ToRebuilder())
func (r *StateRouter[S]) ToRebuilder() StateRebuilder[S] {
	return func(events *pb.EventBook) S {
		return r.WithEventBook(events)
	}
}

// makeEventApplier uses reflection to create an EventApplier from a typed handler.
//
// The handler must have signature: func(*S, *EventType) where EventType is a proto.Message.
// Returns the event type suffix and an applier function.
func makeEventApplier[S any](handler any) (string, EventApplier[S]) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 2 {
		panic("handler must have exactly 2 parameters (state *S, event *EventType)")
	}

	// Get the event type (second parameter)
	eventPtrType := handlerType.In(1)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	// Get the type name for suffix matching
	suffix := eventType.Name()

	// Create the applier function
	applier := func(state *S, value []byte) {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := proto.Unmarshal(value, event); err != nil {
			return // Silently ignore unmarshal errors
		}

		// Call the handler with state and event
		stateValue := reflect.ValueOf(state)
		handlerValue.Call([]reflect.Value{stateValue, eventPtr})
	}

	return suffix, applier
}
