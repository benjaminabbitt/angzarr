// Package angzarr provides DRY dispatch via router types.
//
// CommandRouter replaces manual switch statements in aggregate handlers.
// EventRouter replaces manual switch statements in saga event handlers.
// Both auto-derive descriptors from their On() registrations.
package angzarr

import (
	"fmt"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
)

// Component type constants for Descriptor.
const (
	ComponentAggregate      = "aggregate"
	ComponentSaga           = "saga"
	ComponentProjector      = "projector"
	ComponentProcessManager = "process_manager"
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

	commandAny := commandBook.Pages[0].Command
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
		if rejection.RejectedCommand.Pages[0].Command != nil {
			cmdTypeURL := rejection.RejectedCommand.Pages[0].Command.TypeUrl
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

// Descriptor builds a ComponentDescriptor from registered handlers.
func (r *CommandRouter[S]) Descriptor() *pb.ComponentDescriptor {
	return &pb.ComponentDescriptor{
		Name:          r.domain,
		ComponentType: ComponentAggregate,
		Inputs: []*pb.Target{
			{
				Domain: r.domain,
				Types:  r.Types(),
			},
		},
	}
}

// Types returns registered command type suffixes.
func (r *CommandRouter[S]) Types() []string {
	types := make([]string, len(r.handlers))
	for i, reg := range r.handlers {
		types[i] = reg.suffix
	}
	return types
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

// outputTarget tracks a domain and its command types for topology.
type outputTarget struct {
	domain string
	types  []string
}

// EventRouter dispatches events to handlers by type_url suffix.
// Used for sagas that translate events from one domain to commands in another.
//
// Example:
//
//	router := NewEventRouter("saga-order-fulfillment", "order").
//	    Sends("fulfillment", "CreateShipment").
//	    Prepare("OrderCompleted", prepareOrderCompleted).
//	    On("OrderCompleted", handleOrderCompleted)
//
//	// In saga Execute():
//	commands, err := router.Dispatch(sourceEventBook, destinations)
type EventRouter struct {
	name            string
	inputDomain     string
	outputTargets   []outputTarget
	handlers        []eventRegistration
	prepareHandlers []prepareRegistration
}

type eventRegistration struct {
	suffix  string
	handler EventHandler
}

type prepareRegistration struct {
	suffix  string
	handler PrepareHandler
}

// NewEventRouter creates a new router for the given saga name and input domain.
func NewEventRouter(name, inputDomain string) *EventRouter {
	return &EventRouter{
		name:            name,
		inputDomain:     inputDomain,
		outputTargets:   make([]outputTarget, 0),
		handlers:        make([]eventRegistration, 0),
		prepareHandlers: make([]prepareRegistration, 0),
	}
}

// Sends declares an output domain and command type this saga produces.
// Call multiple times for multiple command types or domains.
func (r *EventRouter) Sends(domain, commandType string) *EventRouter {
	// Find existing target for this domain or create new one
	for i := range r.outputTargets {
		if r.outputTargets[i].domain == domain {
			r.outputTargets[i].types = append(r.outputTargets[i].types, commandType)
			return r
		}
	}
	r.outputTargets = append(r.outputTargets, outputTarget{domain: domain, types: []string{commandType}})
	return r
}

// Prepare registers a prepare handler for an event type_url suffix.
// The prepare handler declares which destinations are needed before Execute.
func (r *EventRouter) Prepare(suffix string, handler PrepareHandler) *EventRouter {
	r.prepareHandlers = append(r.prepareHandlers, prepareRegistration{suffix: suffix, handler: handler})
	return r
}

// On registers a handler for an event type_url suffix.
func (r *EventRouter) On(suffix string, handler EventHandler) *EventRouter {
	r.handlers = append(r.handlers, eventRegistration{suffix: suffix, handler: handler})
	return r
}

// PrepareDestinations returns the destination covers needed for the given source.
// Called during the Prepare phase of the two-phase saga protocol.
func (r *EventRouter) PrepareDestinations(source *pb.EventBook) []*pb.Cover {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	// Use the last event page
	page := source.Pages[len(source.Pages)-1]
	if page.Event == nil {
		return nil
	}

	for _, reg := range r.prepareHandlers {
		if strings.HasSuffix(page.Event.TypeUrl, reg.suffix) {
			return reg.handler(source, page.Event)
		}
	}
	return nil
}

// Dispatch routes all events in an EventBook to registered handlers.
//
// Iterates pages, matches type_url suffixes, and collects commands.
// Destinations are the EventBooks for the covers declared in Prepare.
func (r *EventRouter) Dispatch(source *pb.EventBook, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
	if source == nil {
		return nil, nil
	}

	var commands []*pb.CommandBook
	for _, page := range source.Pages {
		if page.Event == nil {
			continue
		}
		for _, reg := range r.handlers {
			if strings.HasSuffix(page.Event.TypeUrl, reg.suffix) {
				cmds, err := reg.handler(source, page.Event, destinations)
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

// Descriptor builds a ComponentDescriptor from registered handlers.
func (r *EventRouter) Descriptor() *pb.ComponentDescriptor {
	return &pb.ComponentDescriptor{
		Name:          r.name,
		ComponentType: ComponentSaga,
		Inputs: []*pb.Target{
			{
				Domain: r.inputDomain,
				Types:  r.Types(),
			},
		},
	}
}

// Types returns registered event type suffixes.
func (r *EventRouter) Types() []string {
	types := make([]string, len(r.handlers))
	for i, reg := range r.handlers {
		types[i] = reg.suffix
	}
	return types
}

// OutputDomains returns the list of output domain names.
func (r *EventRouter) OutputDomains() []string {
	domains := make([]string, len(r.outputTargets))
	for i, t := range r.outputTargets {
		domains[i] = t.domain
	}
	return domains
}

// OutputTypes returns the command types for a given output domain.
func (r *EventRouter) OutputTypes(domain string) []string {
	for _, t := range r.outputTargets {
		if t.domain == domain {
			return t.types
		}
	}
	return nil
}
