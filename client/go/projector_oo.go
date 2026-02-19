// Package angzarr provides OO-style projector base for event projection.
//
// Projectors subscribe to events from one or more domains and produce
// side effects (logging, database writes, etc.) without emitting commands.
//
// Example usage:
//
//	type OutputProjector struct {
//	    angzarr.ProjectorBase
//	}
//
//	func NewOutputProjector() *OutputProjector {
//	    p := &OutputProjector{}
//	    p.Init("output", []string{"player", "table", "hand"})
//	    p.Projects("PlayerRegistered", p.projectRegistered)
//	    p.Projects("TableCreated", p.projectTableCreated)
//	    return p
//	}
//
//	func (p *OutputProjector) projectRegistered(event *examples.PlayerRegistered) *pb.Projection {
//	    writeLog(fmt.Sprintf("Player registered: %s", event.DisplayName))
//	    return nil // Let base handle default projection
//	}
package angzarr

import (
	"reflect"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
)

// projectorOOFunc is an internal type for projection handlers.
type projectorOOFunc func(data []byte) *pb.Projection

// ProjectorBase provides OO-style projector infrastructure.
//
// Embed this in your projector struct and call Init() to set up the base.
// Then register handlers with Projects().
type ProjectorBase struct {
	name     string
	domains  []string
	handlers map[string]projectorOOFunc
}

// Init initializes the projector base with name and domain configuration.
//
// Call this in your projector's constructor:
//
//	func NewOutputProjector() *OutputProjector {
//	    p := &OutputProjector{}
//	    p.Init("output", []string{"player", "table", "hand"})
//	    // ... register handlers
//	    return p
//	}
func (p *ProjectorBase) Init(name string, domains []string) {
	p.name = name
	p.domains = domains
	p.handlers = make(map[string]projectorOOFunc)
}

// Name returns the projector's name.
func (p *ProjectorBase) Name() string {
	return p.name
}

// Domains returns the domains this projector subscribes to.
func (p *ProjectorBase) Domains() []string {
	return p.domains
}

// Projects registers an event projection handler for a type_url suffix.
//
// The handler function must have signature: func(*EventType) *pb.Projection
// where EventType is a protobuf message type.
// The handler may return nil to use the default projection.
//
// Example:
//
//	p.Projects("PlayerRegistered", p.projectRegistered)
//
//	func (p *OutputProjector) projectRegistered(event *examples.PlayerRegistered) *pb.Projection {
//	    writeLog(fmt.Sprintf("Player: %s", event.DisplayName))
//	    return nil
//	}
func (p *ProjectorBase) Projects(suffix string, handler any) {
	handlerValue := reflect.ValueOf(handler)
	handlerType := handlerValue.Type()

	if handlerType.Kind() != reflect.Func {
		panic("handler must be a function")
	}
	if handlerType.NumIn() != 1 {
		panic("handler must have exactly 1 parameter (event *EventType)")
	}
	if handlerType.NumOut() != 1 {
		panic("handler must return *pb.Projection")
	}

	// Get the event type (first parameter)
	eventPtrType := handlerType.In(0)
	if eventPtrType.Kind() != reflect.Ptr {
		panic("event parameter must be a pointer")
	}
	eventType := eventPtrType.Elem()

	// Create the wrapper function
	wrapper := func(data []byte) *pb.Projection {
		// Create a new instance of the event type
		eventPtr := reflect.New(eventType)
		event := eventPtr.Interface().(proto.Message)

		// Unmarshal the event
		if err := proto.Unmarshal(data, event); err != nil {
			return nil
		}

		// Call the handler
		results := handlerValue.Call([]reflect.Value{eventPtr})

		// Extract result
		if results[0].IsNil() {
			return nil
		}
		return results[0].Interface().(*pb.Projection)
	}

	p.handlers[suffix] = wrapper
}

// Handle processes an EventBook and returns a Projection.
func (p *ProjectorBase) Handle(events *pb.EventBook) (*pb.Projection, error) {
	if events == nil || events.Cover == nil {
		return &pb.Projection{Projector: p.name}, nil
	}

	var lastSeq uint32

	for _, page := range events.Pages {
		event := page.GetEvent()
		if event == nil {
			continue
		}

		// Extract sequence
		lastSeq = page.Sequence

		typeURL := event.TypeUrl

		// Find handler by suffix
		for suffix, handler := range p.handlers {
			if strings.HasSuffix(typeURL, suffix) {
				if projection := handler(event.Value); projection != nil {
					return projection, nil
				}
				break
			}
		}
	}

	// Default projection
	return &pb.Projection{
		Cover:     events.Cover,
		Projector: p.name,
		Sequence:  lastSeq,
	}, nil
}

// RunOOProjectorServer runs a gRPC projector server using the OO projector.
func RunOOProjectorServer(name, port string, projector *ProjectorBase) {
	handler := NewProjectorHandler(name, projector.domains...).
		WithHandle(projector.Handle)
	RunProjectorServer(name, port, handler)
}
