// Package angzarr provides handler interfaces (traits) for the unified router pattern.
//
// These interfaces define the contract for domain handlers. Implementors
// encapsulate their routing logic internally and declare which types
// they handle via CommandTypes(), EventTypes(), etc.
//
// This mirrors the Rust trait pattern from client/rust/src/router/traits.rs.
package angzarr

import (
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
)

// ============================================================================
// Common Types
// ============================================================================

// RejectionHandlerResponse is defined in compensation.go.
// It contains optional Events (compensation) and Notification (upstream propagation).

// SagaHandlerResponse is the response from saga handlers.
type SagaHandlerResponse struct {
	// Commands to send to other aggregates.
	Commands []*pb.CommandBook
	// Facts/events to inject to other aggregates.
	Events []*pb.EventBook
}

// ProcessManagerResponse is the response from process manager handlers.
type ProcessManagerResponse struct {
	// Commands to send to other aggregates.
	Commands []*pb.CommandBook
	// Events to persist to the PM's own domain.
	ProcessEvents *pb.EventBook
	// Facts to inject to other aggregates.
	Facts []*pb.EventBook
}

// ============================================================================
// CommandHandler Handler Interface
// ============================================================================

// CommandHandlerDomainHandler is the handler for a single domain's commands.
//
// Command handlers receive commands and emit events. They maintain state
// that is rebuilt from events using a StateRouter.
//
// Example:
//
//	type PlayerHandler struct {
//	    stateRouter *angzarr.StateRouter[PlayerState]
//	}
//
//	func (h *PlayerHandler) CommandTypes() []string {
//	    return []string{"examples.RegisterPlayer", "examples.DepositFunds"}
//	}
//
//	func (h *PlayerHandler) StateRouter() *angzarr.StateRouter[PlayerState] {
//	    return h.stateRouter
//	}
//
//	func (h *PlayerHandler) Handle(
//	    cmd *pb.CommandBook,
//	    payload *anypb.Any,
//	    state *PlayerState,
//	    seq uint32,
//	) (*pb.EventBook, error) {
//	    // Dispatch to specific command handlers
//	}
type CommandHandlerDomainHandler[S any] interface {
	// CommandTypes returns the fully-qualified command type names this handler processes.
	// Used for subscription derivation and routing.
	CommandTypes() []string

	// Rebuild reconstructs state from events.
	Rebuild(events *pb.EventBook) S

	// Handle processes a command and returns resulting events.
	// The handler should dispatch internally based on payload.TypeUrl.
	Handle(
		cmd *pb.CommandBook,
		payload *anypb.Any,
		state S,
		seq uint32,
	) (*pb.EventBook, error)

	// OnRejected handles a rejection notification.
	// Called when a command issued by a saga/PM targeting this command handler's
	// domain was rejected. Override to provide custom compensation logic.
	// Default implementation should return an empty response.
	OnRejected(
		notification *pb.Notification,
		state S,
		targetDomain string,
		targetCommand string,
	) (*RejectionHandlerResponse, error)
}

// ============================================================================
// Saga Handler Interface
// ============================================================================

// SagaDomainHandler is the handler for a single domain's events in a saga.
//
// Sagas translate events from one domain into commands for another.
// They are stateless -- each event is processed independently.
//
// Example:
//
//	type OrderSagaHandler struct{}
//
//	func (h *OrderSagaHandler) EventTypes() []string {
//	    return []string{"examples.OrderCompleted", "examples.OrderCancelled"}
//	}
//
//	func (h *OrderSagaHandler) Prepare(source *pb.EventBook, event *anypb.Any) []*pb.Cover {
//	    // Declare needed destinations
//	}
//
//	func (h *OrderSagaHandler) Execute(
//	    source *pb.EventBook,
//	    event *anypb.Any,
//	    destinations []*pb.EventBook,
//	) (*SagaHandlerResponse, error) {
//	    // Produce commands and/or events
//	}
type SagaDomainHandler interface {
	// EventTypes returns the fully-qualified event type names this handler processes.
	// Used for subscription derivation.
	EventTypes() []string

	// Prepare declares destination covers needed.
	// Called before Execute to fetch destination aggregate state.
	Prepare(source *pb.EventBook, event *anypb.Any) []*pb.Cover

	// Execute produces commands and/or events.
	// Called with source event and fetched destination state.
	Execute(
		source *pb.EventBook,
		event *anypb.Any,
		destinations []*pb.EventBook,
	) (*SagaHandlerResponse, error)

	// OnRejected handles a rejection notification.
	// Called when a saga-issued command was rejected.
	// Default implementation should return an empty response.
	OnRejected(
		notification *pb.Notification,
		targetDomain string,
		targetCommand string,
	) (*RejectionHandlerResponse, error)
}

// ============================================================================
// Process Manager Handler Interface
// ============================================================================

// ProcessManagerDomainHandler is the handler for a single domain's events in a PM.
//
// Process managers correlate events across multiple domains and maintain
// their own state. Each domain gets its own handler, but they all share
// the same PM state type.
//
// Example:
//
//	type OrderPmHandler struct{}
//
//	func (h *OrderPmHandler) EventTypes() []string {
//	    return []string{"examples.OrderCreated"}
//	}
//
//	func (h *OrderPmHandler) Prepare(
//	    trigger *pb.EventBook,
//	    state *HandFlowState,
//	    event *anypb.Any,
//	) []*pb.Cover {
//	    // Declare needed destinations
//	}
//
//	func (h *OrderPmHandler) Handle(
//	    trigger *pb.EventBook,
//	    state *HandFlowState,
//	    event *anypb.Any,
//	    destinations []*pb.EventBook,
//	) (*ProcessManagerResponse, error) {
//	    // Process event, emit commands and/or PM events
//	}
type ProcessManagerDomainHandler[S any] interface {
	// EventTypes returns the fully-qualified event type names this handler processes.
	EventTypes() []string

	// Prepare declares destination covers needed.
	Prepare(trigger *pb.EventBook, state S, event *anypb.Any) []*pb.Cover

	// Handle processes the event and produces commands and PM events.
	Handle(
		trigger *pb.EventBook,
		state S,
		event *anypb.Any,
		destinations []*pb.EventBook,
	) (*ProcessManagerResponse, error)

	// OnRejected handles a rejection notification.
	// Called when a PM-issued command was rejected.
	OnRejected(
		notification *pb.Notification,
		state S,
		targetDomain string,
		targetCommand string,
	) (*RejectionHandlerResponse, error)
}

// ============================================================================
// Projector Handler Interface
// ============================================================================

// ProjectorDomainHandler is the handler for a single domain's events in a projector.
//
// Projectors consume events and produce external output (read models,
// caches, external systems).
//
// Example:
//
//	type PlayerProjectorHandler struct{}
//
//	func (h *PlayerProjectorHandler) EventTypes() []string {
//	    return []string{"examples.PlayerRegistered", "examples.FundsDeposited"}
//	}
//
//	func (h *PlayerProjectorHandler) Project(events *pb.EventBook) (*pb.Projection, error) {
//	    // Update external read model
//	}
type ProjectorDomainHandler interface {
	// EventTypes returns the fully-qualified event type names this handler processes.
	EventTypes() []string

	// Project processes events and produces external output.
	Project(events *pb.EventBook) (*pb.Projection, error)
}
