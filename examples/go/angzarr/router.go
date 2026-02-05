// Package angzarr provides shared utilities for angzarr example implementations.
//
// CommandRouter[S] replaces manual switch/case chains in aggregate handlers.
// EventRouter replaces manual if/else chains in saga event handlers.
// Both auto-derive type lists and descriptors from their .On() registrations.
package angzarr

import (
	"fmt"
	"strings"

	"google.golang.org/protobuf/types/known/anypb"

	angzarrpb "angzarr/proto/angzarr"
)

// Component type constants for Descriptor.
const (
	ComponentAggregate      = "aggregate"
	ComponentSaga           = "saga"
	ComponentProcessManager = "process_manager"
	ComponentProjector      = "projector"
)

// Error message constants.
const (
	ErrMsgUnknownCommand = "unknown command type"
	ErrMsgNoCommandPages = "no command pages"
)

// ============================================================================
// Descriptor types — mirrors angzarr ComponentDescriptor
// ============================================================================

// Descriptor describes a component for topology discovery.
//
// Mirrors angzarr.ComponentDescriptor without requiring the regenerated proto.
// Will be replaced by the real proto type once generated code includes it.
type Descriptor struct {
	Name          string
	ComponentType string
	Inputs        []SubscriptionDesc
}

// SubscriptionDesc describes what a component subscribes to (maps to Target in proto).
type SubscriptionDesc struct {
	Domain     string
	EventTypes []string // maps to Types in proto
}

// ============================================================================
// CommandRouter — aggregate dispatch
// ============================================================================

// CommandHandler processes a command and returns events.
//
// Receives the CommandBook (for cover metadata), raw command bytes,
// rebuilt state, and next sequence number. Returns new events.
type CommandHandler[S any] func(cb *angzarrpb.CommandBook, data []byte, state *S, seq uint32) (*angzarrpb.EventBook, error)

type commandEntry[S any] struct {
	suffix  string
	handler CommandHandler[S]
}

// CommandRouter dispatches commands to handlers by type_url suffix.
//
// Replaces manual switch/case dispatch in aggregate handlers.
// Takes a ContextualCommand, rebuilds state, matches the command's type_url
// suffix, dispatches to the registered handler, and wraps the result in
// a BusinessResponse.
//
// Example:
//
//	router := common.NewCommandRouter("cart", rebuildState).
//	    On("CreateCart", handleCreateCart).
//	    On("AddItem", handleAddItem)
//
//	// In Handle():
//	resp, err := router.Dispatch(req)
//
//	// For topology:
//	desc := router.Descriptor()
type CommandRouter[S any] struct {
	domain  string
	rebuild func(*angzarrpb.EventBook) S
	entries []commandEntry[S]
}

// NewCommandRouter creates a command router for a domain.
//
// - domain: The aggregate's domain name (e.g., "order").
// - rebuild: Function to rebuild state from prior events.
func NewCommandRouter[S any](domain string, rebuild func(*angzarrpb.EventBook) S) *CommandRouter[S] {
	return &CommandRouter[S]{domain: domain, rebuild: rebuild}
}

// On registers a handler for a command type_url suffix.
//
// The suffix is matched against the end of the command's type_url.
// E.g., .On("CreateCart", handleCreateCart) matches any type_url ending in "CreateCart".
func (r *CommandRouter[S]) On(suffix string, handler CommandHandler[S]) *CommandRouter[S] {
	r.entries = append(r.entries, commandEntry[S]{suffix, handler})
	return r
}

// Dispatch extracts the command from a ContextualCommand, rebuilds state,
// matches the type_url suffix, calls the handler, and returns a BusinessResponse.
func (r *CommandRouter[S]) Dispatch(cmd *angzarrpb.ContextualCommand) (*angzarrpb.BusinessResponse, error) {
	cmdBook := cmd.GetCommand()
	priorEvents := cmd.GetEvents()

	state := r.rebuild(priorEvents)
	seq := NextSequence(priorEvents)

	if cmdBook == nil || len(cmdBook.Pages) == 0 {
		return nil, fmt.Errorf("%s", ErrMsgNoCommandPages)
	}

	cmdAny := cmdBook.Pages[0].Command
	if cmdAny == nil {
		return nil, fmt.Errorf("%s", ErrMsgNoCommandPages)
	}

	for _, e := range r.entries {
		if strings.HasSuffix(cmdAny.TypeUrl, e.suffix) {
			events, err := e.handler(cmdBook, cmdAny.Value, &state, seq)
			if err != nil {
				return nil, err
			}
			return &angzarrpb.BusinessResponse{
				Result: &angzarrpb.BusinessResponse_Events{Events: events},
			}, nil
		}
	}

	return nil, fmt.Errorf("%s: %s", ErrMsgUnknownCommand, cmdAny.TypeUrl)
}

// Domain returns the aggregate domain name.
func (r *CommandRouter[S]) Domain() string { return r.domain }

// Descriptor builds a component descriptor from registered handlers.
func (r *CommandRouter[S]) Descriptor() Descriptor {
	return Descriptor{
		Name:          r.domain,
		ComponentType: ComponentAggregate,
		Inputs: []SubscriptionDesc{
			{Domain: r.domain, EventTypes: r.Types()},
		},
	}
}

// Types returns registered command type suffixes.
func (r *CommandRouter[S]) Types() []string {
	result := make([]string, len(r.entries))
	for i, e := range r.entries {
		result[i] = e.suffix
	}
	return result
}

// ============================================================================
// Helpers
// ============================================================================

// NextSequence computes the next event sequence number from prior events.
func NextSequence(events *angzarrpb.EventBook) uint32 {
	if events == nil || len(events.Pages) == 0 {
		return 0
	}
	return uint32(len(events.Pages))
}

// ============================================================================
// EventRouter — saga dispatch
// ============================================================================

// SagaEventHandler processes a single event and returns commands to issue.
//
// Receives the event Any (for type-specific decoding), source root UUID,
// and correlation ID. Returns commands to execute on other aggregates.
type SagaEventHandler func(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook

type eventEntry struct {
	suffix  string
	handler SagaEventHandler
}

// EventRouter dispatches events to handlers by type_url suffix.
//
// Replaces manual if/else dispatch in saga event handlers.
// Takes an EventBook, iterates its pages, matches type_url suffixes,
// and collects commands from handlers.
//
// Example:
//
//	router := common.NewEventRouter("fulfillment", "order").
//	    Output("fulfillment").
//	    On("OrderCompleted", handleOrderCompleted)
//
//	// In saga Execute:
//	commands := router.Dispatch(sourceEventBook)
//
//	// For topology:
//	desc := router.Descriptor()
type EventRouter struct {
	name          string
	inputDomain   string
	outputDomains []string
	entries       []eventEntry
}

// NewEventRouter creates an event router for a saga.
//
// - name: The saga's name (e.g., "fulfillment").
// - inputDomain: The domain to subscribe to for events.
func NewEventRouter(name, inputDomain string) *EventRouter {
	return &EventRouter{name: name, inputDomain: inputDomain}
}

// Output declares an output domain for this saga.
func (r *EventRouter) Output(domain string) *EventRouter {
	r.outputDomains = append(r.outputDomains, domain)
	return r
}

// On registers a handler for an event type_url suffix.
//
// The suffix is matched against the end of the event's type_url.
// E.g., .On("OrderCompleted", handleOrderCompleted) matches any
// type_url ending in "OrderCompleted".
func (r *EventRouter) On(suffix string, handler SagaEventHandler) *EventRouter {
	r.entries = append(r.entries, eventEntry{suffix, handler})
	return r
}

// Dispatch iterates all event pages in the EventBook and dispatches
// matching events to registered handlers, collecting all commands.
func (r *EventRouter) Dispatch(book *angzarrpb.EventBook) []*angzarrpb.CommandBook {
	var root *angzarrpb.UUID
	var correlationID string
	if book.Cover != nil {
		root = book.Cover.Root
		correlationID = book.Cover.GetCorrelationId()
	}

	var commands []*angzarrpb.CommandBook
	for _, page := range book.Pages {
		if page.Event == nil {
			continue
		}
		for _, e := range r.entries {
			if strings.HasSuffix(page.Event.TypeUrl, e.suffix) {
				commands = append(commands, e.handler(page.Event, root, correlationID)...)
				break
			}
		}
	}
	return commands
}

// Name returns the saga name.
func (r *EventRouter) Name() string { return r.name }

// InputDomain returns the subscribed input domain.
func (r *EventRouter) InputDomain() string { return r.inputDomain }

// OutputDomains returns the declared output domains.
func (r *EventRouter) OutputDomains() []string { return r.outputDomains }

// Descriptor builds a component descriptor from registered handlers.
func (r *EventRouter) Descriptor() Descriptor {
	return Descriptor{
		Name:          r.name,
		ComponentType: ComponentSaga,
		Inputs: []SubscriptionDesc{
			{Domain: r.inputDomain, EventTypes: r.Types()},
		},
	}
}

// Types returns registered event type suffixes.
func (r *EventRouter) Types() []string {
	result := make([]string, len(r.entries))
	for i, e := range r.entries {
		result[i] = e.suffix
	}
	return result
}
