// Package angzarr provides OO-style process manager base for multi-domain orchestration.
//
// Process managers correlate events across multiple domains, managing state machines
// that span domain boundaries. Unlike sagas (stateless), PMs maintain state.
//
// Two-phase protocol support:
//   - Prepare: Declare destination aggregates needed (via Prepares)
//   - Handle: Process events given trigger + state + destinations (via Handles)
//
// State reconstruction:
//   - Applies: Rebuild PM state from its own EventBook
//
// Example usage:
//
//	type HandFlowPM struct {
//	    angzarr.ProcessManagerBase[*PMState]
//	}
//
//	func NewHandFlowPM() *HandFlowPM {
//	    pm := &HandFlowPM{}
//	    pm.Init("hand-flow", "hand-flow", []string{"table", "hand"})
//	    pm.Applies("ProcessStarted", pm.applyProcessStarted)
//	    pm.Prepares("HandStarted", pm.prepareHandStarted)
//	    pm.Handles("HandStarted", pm.handleHandStarted)
//	    return pm
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

// pmPrepareOOFunc is an internal type for prepare handlers.
// Returns covers for destinations needed by this event.
type pmPrepareOOFunc[S any] func(trigger *pb.EventBook, state S, eventAny *anypb.Any) []*pb.Cover

// pmHandlerOOFunc is an internal type for event handlers.
// Returns commands and optionally PM events.
type pmHandlerOOFunc[S any] func(trigger *pb.EventBook, state S, eventAny *anypb.Any, dests []*pb.EventBook) ([]*pb.CommandBook, *pb.EventBook, error)

// pmApplierOOFunc is an internal type for state appliers.
// Mutates state based on a PM event.
type pmApplierOOFunc[S any] func(state S, eventAny *anypb.Any)

// pmRejectionOOFunc is an internal type for rejection handlers.
// Returns RejectionHandlerResponse with events and/or notification.
type pmRejectionOOFunc[S any] func(state S, notification *pb.Notification) *RejectionHandlerResponse

// ProcessManagerBase provides OO-style process manager infrastructure.
//
// Embed this in your PM struct and call Init() to set up the base.
// Then register handlers with Prepares(), Handles(), and Applies().
//
// Type parameter S is the PM state type (should be a pointer type like *PMState).
type ProcessManagerBase[S any] struct {
	name         string
	pmDomain     string
	inputDomains []string
	stateFactory func() S
	prepares     map[string]pmPrepareOOFunc[S]
	handlers     map[string]pmHandlerOOFunc[S]
	appliers     map[string]pmApplierOOFunc[S]
	rejections   map[string]pmRejectionOOFunc[S]
}

// Init initializes the process manager base with name and domain configuration.
//
// Call this in your PM's constructor:
//
//	func NewHandFlowPM() *HandFlowPM {
//	    pm := &HandFlowPM{}
//	    pm.Init("hand-flow", "hand-flow", []string{"table", "hand"})
//	    // ... register handlers
//	    return pm
//	}
func (pm *ProcessManagerBase[S]) Init(name, pmDomain string, inputDomains []string) {
	pm.name = name
	pm.pmDomain = pmDomain
	pm.inputDomains = inputDomains
	pm.prepares = make(map[string]pmPrepareOOFunc[S])
	pm.handlers = make(map[string]pmHandlerOOFunc[S])
	pm.appliers = make(map[string]pmApplierOOFunc[S])
	pm.rejections = make(map[string]pmRejectionOOFunc[S])
}

// WithStateFactory sets the factory function for creating new state instances.
// Required for state reconstruction from events.
func (pm *ProcessManagerBase[S]) WithStateFactory(factory func() S) {
	pm.stateFactory = factory
}

// Name returns the PM's name.
func (pm *ProcessManagerBase[S]) Name() string {
	return pm.name
}

// PMDomain returns the PM's own domain (for its EventBook).
func (pm *ProcessManagerBase[S]) PMDomain() string {
	return pm.pmDomain
}

// InputDomains returns the domains this PM subscribes to.
func (pm *ProcessManagerBase[S]) InputDomains() []string {
	return pm.inputDomains
}

// Prepares registers a prepare handler for an event type_url suffix.
//
// The handler function must have signature:
// func(trigger *pb.EventBook, state S, event *EventType) []*pb.Cover
//
// Example:
//
//	pm.Prepares("HandStarted", pm.prepareHandStarted)
//
//	func (pm *HandFlowPM) prepareHandStarted(
//	    trigger *pb.EventBook,
//	    state *PMState,
//	    event *examples.HandStarted,
//	) []*pb.Cover {
//	    return []*pb.Cover{{Domain: "hand", Root: &pb.UUID{Value: event.HandRoot}}}
//	}
func (pm *ProcessManagerBase[S]) Prepares(suffix string, handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 3 {
		panic("handler must have 3 parameters (trigger *pb.EventBook, state S, event *EventType)")
	}
	if handlerType.NumOut() != 1 {
		panic("handler must return []*pb.Cover")
	}

	// Get the event type (third parameter)
	eventPtrType := handlerType.In(2)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	// Create the wrapper function
	wrapper := func(trigger *pb.EventBook, state S, eventAny *anypb.Any) []*pb.Cover {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := eventAny.UnmarshalTo(event); err != nil {
			return nil
		}

		// Call the handler
		triggerValue := reflect.ValueOf(trigger)
		stateValue := reflect.ValueOf(state)
		results := handlerValue.Call([]reflect.Value{triggerValue, stateValue, eventPtr})

		// Extract result
		if results[0].IsNil() {
			return nil
		}
		return results[0].Interface().([]*pb.Cover)
	}

	pm.prepares[suffix] = wrapper
}

// Handles registers an event handler for a type_url suffix.
//
// The handler function can have two signatures:
//
//  1. Without destinations: func(trigger, state, event) (cmds, pmEvents, error)
//  2. With destinations: func(trigger, state, event, dests) (cmds, pmEvents, error)
//
// Example:
//
//	pm.Handles("HandStarted", pm.handleHandStarted)
//
//	func (pm *HandFlowPM) handleHandStarted(
//	    trigger *pb.EventBook,
//	    state *PMState,
//	    event *examples.HandStarted,
//	    dests []*pb.EventBook,
//	) ([]*pb.CommandBook, *pb.EventBook, error) {
//	    // Process event and return commands
//	    return cmds, nil, nil
//	}
func (pm *ProcessManagerBase[S]) Handles(suffix string, handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}

	numIn := handlerType.NumIn()
	if numIn < 3 || numIn > 4 {
		panic("handler must have 3-4 parameters (trigger, state, event [, dests])")
	}
	if handlerType.NumOut() != 3 {
		panic("handler must return ([]*pb.CommandBook, *pb.EventBook, error)")
	}

	// Get the event type (third parameter)
	eventPtrType := handlerType.In(2)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	withDests := numIn == 4

	// Create the wrapper function
	wrapper := func(trigger *pb.EventBook, state S, eventAny *anypb.Any, dests []*pb.EventBook) ([]*pb.CommandBook, *pb.EventBook, error) {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := eventAny.UnmarshalTo(event); err != nil {
			return nil, nil, fmt.Errorf("failed to unmarshal event: %w", err)
		}

		// Build arguments
		triggerValue := reflect.ValueOf(trigger)
		stateValue := reflect.ValueOf(state)

		var results []reflect.Value
		if withDests {
			destsValue := reflect.ValueOf(dests)
			results = handlerValue.Call([]reflect.Value{triggerValue, stateValue, eventPtr, destsValue})
		} else {
			results = handlerValue.Call([]reflect.Value{triggerValue, stateValue, eventPtr})
		}

		// Extract results
		var cmds []*pb.CommandBook
		if !results[0].IsNil() {
			cmds = results[0].Interface().([]*pb.CommandBook)
		}

		var pmEvents *pb.EventBook
		if !results[1].IsNil() {
			pmEvents = results[1].Interface().(*pb.EventBook)
		}

		var err error
		if !results[2].IsNil() {
			err = results[2].Interface().(error)
		}

		return cmds, pmEvents, err
	}

	pm.handlers[suffix] = wrapper
}

// Applies registers a state applier for a PM event type_url suffix.
//
// The handler function must have signature:
// func(state S, event *EventType)
//
// State is mutated in place.
//
// Example:
//
//	pm.Applies("ProcessStarted", pm.applyProcessStarted)
//
//	func (pm *HandFlowPM) applyProcessStarted(state *PMState, event *ProcessStarted) {
//	    state.HandRoot = event.HandRoot
//	    state.InProgress = true
//	}
func (pm *ProcessManagerBase[S]) Applies(suffix string, handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 2 {
		panic("handler must have 2 parameters (state S, event *EventType)")
	}
	if handlerType.NumOut() != 0 {
		panic("handler must not return anything (mutates state in place)")
	}

	// Get the event type (second parameter)
	eventPtrType := handlerType.In(1)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	// Create the wrapper function
	wrapper := func(state S, eventAny *anypb.Any) {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := eventAny.UnmarshalTo(event); err != nil {
			return
		}

		// Call the handler
		stateValue := reflect.ValueOf(state)
		handlerValue.Call([]reflect.Value{stateValue, eventPtr})
	}

	pm.appliers[suffix] = wrapper
}

// OnRejected registers a rejection handler for when a specific command is rejected.
//
// Called when a PM-issued command targeting the specified domain and command
// type is rejected by the target aggregate.
//
// The handler function must have signature:
// func(state S, notification *pb.Notification) *RejectionHandlerResponse
//
// Example:
//
//	pm.OnRejected("table", "JoinTable", pm.handleJoinRejected)
//
//	func (pm *HandFlowPM) handleJoinRejected(
//	    state *PMState,
//	    notification *pb.Notification,
//	) *RejectionHandlerResponse {
//	    return &angzarr.RejectionHandlerResponse{
//	        Events: compensationEvents,
//	        Notification: upstreamNotification,
//	    }
//	}
func (pm *ProcessManagerBase[S]) OnRejected(domain, command string, handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 2 {
		panic("handler must have 2 parameters (state S, notification *pb.Notification)")
	}
	if handlerType.NumOut() != 1 {
		panic("handler must return *RejectionHandlerResponse")
	}

	// Create the wrapper function
	wrapper := func(state S, notification *pb.Notification) *RejectionHandlerResponse {
		stateValue := reflect.ValueOf(state)
		notificationValue := reflect.ValueOf(notification)
		results := handlerValue.Call([]reflect.Value{stateValue, notificationValue})

		// Extract result
		if results[0].IsNil() {
			return nil
		}
		return results[0].Interface().(*RejectionHandlerResponse)
	}

	key := fmt.Sprintf("%s/%s", domain, command)
	pm.rejections[key] = wrapper
}

// RebuildState reconstructs PM state from the process EventBook.
func (pm *ProcessManagerBase[S]) RebuildState(processState *pb.EventBook) S {
	var state S
	if pm.stateFactory != nil {
		state = pm.stateFactory()
	} else {
		// Use zero value if no factory
		var zero S
		state = zero
	}

	if processState == nil || len(processState.Pages) == 0 {
		return state
	}

	for _, page := range processState.Pages {
		event := page.GetEvent()
		if event == nil {
			continue
		}

		typeURL := event.TypeUrl
		for suffix, applier := range pm.appliers {
			if strings.HasSuffix(typeURL, suffix) {
				applier(state, event)
				break
			}
		}
	}

	return state
}

// PrepareDestinations returns the destination covers needed for the given trigger.
// Called during the Prepare phase of the two-phase PM protocol.
func (pm *ProcessManagerBase[S]) PrepareDestinations(trigger, processState *pb.EventBook) []*pb.Cover {
	if trigger == nil || len(trigger.Pages) == 0 {
		return nil
	}

	state := pm.RebuildState(processState)

	var covers []*pb.Cover
	for _, page := range trigger.Pages {
		event := page.GetEvent()
		if event == nil {
			continue
		}

		typeURL := event.TypeUrl
		for suffix, handler := range pm.prepares {
			if strings.HasSuffix(typeURL, suffix) {
				result := handler(trigger, state, event)
				covers = append(covers, result...)
				break
			}
		}
	}
	return covers
}

// Handle processes events and returns commands, PM events, and notification.
// Called during the Handle phase of the two-phase PM protocol.
//
// Detects Notification (rejection) payloads and routes to rejection handlers.
func (pm *ProcessManagerBase[S]) Handle(trigger, processState *pb.EventBook, destinations []*pb.EventBook) ([]*pb.CommandBook, *pb.EventBook, *pb.Notification, error) {
	if trigger == nil || len(trigger.Pages) == 0 {
		return nil, nil, nil, nil
	}

	state := pm.RebuildState(processState)

	var commands []*pb.CommandBook
	var allPMEvents []*pb.EventPage
	var notification *pb.Notification

	for _, page := range trigger.Pages {
		event := page.GetEvent()
		if event == nil {
			continue
		}

		typeURL := event.TypeUrl

		// Check for Notification (rejection/compensation)
		if strings.HasSuffix(typeURL, "Notification") {
			resp := pm.handleNotification(state, event)
			if resp != nil {
				if resp.Events != nil {
					allPMEvents = append(allPMEvents, resp.Events.Pages...)
				}
				if resp.Notification != nil {
					notification = resp.Notification
				}
			}
			continue
		}

		for suffix, handler := range pm.handlers {
			if strings.HasSuffix(typeURL, suffix) {
				cmds, pmEvents, err := handler(trigger, state, event, destinations)
				if err != nil {
					return nil, nil, nil, err
				}
				commands = append(commands, cmds...)
				if pmEvents != nil {
					allPMEvents = append(allPMEvents, pmEvents.Pages...)
				}
				break
			}
		}
	}

	var resultPMEvents *pb.EventBook
	if len(allPMEvents) > 0 {
		resultPMEvents = &pb.EventBook{
			Pages: allPMEvents,
		}
	}

	return commands, resultPMEvents, notification, nil
}

// handleNotification routes a Notification to the appropriate rejection handler.
func (pm *ProcessManagerBase[S]) handleNotification(state S, eventAny *anypb.Any) *RejectionHandlerResponse {
	// Decode the Notification
	var notification pb.Notification
	if err := eventAny.UnmarshalTo(&notification); err != nil {
		return nil
	}

	// Unpack rejection details from payload
	var rejection pb.RejectionNotification
	if notification.Payload != nil {
		if err := proto.Unmarshal(notification.Payload.Value, &rejection); err != nil {
			return nil
		}
	}

	// Extract domain and command type from rejected_command
	domain, cmdSuffix := extractRejectionKey(&rejection)
	key := fmt.Sprintf("%s/%s", domain, cmdSuffix)

	// Call handler if found
	if handler, ok := pm.rejections[key]; ok {
		return handler(state, &notification)
	}

	// Default: no handler
	return nil
}

// extractRejectionKey extracts domain and command suffix from a RejectionNotification.
func extractRejectionKey(rejection *pb.RejectionNotification) (string, string) {
	if rejection.RejectedCommand == nil {
		return "", ""
	}

	domain := ""
	if rejection.RejectedCommand.Cover != nil {
		domain = rejection.RejectedCommand.Cover.Domain
	}

	cmdSuffix := ""
	if len(rejection.RejectedCommand.Pages) > 0 {
		page := rejection.RejectedCommand.Pages[0]
		if cmd := page.GetCommand(); cmd != nil {
			typeURL := cmd.TypeUrl
			// Extract suffix (last part after /)
			if idx := strings.LastIndex(typeURL, "/"); idx >= 0 {
				cmdSuffix = typeURL[idx+1:]
			} else {
				cmdSuffix = typeURL
			}
		}
	}

	return domain, cmdSuffix
}

// HandlerTypes returns the registered event type suffixes for handlers.
func (pm *ProcessManagerBase[S]) HandlerTypes() []string {
	types := make([]string, 0, len(pm.handlers))
	for suffix := range pm.handlers {
		types = append(types, suffix)
	}
	return types
}
