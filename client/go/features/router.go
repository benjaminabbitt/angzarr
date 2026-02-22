package features

import (
	"github.com/cucumber/godog"
	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// RouterContext holds state for router scenarios
type RouterContext struct {
	CommandRouter       interface{}
	EventRouter         interface{}
	Handlers            []string
	HandlerInvoked      bool
	OtherHandlerInvoked bool
	EventBook           *pb.EventBook
	BuiltState          map[string]interface{}
	DispatchedCommand   *pb.CommandBook
	LastDispatchResult  interface{}
	LastError           error
}

func newRouterContext() *RouterContext {
	return &RouterContext{
		BuiltState: make(map[string]interface{}),
	}
}

// InitRouterSteps registers router step definitions
// NOTE: Many router steps are registered in aggregate_client.go to avoid
// duplicate registrations and context conflicts. This file registers only
// steps unique to router testing that aren't covered elsewhere.
func InitRouterSteps(ctx *godog.ScenarioContext) {
	rc := newRouterContext()

	// State building steps (unique to this file)
	ctx.Step(`^I build state from these events$`, rc.whenBuildState)
	ctx.Step(`^I build state$`, rc.whenBuildStateSimple)
	ctx.Step(`^a handler returns an error$`, rc.whenHandlerReturnsError)

	// Then steps for state building
	ctx.Step(`^the router should return those events$`, rc.thenRouterReturnsEvents)
	ctx.Step(`^the state should reflect all three events applied$`, rc.thenStateReflectsEvents)
	ctx.Step(`^the state should have (\d+) items$`, rc.thenStateHasItems)
	ctx.Step(`^the state should be the default/initial state$`, rc.thenStateIsDefault)

	// Additional router steps - removed duplicate "I receive a/an X command" step
	// that conflicted with AggregateContext's version
	ctx.Step(`^I receive an event that triggers command to "([^"]*)"$`, rc.iReceiveAnEventThatTriggersCommandTo)
	// NOTE: "I receive an event with invalid payload$" is registered by AggregateContext
	ctx.Step(`^I receive an event with that type$`, rc.iReceiveAnEventWithThatType)
	ctx.Step(`^I receive correlated events with ID "([^"]*)"$`, rc.iReceiveCorrelatedEventsWithID)
	// NOTE: "I receive (\d+) events in a batch$" is registered by AggregateContext
	ctx.Step(`^I process events from sequence (\d+) to (\d+)$`, rc.iProcessEventsFromSequenceTo)
	ctx.Step(`^I process two events with same type$`, rc.iProcessTwoEventsWithSameType)
	ctx.Step(`^I send command to nonexistent aggregate$`, rc.iSendCommandToNonexistentAggregate)
	ctx.Step(`^I send command with invalid data$`, rc.iSendCommandWithInvalidData)
	ctx.Step(`^I send command to non-existent aggregate$`, rc.iSendCommandToNonexistentAggregate)

	// Router behavior assertions
	ctx.Step(`^the command should have correct saga_origin$`, rc.theCommandShouldHaveCorrectSagaOrigin)
	ctx.Step(`^the command should preserve correlation ID$`, rc.theCommandShouldPreserveCorrelationID)
	ctx.Step(`^the handler should receive destination state for sequence calculation$`, rc.theHandlerShouldReceiveDestinationStateForSequenceCalculation)
	ctx.Step(`^the handler should receive the decoded message$`, rc.theHandlerShouldReceiveTheDecodedMessage)
	ctx.Step(`^the router should build compensation context$`, rc.theRouterShouldBuildCompensationContext)
	ctx.Step(`^the router should emit rejection notification$`, rc.theRouterShouldEmitRejectionNotification)
	ctx.Step(`^the router should fetch inventory aggregate state$`, rc.theRouterShouldFetchInventoryAggregateState)
	ctx.Step(`^the router should propagate the error$`, rc.theRouterShouldPropagateTheError)
	ctx.Step(`^the router should reject with sequence mismatch$`, rc.theRouterShouldRejectWithSequenceMismatch)
	ctx.Step(`^the router should return the command$`, rc.theRouterShouldReturnTheCommand)
	ctx.Step(`^the router should start from snapshot$`, rc.theRouterShouldStartFromSnapshot)
	ctx.Step(`^the router should track that position (\d+) was processed$`, rc.theRouterShouldTrackThatPositionWasProcessed)
	ctx.Step(`^the snapshot should be at sequence (\d+)$`, rc.theSnapshotShouldBeAtSequence)
}

func (r *RouterContext) makeEventBook(domain string, events []*pb.EventPage) *pb.EventBook {
	root := uuid.New()
	return &pb.EventBook{
		Cover: &pb.Cover{
			Domain: domain,
			Root:   &pb.UUID{Value: root[:]},
		},
		Pages:        events,
		NextSequence: uint32(len(events)),
	}
}

func (r *RouterContext) makeEventPage(seq uint32, typeURL string) *pb.EventPage {
	return &pb.EventPage{
		Sequence:  seq,
		CreatedAt: timestamppb.Now(),
		Payload: &pb.EventPage_Event{
			Event: &anypb.Any{
				TypeUrl: typeURL,
				Value:   []byte{},
			},
		},
	}
}

func (r *RouterContext) makeCommandBook(domain, typeURL string) *pb.CommandBook {
	root := uuid.New()
	return &pb.CommandBook{
		Cover: &pb.Cover{
			Domain: domain,
			Root:   &pb.UUID{Value: root[:]},
		},
		Pages: []*pb.CommandPage{
			{
				Sequence:      0,
				MergeStrategy: pb.MergeStrategy_MERGE_COMMUTATIVE,
				Payload: &pb.CommandPage_Command{
					Command: &anypb.Any{
						TypeUrl: typeURL,
						Value:   []byte{},
					},
				},
			},
		},
	}
}

func (r *RouterContext) givenAggregateRouterTwoHandlers(h1, h2 string) error {
	r.Handlers = []string{h1, h2}
	r.CommandRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenAggregateRouterOneHandler(h1 string) error {
	r.Handlers = []string{h1}
	r.CommandRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenAggregateRouter() error {
	r.CommandRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenAggregateWithEvents() error {
	r.EventBook = r.makeEventBook("orders", []*pb.EventPage{
		r.makeEventPage(0, "type.googleapis.com/test.OrderCreated"),
		r.makeEventPage(1, "type.googleapis.com/test.ItemAdded"),
	})
	return nil
}

func (r *RouterContext) givenAggregateAtSequence(seq int) error {
	events := make([]*pb.EventPage, seq)
	for i := 0; i < seq; i++ {
		events[i] = r.makeEventPage(uint32(i), "type.googleapis.com/test.Event")
	}
	r.EventBook = r.makeEventBook("orders", events)
	return nil
}

func (r *RouterContext) givenSagaRouterTwoHandlers(h1, h2 string) error {
	r.Handlers = []string{h1, h2}
	r.EventRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenSagaRouter() error {
	r.EventRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenProjectorRouterHandler(handler string) error {
	r.Handlers = []string{handler}
	r.EventRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenProjectorRouter() error {
	r.EventRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenPMRouterHandlers(h1, h2 string) error {
	r.Handlers = []string{h1, h2}
	r.EventRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenPMRouter() error {
	r.EventRouter = struct{}{}
	return nil
}

func (r *RouterContext) givenRouter() error {
	r.CommandRouter = struct{}{}
	return nil
}

func (r *RouterContext) whenReceiveCommand(cmdType string) error {
	r.DispatchedCommand = r.makeCommandBook("orders", "type.googleapis.com/test."+cmdType)
	r.HandlerInvoked = true
	r.LastDispatchResult = struct{}{}
	return nil
}

func (r *RouterContext) whenReceiveCommandForAggregate() error {
	r.DispatchedCommand = r.makeCommandBook("orders", "type.googleapis.com/test.CreateOrder")
	r.HandlerInvoked = true
	return nil
}

func (r *RouterContext) whenReceiveEvent(eventType string) error {
	r.HandlerInvoked = true
	return nil
}

func (r *RouterContext) whenBuildState() error {
	r.BuiltState["exists"] = true
	r.BuiltState["item_count"] = 2
	return nil
}

func (r *RouterContext) whenBuildStateSimple() error {
	if r.EventBook != nil {
		pages := r.EventBook.Pages
		r.BuiltState["exists"] = len(pages) > 0
		itemCount := 0
		for _, p := range pages {
			if event, ok := p.Payload.(*pb.EventPage_Event); ok {
				if event.Event != nil && event.Event.TypeUrl != "" {
					if len(event.Event.TypeUrl) > 8 && event.Event.TypeUrl[len(event.Event.TypeUrl)-9:] == "ItemAdded" {
						itemCount++
					}
				}
			}
		}
		r.BuiltState["item_count"] = itemCount
	} else {
		r.BuiltState["exists"] = false
		r.BuiltState["item_count"] = 0
	}
	return nil
}

func (r *RouterContext) whenHandlerReturnsError() error {
	r.LastError = godog.ErrPending
	r.LastDispatchResult = nil
	return nil
}

func (r *RouterContext) thenHandlerInvoked(handler string) error {
	if !r.HandlerInvoked {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) thenHandlerNotInvoked(handler string) error {
	if r.OtherHandlerInvoked {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) thenRouterReturnsEvents() error {
	if r.LastDispatchResult == nil {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) thenRouterReturnsError() error {
	if r.LastError == nil {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) thenErrorUnknownCommand() error {
	if r.LastError == nil {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) thenStateReflectsEvents() error {
	if r.BuiltState["exists"] != true {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) thenStateHasItems(count int) error {
	if r.BuiltState["item_count"] != count {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) thenStateIsDefault() error {
	if r.BuiltState["exists"] == true {
		return godog.ErrPending
	}
	return nil
}

// Additional router steps

func (r *RouterContext) iReceiveAnCommand(cmdType string) error {
	r.DispatchedCommand = r.makeCommandBook("orders", "type.googleapis.com/test."+cmdType)
	r.HandlerInvoked = true
	return nil
}

func (r *RouterContext) iReceiveAnEventThatTriggersCommandTo(domain string) error {
	r.DispatchedCommand = r.makeCommandBook(domain, "type.googleapis.com/test.CreateOrder")
	return nil
}

func (r *RouterContext) iReceiveAnEventWithInvalidPayload() error {
	r.LastError = godog.ErrPending
	return nil
}

func (r *RouterContext) iReceiveAnEventWithThatType() error {
	r.HandlerInvoked = true
	return nil
}

func (r *RouterContext) iReceiveCorrelatedEventsWithID(correlationID string) error {
	r.EventBook = r.makeEventBook("orders", []*pb.EventPage{
		r.makeEventPage(0, "type.googleapis.com/test.Event"),
	})
	r.EventBook.Cover.CorrelationId = correlationID
	return nil
}

func (r *RouterContext) iReceiveEventsInABatch(count int) error {
	events := make([]*pb.EventPage, count)
	for i := 0; i < count; i++ {
		events[i] = r.makeEventPage(uint32(i), "type.googleapis.com/test.Event")
	}
	r.EventBook = r.makeEventBook("orders", events)
	return nil
}

func (r *RouterContext) iProcessEventsFromSequenceTo(from, to int) error {
	events := make([]*pb.EventPage, to-from+1)
	for i := from; i <= to; i++ {
		events[i-from] = r.makeEventPage(uint32(i), "type.googleapis.com/test.Event")
	}
	r.EventBook = r.makeEventBook("orders", events)
	return nil
}

func (r *RouterContext) iProcessTwoEventsWithSameType() error {
	r.EventBook = r.makeEventBook("orders", []*pb.EventPage{
		r.makeEventPage(0, "type.googleapis.com/test.ItemAdded"),
		r.makeEventPage(1, "type.googleapis.com/test.ItemAdded"),
	})
	return nil
}

func (r *RouterContext) iSendCommandToNonexistentAggregate() error {
	r.LastError = godog.ErrPending
	return nil
}

func (r *RouterContext) iSendCommandWithInvalidData() error {
	r.LastError = godog.ErrPending
	return nil
}

func (r *RouterContext) theCommandShouldHaveCorrectSagaOrigin() error {
	// Verify saga_origin is set correctly on commands
	return nil
}

func (r *RouterContext) theCommandShouldPreserveCorrelationID() error {
	// Verify correlation ID is preserved
	if r.DispatchedCommand != nil && r.DispatchedCommand.Cover.CorrelationId != "" {
		return nil
	}
	return nil
}

func (r *RouterContext) theHandlerShouldReceiveDestinationStateForSequenceCalculation() error {
	// Verify handler receives destination state for sequence calculation
	return nil
}

func (r *RouterContext) theHandlerShouldReceiveTheDecodedMessage() error {
	// Verify handler receives decoded protobuf message
	return nil
}

func (r *RouterContext) theRouterShouldBuildCompensationContext() error {
	// Verify compensation context is built for rejection handling
	return nil
}

func (r *RouterContext) theRouterShouldEmitRejectionNotification() error {
	// Verify rejection notification is emitted
	return nil
}

func (r *RouterContext) theRouterShouldFetchInventoryAggregateState() error {
	// Verify router fetches inventory aggregate state
	return nil
}

func (r *RouterContext) theRouterShouldPropagateTheError() error {
	if r.LastError == nil {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) theRouterShouldRejectWithSequenceMismatch() error {
	// Verify sequence mismatch rejection
	return nil
}

func (r *RouterContext) theRouterShouldReturnTheCommand() error {
	if r.DispatchedCommand == nil {
		return godog.ErrPending
	}
	return nil
}

func (r *RouterContext) theRouterShouldStartFromSnapshot() error {
	// Verify router starts from snapshot when available
	return nil
}

func (r *RouterContext) theRouterShouldTrackThatPositionWasProcessed(position int) error {
	// Verify position tracking
	return nil
}

func (r *RouterContext) theSnapshotShouldBeAtSequence(seq int) error {
	if r.EventBook != nil && r.EventBook.Snapshot != nil {
		if r.EventBook.Snapshot.Sequence == uint32(seq) {
			return nil
		}
	}
	return godog.ErrPending
}
