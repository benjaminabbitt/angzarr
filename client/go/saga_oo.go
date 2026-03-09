// Package angzarr provides OO-style saga base for event-driven command production.
//
// Sagas translate events from one domain into commands for another domain.
// They are stateless translators - each event is processed independently.
// The framework handles destination state fetching and sequence stamping.
//
// Example usage:
//
//	type TableHandSaga struct {
//	    angzarr.SagaBase
//	}
//
//	func NewTableHandSaga() *TableHandSaga {
//	    s := &TableHandSaga{}
//	    s.Init("saga-table-hand", "table", "hand")
//	    s.Handles(s.handleHandStarted)
//	    return s
//	}
//
//	func (s *TableHandSaga) handleHandStarted(
//	    event *table.HandStarted,
//	) (*pb.CommandBook, error) {
//	    // ... build DealCards command
//	    return &pb.CommandBook{...}, nil
//	}
package angzarr

import (
	"fmt"
	"reflect"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// handlerOOFunc is an internal type for event handlers.
type handlerOOFunc func(eventAny *anypb.Any, dests []*pb.EventBook) ([]*pb.CommandBook, error)

// SagaBase provides OO-style saga infrastructure.
//
// Embed this in your saga struct and call Init() to set up the base.
// Then register handlers with Handles().
type SagaBase struct {
	name         string
	inputDomain  string
	outputDomain string
	handlers     map[string]handlerOOFunc
	events       []*pb.EventBook // Accumulated events for EmitFact
}

// Init initializes the saga base with name and domain configuration.
//
// Call this in your saga's constructor:
//
//	func NewTableHandSaga() *TableHandSaga {
//	    s := &TableHandSaga{}
//	    s.Init("saga-table-hand", "table", "hand")
//	    // ... register handlers
//	    return s
//	}
func (s *SagaBase) Init(name, inputDomain, outputDomain string) {
	s.name = name
	s.inputDomain = inputDomain
	s.outputDomain = outputDomain
	s.handlers = make(map[string]handlerOOFunc)
}

// Name returns the saga's name.
func (s *SagaBase) Name() string {
	return s.name
}

// InputDomain returns the domain this saga listens to.
func (s *SagaBase) InputDomain() string {
	return s.inputDomain
}

// OutputDomain returns the domain this saga sends commands to.
func (s *SagaBase) OutputDomain() string {
	return s.outputDomain
}

// Prepares registers a prepare handler for an event type.
//
// The handler function must have signature: func(*EventType) []*pb.Cover
// where EventType is a protobuf message type. The event type is automatically
// extracted via proto reflection - no type name string needed.
//
// Handles registers an event handler.
//
// The handler function signature: func(*EventType) (*pb.CommandBook, error)
//
// The event type is automatically extracted via proto reflection.
//
// Example:
//
//	s.Handles(s.handleHandStarted)
//
//	func (s *TableHandSaga) handleHandStarted(
//	    event *table.HandStarted,
//	) (*pb.CommandBook, error) {
//	    // ... build command
//	    return &pb.CommandBook{...}, nil
//	}
func (s *SagaBase) Handles(handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}

	numIn := handlerType.NumIn()
	if numIn < 1 || numIn > 2 {
		panic("handler must have 1-2 parameters (event *EventType [, dests []*pb.EventBook])")
	}
	if handlerType.NumOut() != 2 {
		panic("handler must return (*pb.CommandBook, error)")
	}

	// Get the event type (first parameter)
	eventPtrType := handlerType.In(0)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	// Extract fully-qualified type name via proto reflection
	eventPtr := reflect.New(eventType)
	protoMsg := eventPtr.Interface().(proto.Message)
	fullName := string(protoMsg.ProtoReflect().Descriptor().FullName())

	withDests := numIn == 2

	// Create the wrapper function
	wrapper := func(eventAny *anypb.Any, dests []*pb.EventBook) ([]*pb.CommandBook, error) {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := eventAny.UnmarshalTo(event); err != nil {
			return nil, fmt.Errorf("failed to unmarshal event: %w", err)
		}

		// Call the handler
		var results []reflect.Value
		if withDests {
			destsValue := reflect.ValueOf(dests)
			results = handlerValue.Call([]reflect.Value{eventPtr, destsValue})
		} else {
			results = handlerValue.Call([]reflect.Value{eventPtr})
		}

		// Extract results
		var cmdBook *pb.CommandBook
		if !results[0].IsNil() {
			cmdBook = results[0].Interface().(*pb.CommandBook)
		}

		var err error
		if !results[1].IsNil() {
			err = results[1].Interface().(error)
		}

		if cmdBook != nil {
			return []*pb.CommandBook{cmdBook}, err
		}
		return nil, err
	}

	s.handlers[fullName] = wrapper
}

// HandlesMulti registers an event handler that returns multiple commands.
//
// The handler function must have signature:
// func(*EventType, []*pb.EventBook) ([]*pb.CommandBook, error)
//
// The event type is automatically extracted via proto reflection.
// Use this for broadcasting to multiple aggregates.
func (s *SagaBase) HandlesMulti(handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 2 {
		panic("handler must have 2 parameters (event *EventType, dests []*pb.EventBook)")
	}
	if handlerType.NumOut() != 2 {
		panic("handler must return ([]*pb.CommandBook, error)")
	}

	// Get the event type (first parameter)
	eventPtrType := handlerType.In(0)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	// Extract fully-qualified type name via proto reflection
	eventPtr := reflect.New(eventType)
	protoMsg := eventPtr.Interface().(proto.Message)
	fullName := string(protoMsg.ProtoReflect().Descriptor().FullName())

	// Create the wrapper function
	wrapper := func(eventAny *anypb.Any, dests []*pb.EventBook) ([]*pb.CommandBook, error) {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := eventAny.UnmarshalTo(event); err != nil {
			return nil, fmt.Errorf("failed to unmarshal event: %w", err)
		}

		// Call the handler
		destsValue := reflect.ValueOf(dests)
		results := handlerValue.Call([]reflect.Value{eventPtr, destsValue})

		// Extract results
		var cmdBooks []*pb.CommandBook
		if !results[0].IsNil() {
			cmdBooks = results[0].Interface().([]*pb.CommandBook)
		}

		var err error
		if !results[1].IsNil() {
			err = results[1].Interface().(error)
		}

		return cmdBooks, err
	}

	s.handlers[fullName] = wrapper
}

// EmitFact queues an EventBook to be emitted as a fact.
//
// Facts are events injected directly into target aggregates, bypassing
// command validation. Use for cross-aggregate coordination where the
// aggregate must accept the fact (e.g., "hand says it's your turn").
//
// Call this during handler execution. The events will be included
// in the SagaHandlerResponse.
func (s *SagaBase) EmitFact(event *pb.EventBook) {
	s.events = append(s.events, event)
}

// ClearEvents resets the accumulated events. Called before each Handle.
func (s *SagaBase) ClearEvents() {
	s.events = nil
}

// Handle processes source events and returns commands and facts for other aggregates.
// Sagas are stateless translators - they receive source events only.
func (s *SagaBase) Handle(source *pb.EventBook) (*SagaHandlerResponse, error) {
	if source == nil || len(source.Pages) == 0 {
		return &SagaHandlerResponse{}, nil
	}

	// Clear accumulated events from any prior execution
	s.ClearEvents()

	var commands []*pb.CommandBook
	for _, page := range source.Pages {
		event := page.GetEvent()
		if event == nil {
			continue
		}

		typeURL := event.TypeUrl
		for fullName, handler := range s.handlers {
			if typeURL == TypeURLPrefix+fullName {
				cmds, err := handler(event, nil)
				if err != nil {
					return nil, err
				}
				commands = append(commands, cmds...)
				break
			}
		}
	}
	return &SagaHandlerResponse{
		Commands: commands,
		Events:   s.events,
	}, nil
}

// HandlerTypes returns the registered fully-qualified event type names.
func (s *SagaBase) HandlerTypes() []string {
	types := make([]string, 0, len(s.handlers))
	for fullName := range s.handlers {
		types = append(types, fullName)
	}
	return types
}
