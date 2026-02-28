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
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
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
	fullName string
	handler  CommandHandler[S]
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

// On registers a handler for a command type by fully-qualified name.
//
// The fullName should be the proto full name (e.g., "examples.RegisterPlayer").
// For type-safe registration using generics, use OnType instead.
func (r *CommandRouter[S]) On(fullName string, handler CommandHandler[S]) *CommandRouter[S] {
	r.handlers = append(r.handlers, commandRegistration[S]{fullName: fullName, handler: handler})
	return r
}

// OnType registers a handler for a command type using proto reflection.
//
// Example:
//
//	router.OnType[*examples.RegisterPlayer](handleRegisterPlayer)
func OnType[T proto.Message, S any](r *CommandRouter[S], handler CommandHandler[S]) *CommandRouter[S] {
	var zero T
	fullName := string(zero.ProtoReflect().Descriptor().FullName())
	r.handlers = append(r.handlers, commandRegistration[S]{fullName: fullName, handler: handler})
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
	if typeURL == TypeURLPrefix+"angzarr.Notification" {
		notification := &pb.Notification{}
		if err := commandAny.UnmarshalTo(notification); err != nil {
			return nil, fmt.Errorf("failed to unmarshal Notification: %w", err)
		}
		return r.dispatchRejection(notification, state)
	}

	// Normal command dispatch - exact type URL matching
	for _, reg := range r.handlers {
		if typeURL == TypeURLPrefix+reg.fullName {
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
//
// # Why Route by Domain + Command Type
//
// Compensation handlers are registered per (domain, command) pair because:
// 1. Different target domains may need different compensation logic
// 2. Different command types may require different rollback strategies
//
// Example: An order aggregate might handle "fulfillment/CreateShipment" rejection
// differently from "payment/ProcessPayment" rejection.
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
		fmt.Sprintf("CommandHandler %s has no custom compensation for %s", r.domain, key),
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
	handlers        map[string][]eventRegistration   // domain -> handlers
	prepareHandlers map[string][]prepareRegistration // domain -> prepare handlers
}

type eventRegistration struct {
	fullName string
	handler  EventHandler
}

type prepareRegistration struct {
	fullName string
	handler  PrepareHandler
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

// Prepare registers a prepare handler for an event type by fully-qualified name.
// The prepare handler declares which destinations are needed before Execute.
// Must be called after Domain() to set context.
//
// The fullName should be the proto full name (e.g., "examples.HandStarted").
func (r *EventRouter) Prepare(fullName string, handler PrepareHandler) *EventRouter {
	if r.currentDomain == "" {
		panic("Must call Domain() before Prepare()")
	}
	r.prepareHandlers[r.currentDomain] = append(
		r.prepareHandlers[r.currentDomain],
		prepareRegistration{fullName: fullName, handler: handler},
	)
	return r
}

// On registers a handler for an event type by fully-qualified name in current domain.
// Must be called after Domain() to set context.
//
// The fullName should be the proto full name (e.g., "examples.HandStarted").
// For type-safe registration using generics, use OnEvent instead.
func (r *EventRouter) On(fullName string, handler EventHandler) *EventRouter {
	if r.currentDomain == "" {
		panic("Must call Domain() before On()")
	}
	r.handlers[r.currentDomain] = append(
		r.handlers[r.currentDomain],
		eventRegistration{fullName: fullName, handler: handler},
	)
	return r
}

// OnEvent registers a handler for an event type using proto reflection.
// Must be called after Domain() to set context.
//
// Example:
//
//	OnEvent[*examples.HandStarted](router, handleStarted)
func OnEvent[T proto.Message](r *EventRouter, handler EventHandler) *EventRouter {
	if r.currentDomain == "" {
		panic("Must call Domain() before OnEvent()")
	}
	var zero T
	fullName := string(zero.ProtoReflect().Descriptor().FullName())
	r.handlers[r.currentDomain] = append(
		r.handlers[r.currentDomain],
		eventRegistration{fullName: fullName, handler: handler},
	)
	return r
}

// PrepareEvent registers a prepare handler for an event type using proto reflection.
// Must be called after Domain() to set context.
//
// Example:
//
//	PrepareEvent[*examples.HandStarted](router, prepareStarted)
func PrepareEvent[T proto.Message](r *EventRouter, handler PrepareHandler) *EventRouter {
	if r.currentDomain == "" {
		panic("Must call Domain() before PrepareEvent()")
	}
	var zero T
	fullName := string(zero.ProtoReflect().Descriptor().FullName())
	r.prepareHandlers[r.currentDomain] = append(
		r.prepareHandlers[r.currentDomain],
		prepareRegistration{fullName: fullName, handler: handler},
	)
	return r
}

// Subscriptions auto-derives subscriptions from registered handlers.
// Returns list of (domain, event_types) pairs with fully-qualified type names.
func (r *EventRouter) Subscriptions() map[string][]string {
	result := make(map[string][]string)
	for domain, handlers := range r.handlers {
		if len(handlers) > 0 {
			types := make([]string, len(handlers))
			for i, reg := range handlers {
				types[i] = reg.fullName
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
		if event.TypeUrl == TypeURLPrefix+reg.fullName {
			return reg.handler(source, event)
		}
	}
	return nil
}

// Dispatch routes all events in an EventBook to registered handlers.
// Routes based on source domain and event type (exact match).
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
			if event.TypeUrl == TypeURLPrefix+reg.fullName {
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

// ============================================================================
// StateRouter - fluent state reconstruction
// ============================================================================

// StateFactory creates a new zero-value state instance.
type StateFactory[S any] func() S

// EventApplier applies an event to state.
// The handler receives raw bytes and is responsible for unmarshaling.
type EventApplier[S any] func(state *S, value []byte)

// stateRegistration holds a fully-qualified type name and its handler.
type stateRegistration[S any] struct {
	fullName string
	applier  EventApplier[S]
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
	fullName, applier := makeEventApplier[S](handler)
	r.handlers = append(r.handlers, stateRegistration[S]{
		fullName: fullName,
		applier:  applier,
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
//
// Unknown event types are silently ignored for forward compatibility:
// When a new event type is added, old code that hasn't been updated
// can still process the EventBook without crashing. The new event
// won't affect state until the code is updated with a handler.
func (r *StateRouter[S]) ApplySingle(state *S, eventAny *anypb.Any) {
	typeURL := eventAny.TypeUrl
	for _, reg := range r.handlers {
		if typeURL == TypeURLPrefix+reg.fullName {
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
// Returns the fully-qualified type name and an applier function.
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

	// Create a zero instance to get the full proto name via reflection
	eventPtr := reflect.New(eventType)
	protoMsg := eventPtr.Interface().(proto.Message)
	fullName := string(protoMsg.ProtoReflect().Descriptor().FullName())

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

	return fullName, applier
}

// ============================================================================
// Unified Routers (Trait-Based)
// ============================================================================
//
// These routers wrap handler interfaces (traits) for a cleaner separation
// of concerns. They are an alternative to the fluent CommandRouter/EventRouter
// pattern above.
//
// Two router patterns based on domain cardinality:
//
//   - CommandHandlerRouter/SagaRouter: For single-domain components (domain set at construction)
//   - ProcessManagerRouter/ProjectorRouter: For multi-domain components (fluent .Domain() pattern)

// ============================================================================
// CommandHandlerRouter -- Single Domain
// ============================================================================

// CommandHandlerRouter wraps a CommandHandlerDomainHandler for routing commands.
//
// Domain is set at construction time. No Domain() method exists,
// enforcing single-domain constraint.
type CommandHandlerRouter[S any] struct {
	name    string
	domain  string
	handler CommandHandlerDomainHandler[S]
}

// NewCommandHandlerRouter creates a new command handler router.
//
// Command handlers handle commands and emit events. Single domain enforced at construction.
func NewCommandHandlerRouter[S any](name, domain string, handler CommandHandlerDomainHandler[S]) *CommandHandlerRouter[S] {
	return &CommandHandlerRouter[S]{
		name:    name,
		domain:  domain,
		handler: handler,
	}
}

// Name returns the router name.
func (r *CommandHandlerRouter[S]) Name() string {
	return r.name
}

// Domain returns the domain.
func (r *CommandHandlerRouter[S]) Domain() string {
	return r.domain
}

// CommandTypes returns command types from the handler.
func (r *CommandHandlerRouter[S]) CommandTypes() []string {
	return r.handler.CommandTypes()
}

// Subscriptions returns subscriptions for this command handler.
// Returns a map of domain -> command types.
func (r *CommandHandlerRouter[S]) Subscriptions() map[string][]string {
	return map[string][]string{
		r.domain: r.handler.CommandTypes(),
	}
}

// RebuildState rebuilds state from events using the handler.
func (r *CommandHandlerRouter[S]) RebuildState(events *pb.EventBook) S {
	return r.handler.Rebuild(events)
}

// Dispatch routes a contextual command to the handler.
func (r *CommandHandlerRouter[S]) Dispatch(cmd *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	commandBook := cmd.GetCommand()
	if commandBook == nil {
		return nil, status.Error(codes.InvalidArgument, "missing command book")
	}

	if len(commandBook.Pages) == 0 {
		return nil, status.Error(codes.InvalidArgument, "no command pages")
	}

	commandPage := commandBook.Pages[0]
	commandAny := commandPage.GetCommand()
	if commandAny == nil {
		return nil, status.Error(codes.InvalidArgument, "missing command")
	}

	eventBook := cmd.GetEvents()
	if eventBook == nil {
		eventBook = &pb.EventBook{}
	}

	// Rebuild state
	state := r.handler.Rebuild(eventBook)
	seq := NextSequence(eventBook)

	typeURL := commandAny.TypeUrl

	// Check for Notification (rejection/compensation)
	if strings.HasSuffix(typeURL, "Notification") {
		return r.dispatchCHNotification(commandAny, state)
	}

	// Execute handler
	resultBook, err := r.handler.Handle(commandBook, commandAny, state, seq)
	if err != nil {
		return nil, err
	}

	return &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Events{Events: resultBook},
	}, nil
}

// dispatchCHNotification routes a Notification to the command handler's rejection handler.
func (r *CommandHandlerRouter[S]) dispatchCHNotification(commandAny *anypb.Any, state S) (*pb.BusinessResponse, error) {
	notification := &pb.Notification{}
	if err := proto.Unmarshal(commandAny.Value, notification); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to decode Notification: %v", err)
	}

	rejection := &pb.RejectionNotification{}
	if notification.Payload != nil {
		if err := proto.Unmarshal(notification.Payload.Value, rejection); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to decode RejectionNotification: %v", err)
		}
	}

	domain, cmdSuffix := extractRejectionKey(rejection)

	response, err := r.handler.OnRejected(notification, state, domain, cmdSuffix)
	if err != nil {
		return nil, err
	}

	if response == nil {
		response = &RejectionHandlerResponse{}
	}

	switch {
	case response.Events != nil:
		return &pb.BusinessResponse{
			Result: &pb.BusinessResponse_Events{Events: response.Events},
		}, nil
	case response.Notification != nil:
		return &pb.BusinessResponse{
			Result: &pb.BusinessResponse_Notification{Notification: response.Notification},
		}, nil
	default:
		return &pb.BusinessResponse{
			Result: &pb.BusinessResponse_Revocation{Revocation: &pb.RevocationResponse{
				EmitSystemRevocation:  true,
				SendToDeadLetterQueue: false,
				Escalate:              false,
				Abort:                 false,
				Reason:                fmt.Sprintf("Handler returned empty response for %s/%s", domain, cmdSuffix),
			}},
		}, nil
	}
}

// ============================================================================
// SagaRouter -- Single Domain
// ============================================================================

// SagaRouter wraps a SagaDomainHandler for routing events.
//
// Domain is set at construction time. No Domain() method exists,
// enforcing single-domain constraint.
type SagaRouter struct {
	name    string
	domain  string
	handler SagaDomainHandler
}

// NewSagaRouter creates a new saga router.
//
// Sagas translate events from one domain to commands for another.
// Single domain enforced at construction.
func NewSagaRouter(name, domain string, handler SagaDomainHandler) *SagaRouter {
	return &SagaRouter{
		name:    name,
		domain:  domain,
		handler: handler,
	}
}

// Name returns the router name.
func (r *SagaRouter) Name() string {
	return r.name
}

// InputDomain returns the input domain.
func (r *SagaRouter) InputDomain() string {
	return r.domain
}

// EventTypes returns event types from the handler.
func (r *SagaRouter) EventTypes() []string {
	return r.handler.EventTypes()
}

// Subscriptions returns subscriptions for this saga.
// Returns a map of domain -> event types.
func (r *SagaRouter) Subscriptions() map[string][]string {
	return map[string][]string{
		r.domain: r.handler.EventTypes(),
	}
}

// PrepareDestinations returns destinations needed for the given source events.
func (r *SagaRouter) PrepareDestinations(source *pb.EventBook) []*pb.Cover {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	eventPage := source.Pages[len(source.Pages)-1]
	eventAny := eventPage.GetEvent()
	if eventAny == nil {
		return nil
	}

	return r.handler.Prepare(source, eventAny)
}

// Dispatch routes an event to the saga handler.
func (r *SagaRouter) Dispatch(source *pb.EventBook, destinations []*pb.EventBook) (*pb.SagaResponse, error) {
	if source == nil || len(source.Pages) == 0 {
		return nil, status.Error(codes.InvalidArgument, "source event book has no events")
	}

	eventPage := source.Pages[len(source.Pages)-1]
	eventAny := eventPage.GetEvent()
	if eventAny == nil {
		return nil, status.Error(codes.InvalidArgument, "missing event payload")
	}

	// Check for Notification (rejection/compensation)
	if strings.HasSuffix(eventAny.TypeUrl, "Notification") {
		return r.dispatchSagaNotification(eventAny)
	}

	response, err := r.handler.Execute(source, eventAny, destinations)
	if err != nil {
		return nil, err
	}

	if response == nil {
		response = &SagaHandlerResponse{}
	}

	return &pb.SagaResponse{
		Commands: response.Commands,
		Events:   response.Events,
	}, nil
}

// dispatchSagaNotification routes a Notification to the saga's rejection handler.
func (r *SagaRouter) dispatchSagaNotification(eventAny *anypb.Any) (*pb.SagaResponse, error) {
	notification := &pb.Notification{}
	if err := proto.Unmarshal(eventAny.Value, notification); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to decode Notification: %v", err)
	}

	rejection := &pb.RejectionNotification{}
	if notification.Payload != nil {
		if err := proto.Unmarshal(notification.Payload.Value, rejection); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to decode RejectionNotification: %v", err)
		}
	}

	domain, cmdSuffix := extractRejectionKey(rejection)

	response, err := r.handler.OnRejected(notification, domain, cmdSuffix)
	if err != nil {
		return nil, err
	}

	// Sagas can only return events for compensation (no commands on rejection)
	var events []*pb.EventBook
	if response != nil && response.Events != nil {
		events = []*pb.EventBook{response.Events}
	}

	return &pb.SagaResponse{
		Commands: nil,
		Events:   events,
	}, nil
}

// ============================================================================
// ProcessManagerRouter -- Multi-Domain
// ============================================================================

// ProcessManagerRouter wraps multiple ProcessManagerDomainHandlers for routing events.
//
// Domains are registered via fluent Domain() calls.
type ProcessManagerRouter[S any] struct {
	name     string
	pmDomain string
	rebuild  func(*pb.EventBook) S
	domains  map[string]ProcessManagerDomainHandler[S]
}

// NewProcessManagerRouter creates a new process manager router.
//
// Process managers correlate events across multiple domains and maintain
// their own state. The pmDomain is used for storing PM state.
func NewProcessManagerRouter[S any](name, pmDomain string, rebuild func(*pb.EventBook) S) *ProcessManagerRouter[S] {
	return &ProcessManagerRouter[S]{
		name:     name,
		pmDomain: pmDomain,
		rebuild:  rebuild,
		domains:  make(map[string]ProcessManagerDomainHandler[S]),
	}
}

// Domain registers a domain handler.
//
// Process managers can have multiple input domains.
func (r *ProcessManagerRouter[S]) Domain(name string, handler ProcessManagerDomainHandler[S]) *ProcessManagerRouter[S] {
	r.domains[name] = handler
	return r
}

// Name returns the router name.
func (r *ProcessManagerRouter[S]) Name() string {
	return r.name
}

// PMDomain returns the PM's own domain (for state storage).
func (r *ProcessManagerRouter[S]) PMDomain() string {
	return r.pmDomain
}

// Subscriptions returns subscriptions (domain + event types) for this PM.
func (r *ProcessManagerRouter[S]) Subscriptions() map[string][]string {
	result := make(map[string][]string)
	for domain, handler := range r.domains {
		result[domain] = handler.EventTypes()
	}
	return result
}

// RebuildState rebuilds PM state from events.
func (r *ProcessManagerRouter[S]) RebuildState(events *pb.EventBook) S {
	return r.rebuild(events)
}

// PrepareDestinations returns destinations needed for the given trigger and process state.
func (r *ProcessManagerRouter[S]) PrepareDestinations(trigger, processState *pb.EventBook) []*pb.Cover {
	if trigger == nil || len(trigger.Pages) == 0 {
		return nil
	}

	triggerDomain := ""
	if trigger.Cover != nil {
		triggerDomain = trigger.Cover.Domain
	}

	eventPage := trigger.Pages[len(trigger.Pages)-1]
	eventAny := eventPage.GetEvent()
	if eventAny == nil {
		return nil
	}

	var state S
	if processState != nil {
		state = r.rebuild(processState)
	} else {
		state = r.rebuild(&pb.EventBook{})
	}

	handler, ok := r.domains[triggerDomain]
	if !ok {
		return nil
	}

	return handler.Prepare(trigger, state, eventAny)
}

// Dispatch routes a trigger event to the appropriate handler.
func (r *ProcessManagerRouter[S]) Dispatch(
	trigger, processState *pb.EventBook,
	destinations []*pb.EventBook,
) (*pb.ProcessManagerHandleResponse, error) {
	if trigger == nil || len(trigger.Pages) == 0 {
		return nil, status.Error(codes.InvalidArgument, "trigger event book has no events")
	}

	triggerDomain := ""
	if trigger.Cover != nil {
		triggerDomain = trigger.Cover.Domain
	}

	handler, ok := r.domains[triggerDomain]
	if !ok {
		return nil, status.Errorf(codes.Unimplemented, "no handler for domain: %s", triggerDomain)
	}

	eventPage := trigger.Pages[len(trigger.Pages)-1]
	eventAny := eventPage.GetEvent()
	if eventAny == nil {
		return nil, status.Error(codes.InvalidArgument, "missing event payload")
	}

	state := r.rebuild(processState)

	// Check for Notification
	if strings.HasSuffix(eventAny.TypeUrl, "Notification") {
		return r.dispatchPMNotification(handler, eventAny, state)
	}

	response, err := handler.Handle(trigger, state, eventAny, destinations)
	if err != nil {
		return nil, err
	}

	if response == nil {
		response = &ProcessManagerResponse{}
	}

	return &pb.ProcessManagerHandleResponse{
		Commands:      response.Commands,
		ProcessEvents: response.ProcessEvents,
		// TODO: Add response.Facts once Go proto is regenerated
		// Facts:         response.Facts,
	}, nil
}

// dispatchPMNotification routes a Notification to the PM's rejection handler.
func (r *ProcessManagerRouter[S]) dispatchPMNotification(
	handler ProcessManagerDomainHandler[S],
	eventAny *anypb.Any,
	state S,
) (*pb.ProcessManagerHandleResponse, error) {
	notification := &pb.Notification{}
	if err := proto.Unmarshal(eventAny.Value, notification); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to decode Notification: %v", err)
	}

	rejection := &pb.RejectionNotification{}
	if notification.Payload != nil {
		if err := proto.Unmarshal(notification.Payload.Value, rejection); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to decode RejectionNotification: %v", err)
		}
	}

	domain, cmdSuffix := extractRejectionKey(rejection)

	response, err := handler.OnRejected(notification, state, domain, cmdSuffix)
	if err != nil {
		return nil, err
	}

	var events *pb.EventBook
	if response != nil {
		events = response.Events
	}

	return &pb.ProcessManagerHandleResponse{
		Commands:      nil,
		ProcessEvents: events,
	}, nil
}

// ============================================================================
// ProjectorRouter -- Multi-Domain
// ============================================================================

// ProjectorRouter wraps multiple ProjectorDomainHandlers for routing events.
//
// Domains are registered via fluent Domain() calls.
type ProjectorRouter struct {
	name    string
	domains map[string]ProjectorDomainHandler
}

// NewProjectorRouter creates a new projector router.
//
// Projectors consume events from multiple domains and produce external output.
func NewProjectorRouter(name string) *ProjectorRouter {
	return &ProjectorRouter{
		name:    name,
		domains: make(map[string]ProjectorDomainHandler),
	}
}

// Domain registers a domain handler.
//
// Projectors can have multiple input domains.
func (r *ProjectorRouter) Domain(name string, handler ProjectorDomainHandler) *ProjectorRouter {
	r.domains[name] = handler
	return r
}

// Name returns the router name.
func (r *ProjectorRouter) Name() string {
	return r.name
}

// Subscriptions returns subscriptions (domain + event types) for this projector.
func (r *ProjectorRouter) Subscriptions() map[string][]string {
	result := make(map[string][]string)
	for domain, handler := range r.domains {
		result[domain] = handler.EventTypes()
	}
	return result
}

// Dispatch routes events to the appropriate handler.
func (r *ProjectorRouter) Dispatch(events *pb.EventBook) (*pb.Projection, error) {
	if events == nil || events.Cover == nil {
		return nil, status.Error(codes.InvalidArgument, "missing event book cover")
	}

	domain := events.Cover.Domain

	handler, ok := r.domains[domain]
	if !ok {
		return nil, status.Errorf(codes.Unimplemented, "no handler for domain: %s", domain)
	}

	return handler.Project(events)
}

// Note: extractRejectionKey is defined in pm_oo.go
