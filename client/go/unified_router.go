// Package angzarr provides unified routers for aggregates, sagas, process managers, and projectors.
//
// Two router patterns based on domain cardinality:
//
//   - AggregateRouter/SagaRouter: For single-domain components (domain set at construction)
//   - ProcessManagerRouter/ProjectorRouter: For multi-domain components (fluent .Domain() pattern)
//
// Example:
//
//	// Aggregate (single domain -- domain in constructor)
//	router := NewAggregateRouter("player", "player", playerHandler)
//
//	// Saga (single domain -- domain in constructor)
//	router := NewSagaRouter("saga-order-fulfillment", "order", orderHandler)
//
//	// Process Manager (multi-domain -- fluent .Domain())
//	router := NewProcessManagerRouter[*PMState]("pmg-hand-flow", "hand-flow", rebuildState).
//	    Domain("order", orderPmHandler).
//	    Domain("inventory", inventoryPmHandler)
//
//	// Projector (multi-domain -- fluent .Domain())
//	router := NewProjectorRouter("prj-output").
//	    Domain("player", playerProjectorHandler).
//	    Domain("hand", handProjectorHandler)
package angzarr

import (
	"fmt"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// ============================================================================
// AggregateRouter -- Single Domain
// ============================================================================

// AggregateRouter wraps an AggregateDomainHandler for routing commands.
//
// Domain is set at construction time. No Domain() method exists,
// enforcing single-domain constraint.
type AggregateRouter[S any] struct {
	name    string
	domain  string
	handler AggregateDomainHandler[S]
}

// NewAggregateRouter creates a new aggregate router.
//
// Aggregates handle commands and emit events. Single domain enforced at construction.
func NewAggregateRouter[S any](name, domain string, handler AggregateDomainHandler[S]) *AggregateRouter[S] {
	return &AggregateRouter[S]{
		name:    name,
		domain:  domain,
		handler: handler,
	}
}

// Name returns the router name.
func (r *AggregateRouter[S]) Name() string {
	return r.name
}

// Domain returns the domain.
func (r *AggregateRouter[S]) Domain() string {
	return r.domain
}

// CommandTypes returns command types from the handler.
func (r *AggregateRouter[S]) CommandTypes() []string {
	return r.handler.CommandTypes()
}

// Subscriptions returns subscriptions for this aggregate.
// Returns a map of domain -> command types.
func (r *AggregateRouter[S]) Subscriptions() map[string][]string {
	return map[string][]string{
		r.domain: r.handler.CommandTypes(),
	}
}

// RebuildState rebuilds state from events using the handler.
func (r *AggregateRouter[S]) RebuildState(events *pb.EventBook) S {
	return r.handler.Rebuild(events)
}

// Dispatch routes a contextual command to the handler.
func (r *AggregateRouter[S]) Dispatch(cmd *pb.ContextualCommand) (*pb.BusinessResponse, error) {
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
		return r.dispatchNotification(commandAny, state)
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

// dispatchNotification routes a Notification to the aggregate's rejection handler.
func (r *AggregateRouter[S]) dispatchNotification(commandAny *anypb.Any, state S) (*pb.BusinessResponse, error) {
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

	domain, cmdSuffix := extractRejectionKeyFromNotification(rejection)

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

	commands, err := r.handler.Execute(source, eventAny, destinations)
	if err != nil {
		return nil, err
	}

	return &pb.SagaResponse{
		Commands: commands,
		Events:   []*pb.EventBook{},
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

	domain, cmdSuffix := extractRejectionKeyFromNotification(rejection)

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

// ============================================================================
// Helper Functions
// ============================================================================

// extractRejectionKeyFromNotification extracts domain and command suffix from a RejectionNotification.
func extractRejectionKeyFromNotification(rejection *pb.RejectionNotification) (string, string) {
	if rejection == nil || rejection.RejectedCommand == nil {
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
			cmdSuffix = TypeNameFromURL(cmd.TypeUrl)
		}
	}

	return domain, cmdSuffix
}
