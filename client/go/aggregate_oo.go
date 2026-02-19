// Package angzarr provides OO-style aggregate base for rich domain models.
//
// This module provides the framework for implementing event-sourced aggregates
// using the rich domain model pattern. Business logic lives as methods on the
// aggregate struct, with registration methods for handlers:
//
//   - Handles: Register command handlers that emit events
//   - Applies: Register event appliers that mutate state
//
// Example usage:
//
//	type PlayerState struct {
//	    PlayerID    string
//	    DisplayName string
//	    Bankroll    int64
//	}
//
//	type Player struct {
//	    angzarr.AggregateBase[PlayerState]
//	}
//
//	func NewPlayer(eventBook *pb.EventBook) *Player {
//	    p := &Player{}
//	    p.Init(eventBook, func() PlayerState { return PlayerState{} })
//	    p.Applies("PlayerRegistered", p.applyRegistered)
//	    p.Applies("FundsDeposited", p.applyDeposited)
//	    p.Handles("RegisterPlayer", p.register)
//	    p.Handles("DepositFunds", p.deposit)
//	    return p
//	}
//
//	func (p *Player) applyRegistered(state *PlayerState, event *examples.PlayerRegistered) {
//	    state.PlayerID = "player_" + event.Email
//	    state.DisplayName = event.DisplayName
//	}
//
//	func (p *Player) register(cmd *examples.RegisterPlayer) (proto.Message, error) {
//	    if p.Exists() {
//	        return nil, NewCommandRejectedError("Player already exists")
//	    }
//	    return &examples.PlayerRegistered{
//	        DisplayName: cmd.DisplayName,
//	        Email:       cmd.Email,
//	    }, nil
//	}
package angzarr

import (
	"fmt"
	"reflect"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// applierFunc is an internal type for event appliers.
type applierFunc[S any] func(state *S, value []byte)

// handlerFunc is an internal type for command handlers.
type handlerFunc func(cmd *anypb.Any) (proto.Message, error)

// multiHandlerFunc is an internal type for multi-event command handlers.
type multiHandlerFunc func(cmd *anypb.Any) ([]proto.Message, error)

// AggregateBase provides OO-style aggregate infrastructure.
//
// Embed this in your aggregate struct and call Init() to set up the base.
// Then register handlers with Handles() and appliers with Applies().
type AggregateBase[S any] struct {
	eventBook     *pb.EventBook
	state         *S
	stateSet      bool
	factory       func() S
	handlers      map[string]handlerFunc
	multiHandlers map[string]multiHandlerFunc
	appliers      map[string]applierFunc[S]
	domain        string
}

// Init initializes the aggregate base with an event book and state factory.
//
// Call this in your aggregate's constructor:
//
//	func NewPlayer(eventBook *pb.EventBook) *Player {
//	    p := &Player{}
//	    p.Init(eventBook, func() PlayerState { return PlayerState{} })
//	    // ... register handlers and appliers
//	    return p
//	}
func (a *AggregateBase[S]) Init(eventBook *pb.EventBook, factory func() S) {
	if eventBook == nil {
		eventBook = &pb.EventBook{}
	}
	a.eventBook = eventBook
	a.factory = factory
	a.handlers = make(map[string]handlerFunc)
	a.multiHandlers = make(map[string]multiHandlerFunc)
	a.appliers = make(map[string]applierFunc[S])
}

// SetDomain sets the aggregate's domain name for descriptor generation.
func (a *AggregateBase[S]) SetDomain(domain string) {
	a.domain = domain
}

// Domain returns the aggregate's domain name.
func (a *AggregateBase[S]) Domain() string {
	return a.domain
}

// Handles registers a command handler for a type_url suffix.
//
// The handler function must have signature: func(*CommandType) (proto.Message, error)
// where CommandType is a protobuf message type.
//
// Example:
//
//	p.Handles("RegisterPlayer", p.register)
//
//	func (p *Player) register(cmd *examples.RegisterPlayer) (proto.Message, error) {
//	    if p.Exists() {
//	        return nil, NewCommandRejectedError("Player already exists")
//	    }
//	    return &examples.PlayerRegistered{...}, nil
//	}
func (a *AggregateBase[S]) Handles(suffix string, handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 1 {
		panic("handler must have exactly 1 parameter (cmd *CommandType)")
	}
	if handlerType.NumOut() != 2 {
		panic("handler must return (proto.Message, error)")
	}

	// Get the command type (first parameter)
	cmdPtrType := handlerType.In(0)
	if cmdPtrType.Kind() != reflect.Ptr {
		panic("command parameter must be a pointer")
	}
	cmdType := cmdPtrType.Elem()

	// Create the wrapper function
	wrapper := func(cmdAny *anypb.Any) (proto.Message, error) {
		// Create a new instance of the command type
		cmdPtr := reflect.New(cmdType)
		cmd := cmdPtr.Interface().(proto.Message)

		// Unmarshal the command
		if err := cmdAny.UnmarshalTo(cmd); err != nil {
			return nil, fmt.Errorf("failed to unmarshal command: %w", err)
		}

		// Call the handler
		results := handlerValue.Call([]reflect.Value{cmdPtr})

		// Extract results
		var event proto.Message
		if !results[0].IsNil() {
			event = results[0].Interface().(proto.Message)
		}

		var err error
		if !results[1].IsNil() {
			err = results[1].Interface().(error)
		}

		return event, err
	}

	a.handlers[suffix] = wrapper
}

// HandlesMulti registers a command handler that returns multiple events.
//
// The handler function must have signature: func(*CommandType) ([]proto.Message, error)
// where CommandType is a protobuf message type.
//
// Use this for commands that produce multiple events, like AwardPot which
// emits both PotAwarded and HandComplete.
//
// Example:
//
//	h.HandlesMulti("AwardPot", h.awardPot)
//
//	func (h *Hand) awardPot(cmd *examples.AwardPot) ([]proto.Message, error) {
//	    // ... validation ...
//	    return []proto.Message{potAwardedEvent, handCompleteEvent}, nil
//	}
func (a *AggregateBase[S]) HandlesMulti(suffix string, handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 1 {
		panic("handler must have exactly 1 parameter (cmd *CommandType)")
	}
	if handlerType.NumOut() != 2 {
		panic("handler must return ([]proto.Message, error)")
	}

	// Get the command type (first parameter)
	cmdPtrType := handlerType.In(0)
	if cmdPtrType.Kind() != reflect.Ptr {
		panic("command parameter must be a pointer")
	}
	cmdType := cmdPtrType.Elem()

	// Create the wrapper function
	wrapper := func(cmdAny *anypb.Any) ([]proto.Message, error) {
		// Create a new instance of the command type
		cmdPtr := reflect.New(cmdType)
		cmd := cmdPtr.Interface().(proto.Message)

		// Unmarshal the command
		if err := cmdAny.UnmarshalTo(cmd); err != nil {
			return nil, fmt.Errorf("failed to unmarshal command: %w", err)
		}

		// Call the handler
		results := handlerValue.Call([]reflect.Value{cmdPtr})

		// Extract results
		var events []proto.Message
		if !results[0].IsNil() {
			// The handler returns []proto.Message or a slice of concrete types
			slice := results[0]
			events = make([]proto.Message, slice.Len())
			for i := 0; i < slice.Len(); i++ {
				events[i] = slice.Index(i).Interface().(proto.Message)
			}
		}

		var err error
		if !results[1].IsNil() {
			err = results[1].Interface().(error)
		}

		return events, err
	}

	a.multiHandlers[suffix] = wrapper
}

// Applies registers an event applier for a type_url suffix.
//
// The applier function must have signature: func(*S, *EventType)
// where S is the state type and EventType is a protobuf message type.
//
// Example:
//
//	p.Applies("PlayerRegistered", p.applyRegistered)
//
//	func (p *Player) applyRegistered(state *PlayerState, event *examples.PlayerRegistered) {
//	    state.PlayerID = "player_" + event.Email
//	    state.DisplayName = event.DisplayName
//	}
func (a *AggregateBase[S]) Applies(suffix string, applier any) {
	applierValue := reflect.ValueOf(applier)
	applierType := applierValue.Type()

	if applierType.Kind() != reflect.Func {
		panic("applier must be a function")
	}
	if applierType.NumIn() != 2 {
		panic("applier must have exactly 2 parameters (state *S, event *EventType)")
	}

	// Get the event type (second parameter)
	eventPtrType := applierType.In(1)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	// Create the wrapper function
	wrapper := func(state *S, value []byte) {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := proto.Unmarshal(value, event); err != nil {
			return // Silently ignore unmarshal errors
		}

		// Call the applier with state and event
		stateValue := reflect.ValueOf(state)
		applierValue.Call([]reflect.Value{stateValue, eventPtr})
	}

	a.appliers[suffix] = wrapper
}

// State returns the current state, rebuilding from events if needed.
func (a *AggregateBase[S]) State() *S {
	if !a.stateSet {
		a.rebuild()
	}
	return a.state
}

// Exists returns true if this aggregate has prior events.
func (a *AggregateBase[S]) Exists() bool {
	// Force state rebuild to check
	_ = a.State()
	return a.stateSet && (a.eventBook != nil && len(a.eventBook.Pages) > 0)
}

// EventBook returns the event book for persistence.
func (a *AggregateBase[S]) EventBook() *pb.EventBook {
	return a.eventBook
}

// Dispatch routes a command to the matching handler.
//
// The command is unpacked from the Any, the matching handler is called,
// and the resulting event(s) are applied to state and recorded in the event book.
func (a *AggregateBase[S]) Dispatch(cmdAny *anypb.Any) error {
	if cmdAny == nil || cmdAny.TypeUrl == "" {
		return fmt.Errorf("no command provided")
	}

	typeURL := cmdAny.TypeUrl

	// Check single-event handlers first
	for suffix, handler := range a.handlers {
		if strings.HasSuffix(typeURL, suffix) {
			// Ensure state is built before calling handler
			_ = a.State()

			event, err := handler(cmdAny)
			if err != nil {
				return err
			}

			if event != nil {
				a.applyAndRecord(event)
			}
			return nil
		}
	}

	// Check multi-event handlers
	for suffix, handler := range a.multiHandlers {
		if strings.HasSuffix(typeURL, suffix) {
			// Ensure state is built before calling handler
			_ = a.State()

			events, err := handler(cmdAny)
			if err != nil {
				return err
			}

			for _, event := range events {
				if event != nil {
					a.applyAndRecord(event)
				}
			}
			return nil
		}
	}

	return fmt.Errorf("%s: %s", ErrMsgUnknownCommand, typeURL)
}

// applyAndRecord packs the event, applies it to state, and adds it to the event book.
func (a *AggregateBase[S]) applyAndRecord(event proto.Message) {
	eventAny, err := anypb.New(event)
	if err != nil {
		return
	}

	// Apply to cached state
	if a.state != nil {
		a.applyEvent(a.state, eventAny)
	}

	// Record in event book
	page := &pb.EventPage{Payload: &pb.EventPage_Event{Event: eventAny}}
	a.eventBook.Pages = append(a.eventBook.Pages, page)
}

// applyEvent applies a single event to state using registered appliers.
func (a *AggregateBase[S]) applyEvent(state *S, eventAny *anypb.Any) {
	typeURL := eventAny.TypeUrl
	for suffix, applier := range a.appliers {
		if strings.HasSuffix(typeURL, suffix) {
			applier(state, eventAny.Value)
			return
		}
	}
	// Unknown event type - silently ignore (forward compatibility)
}

// rebuild reconstructs state from the event book, then clears consumed events.
func (a *AggregateBase[S]) rebuild() {
	state := a.factory()
	a.state = &state
	a.stateSet = true

	if a.eventBook == nil {
		return
	}

	// Apply all events
	for _, page := range a.eventBook.Pages {
		if event := page.GetEvent(); event != nil {
			a.applyEvent(a.state, event)
		}
	}

	// Clear consumed events - only new events will be in the book
	a.eventBook.Pages = nil
}

// HandlerTypes returns the registered command type suffixes.
func (a *AggregateBase[S]) HandlerTypes() []string {
	types := make([]string, 0, len(a.handlers)+len(a.multiHandlers))
	for suffix := range a.handlers {
		types = append(types, suffix)
	}
	for suffix := range a.multiHandlers {
		types = append(types, suffix)
	}
	return types
}

// Handle processes a full gRPC request.
//
// This is the entry point for gRPC integration. It extracts the command,
// dispatches it, and returns the event book.
//
// Note: Unlike the functional router, OO aggregates need to be instantiated
// fresh for each request with the prior events. Use NewOOAggregateHandler
// to wrap OO aggregates for gRPC.
func (a *AggregateBase[S]) Handle(request *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	if len(request.Command.Pages) == 0 {
		return nil, fmt.Errorf("%s", ErrMsgNoCommandPages)
	}

	cmdAny := request.Command.Pages[0].GetCommand()
	if cmdAny == nil || cmdAny.TypeUrl == "" {
		return nil, fmt.Errorf("%s", ErrMsgNoCommandPages)
	}

	// Check for Notification (rejection/compensation)
	if strings.HasSuffix(cmdAny.TypeUrl, "Notification") {
		// TODO: Implement revocation handling
		return DelegateToFramework("OO aggregate revocation not yet implemented"), nil
	}

	if err := a.Dispatch(cmdAny); err != nil {
		return nil, err
	}

	return &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Events{Events: a.eventBook},
	}, nil
}
