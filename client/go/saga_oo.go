// Package angzarr provides OO-style saga base for event-driven command production.
//
// Sagas translate events from one domain into commands for another domain.
// They are stateless - each event is processed independently.
//
// Two-phase protocol support:
//   - Prepare: Declare destination aggregates needed (via Prepares)
//   - Execute: Produce commands given source + destination state (via ReactsTo)
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
//	    s.Prepares("HandStarted", s.prepareHandStarted)
//	    s.ReactsTo("HandStarted", s.handleHandStarted)
//	    return s
//	}
//
//	func (s *TableHandSaga) prepareHandStarted(event *table.HandStarted) []*pb.Cover {
//	    return []*pb.Cover{{
//	        Domain: "hand",
//	        Root:   &pb.UUID{Value: event.HandRoot},
//	    }}
//	}
//
//	func (s *TableHandSaga) handleHandStarted(
//	    event *table.HandStarted,
//	    dests []*pb.EventBook,
//	) (*pb.CommandBook, error) {
//	    destSeq := NextSequence(dests[0])
//	    // ... build DealCards command
//	    return &pb.CommandBook{...}, nil
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

// prepareOOFunc is an internal type for prepare handlers.
type prepareOOFunc func(eventAny *anypb.Any) []*pb.Cover

// handlerOOFunc is an internal type for event handlers.
type handlerOOFunc func(eventAny *anypb.Any, dests []*pb.EventBook) ([]*pb.CommandBook, error)

// SagaBase provides OO-style saga infrastructure.
//
// Embed this in your saga struct and call Init() to set up the base.
// Then register handlers with Prepares() and ReactsTo().
type SagaBase struct {
	name         string
	inputDomain  string
	outputDomain string
	prepares     map[string]prepareOOFunc
	handlers     map[string]handlerOOFunc
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
	s.prepares = make(map[string]prepareOOFunc)
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

// Prepares registers a prepare handler for an event type_url suffix.
//
// The handler function must have signature: func(*EventType) []*pb.Cover
// where EventType is a protobuf message type.
//
// Example:
//
//	s.Prepares("HandStarted", s.prepareHandStarted)
//
//	func (s *TableHandSaga) prepareHandStarted(event *table.HandStarted) []*pb.Cover {
//	    return []*pb.Cover{{Domain: "hand", Root: &pb.UUID{Value: event.HandRoot}}}
//	}
func (s *SagaBase) Prepares(suffix string, handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 1 {
		panic("handler must have exactly 1 parameter (event *EventType)")
	}
	if handlerType.NumOut() != 1 {
		panic("handler must return []*pb.Cover")
	}

	// Get the event type (first parameter)
	eventPtrType := handlerType.In(0)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	// Create the wrapper function
	wrapper := func(eventAny *anypb.Any) []*pb.Cover {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := eventAny.UnmarshalTo(event); err != nil {
			return nil
		}

		// Call the handler
		results := handlerValue.Call([]reflect.Value{eventPtr})

		// Extract result
		if results[0].IsNil() {
			return nil
		}
		return results[0].Interface().([]*pb.Cover)
	}

	s.prepares[suffix] = wrapper
}

// ReactsTo registers an event handler for a type_url suffix.
//
// The handler function can have two signatures:
//
//  1. Without destinations: func(*EventType) (*pb.CommandBook, error)
//  2. With destinations: func(*EventType, []*pb.EventBook) (*pb.CommandBook, error)
//
// Example:
//
//	s.ReactsTo("HandStarted", s.handleHandStarted)
//
//	func (s *TableHandSaga) handleHandStarted(
//	    event *table.HandStarted,
//	    dests []*pb.EventBook,
//	) (*pb.CommandBook, error) {
//	    destSeq := NextSequence(dests[0])
//	    // ... build command
//	    return &pb.CommandBook{...}, nil
//	}
func (s *SagaBase) ReactsTo(suffix string, handler any) {
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

	s.handlers[suffix] = wrapper
}

// ReactsToMulti registers an event handler that returns multiple commands.
//
// The handler function must have signature:
// func(*EventType, []*pb.EventBook) ([]*pb.CommandBook, error)
//
// Use this for broadcasting to multiple aggregates.
func (s *SagaBase) ReactsToMulti(suffix string, handler any) {
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

	s.handlers[suffix] = wrapper
}

// PrepareDestinations returns the destination covers needed for the given source.
// Called during the Prepare phase of the two-phase saga protocol.
func (s *SagaBase) PrepareDestinations(source *pb.EventBook) []*pb.Cover {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	var covers []*pb.Cover
	for _, page := range source.Pages {
		if page.Event == nil {
			continue
		}

		typeURL := page.Event.TypeUrl
		for suffix, handler := range s.prepares {
			if strings.HasSuffix(typeURL, suffix) {
				result := handler(page.Event)
				covers = append(covers, result...)
				break
			}
		}
	}
	return covers
}

// Execute processes events and returns commands for other aggregates.
// Called during the Execute phase of the two-phase saga protocol.
func (s *SagaBase) Execute(source *pb.EventBook, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
	if source == nil || len(source.Pages) == 0 {
		return nil, nil
	}

	var commands []*pb.CommandBook
	for _, page := range source.Pages {
		if page.Event == nil {
			continue
		}

		typeURL := page.Event.TypeUrl
		for suffix, handler := range s.handlers {
			if strings.HasSuffix(typeURL, suffix) {
				cmds, err := handler(page.Event, destinations)
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

// HandlerTypes returns the registered event type suffixes.
func (s *SagaBase) HandlerTypes() []string {
	types := make([]string, 0, len(s.handlers))
	for suffix := range s.handlers {
		types = append(types, suffix)
	}
	return types
}

// Descriptor builds a ComponentDescriptor from registered handlers.
func (s *SagaBase) Descriptor() *pb.ComponentDescriptor {
	return &pb.ComponentDescriptor{
		Name:          s.name,
		ComponentType: ComponentSaga,
		Inputs: []*pb.Target{
			{
				Domain: s.inputDomain,
				Types:  s.HandlerTypes(),
			},
		},
	}
}
