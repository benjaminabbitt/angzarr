// Package angzarr provides OO-style command handler base for rich domain models.
//
// This module provides the framework for implementing event-sourced command handlers
// using the rich domain model pattern. Business logic lives as methods on the
// command handler struct, with registration methods for handlers:
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
//	    angzarr.CommandHandlerBase[PlayerState]
//	}
//
//	func NewPlayer(eventBook *pb.EventBook) *Player {
//	    p := &Player{}
//	    p.Init(eventBook, func() PlayerState { return PlayerState{} })
//	    p.Applies(p.applyRegistered)
//	    p.Applies(p.applyDeposited)
//	    p.Handles(p.register)
//	    p.Handles(p.deposit)
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

// rejectionHandlerFunc is an internal type for rejection handlers.
// Receives the Notification and returns a BusinessResponse for compensation.
type rejectionHandlerFunc func(notification *pb.Notification) *pb.BusinessResponse

// CommandHandlerBase provides OO-style command handler infrastructure.
//
// Embed this in your command handler struct and call Init() to set up the base.
// Then register handlers with Handles() and appliers with Applies().
type CommandHandlerBase[S any] struct {
	eventBook         *pb.EventBook
	state             *S
	stateSet          bool
	factory           func() S
	handlers          map[string]handlerFunc
	multiHandlers     map[string]multiHandlerFunc
	appliers          map[string]applierFunc[S]
	rejectionHandlers map[string]rejectionHandlerFunc
	domain            string
}

// Init initializes the command handler base with an event book and state factory.
//
// Call this in your command handler's constructor:
//
//	func NewPlayer(eventBook *pb.EventBook) *Player {
//	    p := &Player{}
//	    p.Init(eventBook, func() PlayerState { return PlayerState{} })
//	    // ... register handlers and appliers
//	    return p
//	}
func (a *CommandHandlerBase[S]) Init(eventBook *pb.EventBook, factory func() S) {
	if eventBook == nil {
		eventBook = &pb.EventBook{}
	}
	a.eventBook = eventBook
	a.factory = factory
	a.handlers = make(map[string]handlerFunc)
	a.multiHandlers = make(map[string]multiHandlerFunc)
	a.appliers = make(map[string]applierFunc[S])
	a.rejectionHandlers = make(map[string]rejectionHandlerFunc)
}

// SetDomain sets the command handler's domain name for descriptor generation.
func (a *CommandHandlerBase[S]) SetDomain(domain string) {
	a.domain = domain
}

// Domain returns the command handler's domain name.
func (a *CommandHandlerBase[S]) Domain() string {
	return a.domain
}

// Handles registers a command handler.
//
// The handler function must have signature: func(*CommandType) (proto.Message, error)
// where CommandType is a protobuf message type. The command type is automatically
// extracted via proto reflection.
//
// Example:
//
//	p.Handles(p.register)
//
//	func (p *Player) register(cmd *examples.RegisterPlayer) (proto.Message, error) {
//	    if p.Exists() {
//	        return nil, NewCommandRejectedError("Player already exists")
//	    }
//	    return &examples.PlayerRegistered{...}, nil
//	}
func (a *CommandHandlerBase[S]) Handles(handler any) {
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

	// Extract fully-qualified type name via proto reflection
	cmdPtr := reflect.New(cmdType)
	protoMsg := cmdPtr.Interface().(proto.Message)
	fullName := string(protoMsg.ProtoReflect().Descriptor().FullName())

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

	a.handlers[fullName] = wrapper
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
//	h.HandlesMulti(h.awardPot)
//
//	func (h *Hand) awardPot(cmd *examples.AwardPot) ([]proto.Message, error) {
//	    // ... validation ...
//	    return []proto.Message{potAwardedEvent, handCompleteEvent}, nil
//	}
func (a *CommandHandlerBase[S]) HandlesMulti(handler any) {
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

	// Extract fully-qualified type name via proto reflection
	cmdPtr := reflect.New(cmdType)
	protoMsg := cmdPtr.Interface().(proto.Message)
	fullName := string(protoMsg.ProtoReflect().Descriptor().FullName())

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

	a.multiHandlers[fullName] = wrapper
}

// HandlesRejection registers a rejection handler for compensation logic.
//
// The handler function must have signature: func(*pb.Notification) *pb.BusinessResponse
// The handler receives the rejection notification and returns either:
//   - EmitCompensationEvents() with events to undo the operation
//   - DelegateToFramework() to let the framework handle compensation
//
// Example:
//
//	p.HandlesRejection("payment", "ProcessPayment", p.handlePaymentRejected)
//
//	func (p *Player) handlePaymentRejected(notification *pb.Notification) *pb.BusinessResponse {
//	    ctx := angzarr.NewCompensationContext(notification)
//	    // Emit compensation event to release reserved funds
//	    event := &examples.FundsReleased{
//	        Amount: p.State().ReservedAmount,
//	        Reason: "Payment failed: " + ctx.RejectionReason,
//	    }
//	    p.applyAndRecord(event)
//	    return angzarr.EmitCompensationEvents(p.EventBook())
//	}
func (a *CommandHandlerBase[S]) HandlesRejection(domain, command string, handler func(*pb.Notification) *pb.BusinessResponse) {
	key := domain + "/" + command
	a.rejectionHandlers[key] = handler
}

// Applies registers an event applier.
//
// The applier function must have signature: func(*S, *EventType)
// where S is the state type and EventType is a protobuf message type.
// The event type is automatically extracted via proto reflection.
//
// Example:
//
//	p.Applies(p.applyRegistered)
//
//	func (p *Player) applyRegistered(state *PlayerState, event *examples.PlayerRegistered) {
//	    state.PlayerID = "player_" + event.Email
//	    state.DisplayName = event.DisplayName
//	}
func (a *CommandHandlerBase[S]) Applies(applier any) {
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

	// Extract fully-qualified type name via proto reflection
	eventPtr := reflect.New(eventType)
	protoMsg := eventPtr.Interface().(proto.Message)
	fullName := string(protoMsg.ProtoReflect().Descriptor().FullName())

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

	a.appliers[fullName] = wrapper
}

// State returns the current state, rebuilding from events if needed.
func (a *CommandHandlerBase[S]) State() *S {
	if !a.stateSet {
		a.rebuild()
	}
	return a.state
}

// Exists returns true if this command handler has prior events.
func (a *CommandHandlerBase[S]) Exists() bool {
	// Force state rebuild to check
	_ = a.State()
	return a.stateSet && (a.eventBook != nil && len(a.eventBook.Pages) > 0)
}

// EventBook returns the event book for persistence.
func (a *CommandHandlerBase[S]) EventBook() *pb.EventBook {
	return a.eventBook
}

// Dispatch routes a command to the matching handler.
//
// The command is unpacked from the Any, the matching handler is called,
// and the resulting event(s) are applied to state and recorded in the event book.
func (a *CommandHandlerBase[S]) Dispatch(cmdAny *anypb.Any) error {
	if cmdAny == nil || cmdAny.TypeUrl == "" {
		return fmt.Errorf("no command provided")
	}

	typeURL := cmdAny.TypeUrl

	// Check single-event handlers first
	for fullName, handler := range a.handlers {
		if typeURL == TypeURLPrefix+fullName {
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
	for fullName, handler := range a.multiHandlers {
		if typeURL == TypeURLPrefix+fullName {
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
func (a *CommandHandlerBase[S]) applyAndRecord(event proto.Message) {
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
func (a *CommandHandlerBase[S]) applyEvent(state *S, eventAny *anypb.Any) {
	typeURL := eventAny.TypeUrl
	for fullName, applier := range a.appliers {
		if typeURL == TypeURLPrefix+fullName {
			applier(state, eventAny.Value)
			return
		}
	}
	// Unknown event type - silently ignore (forward compatibility)
}

// rebuild reconstructs state from the event book, then clears consumed events.
//
// # Why Clear Events After Rebuild?
//
// The EventBook serves dual purposes:
// 1. **Input**: Prior events passed in from the framework (for state reconstruction)
// 2. **Output**: New events produced by command handlers (to persist)
//
// After rebuilding state from prior events, we clear the Pages slice so that
// only NEW events produced by this request are returned. Without clearing:
// - The response would contain both old and new events
// - The framework would try to re-persist events that already exist
// - Event sequences would be wrong (duplicates)
//
// This "consume then clear" pattern is essential for the OO command handler model
// where the same EventBook struct flows through input -> processing -> output.
func (a *CommandHandlerBase[S]) rebuild() {
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

// HandlerTypes returns the registered fully-qualified command type names.
func (a *CommandHandlerBase[S]) HandlerTypes() []string {
	types := make([]string, 0, len(a.handlers)+len(a.multiHandlers))
	for fullName := range a.handlers {
		types = append(types, fullName)
	}
	for fullName := range a.multiHandlers {
		types = append(types, fullName)
	}
	return types
}

// Handle processes a full gRPC request.
//
// This is the entry point for gRPC integration. It extracts the command,
// dispatches it, and returns the event book.
//
// Note: Unlike the functional router, OO command handlers need to be instantiated
// fresh for each request with the prior events. Use NewOOCommandHandlerGrpc
// to wrap OO command handlers for gRPC.
func (a *CommandHandlerBase[S]) Handle(request *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	if len(request.Command.Pages) == 0 {
		return nil, fmt.Errorf("%s", ErrMsgNoCommandPages)
	}

	cmdAny := request.Command.Pages[0].GetCommand()
	if cmdAny == nil || cmdAny.TypeUrl == "" {
		return nil, fmt.Errorf("%s", ErrMsgNoCommandPages)
	}

	// Check for Notification (rejection/compensation)
	if cmdAny.TypeUrl == TypeURLPrefix+"angzarr.Notification" {
		return a.dispatchRejection(cmdAny)
	}

	if err := a.Dispatch(cmdAny); err != nil {
		return nil, err
	}

	return &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Events{Events: a.eventBook},
	}, nil
}

// dispatchRejection routes a rejection notification to the matching handler.
//
// Handlers are registered via HandlesRejection() with domain/command key.
// If no handler matches, delegates to the framework for default compensation.
func (a *CommandHandlerBase[S]) dispatchRejection(cmdAny *anypb.Any) (*pb.BusinessResponse, error) {
	// Unmarshal the Notification
	notification := &pb.Notification{}
	if err := cmdAny.UnmarshalTo(notification); err != nil {
		return nil, fmt.Errorf("failed to unmarshal Notification: %w", err)
	}

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
	if handler, ok := a.rejectionHandlers[key]; ok {
		// Ensure state is rebuilt before calling handler
		_ = a.State()
		return handler(notification), nil
	}

	return DelegateToFramework(
		fmt.Sprintf("CommandHandler %s has no custom compensation for %s", a.domain, key),
	), nil
}
