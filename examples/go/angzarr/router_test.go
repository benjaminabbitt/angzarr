package angzarr

import (
	"fmt"
	"testing"

	"google.golang.org/protobuf/types/known/anypb"

	angzarrpb "angzarr/proto/angzarr"
)

// Test constants — reused across test cases.
const (
	domainTest        = "test"
	domainOrder       = "order"
	domainFulfillment = "fulfillment"
	domainInventory   = "inventory"
	domainCart        = "cart"

	suffixCommandA      = "CommandA"
	suffixCommandB      = "CommandB"
	suffixCreate        = "Create"
	suffixCreateOrder   = "CreateOrder"
	suffixCancelOrder   = "CancelOrder"
	suffixOrderComplete = "OrderCompleted"
	suffixOrderCancel   = "OrderCancelled"
	suffixQtyUpdated    = "QuantityUpdated"

	typeURLCommandA   = "type.test/CommandA"
	typeURLCommandB   = "type.test/CommandB"
	typeURLUnknown    = "type.test/UnknownCommand"
	typeURLCreate     = "type.test/Create"
	typeURLFullCreate = "type.examples/examples.CreateOrder"
	typeURLOrderDone  = "type.examples/examples.OrderCompleted"
	typeURLOtherEvent = "type.examples/examples.SomethingElse"
	typeURLQtyUpdated = "type.examples/examples.QuantityUpdated"

	corrID1 = "corr-1"
	corrID2 = "corr-2"

	sagaFulfillment      = "fulfillment"
	sagaTestSaga         = "test-saga"
	sagaInventoryReserve = "inventory-reservation"
)

// ============================================================================
// Helpers
// ============================================================================

type testState struct {
	exists bool
}

func dummyRebuild(events *angzarrpb.EventBook) testState {
	return testState{}
}

func existsRebuild(events *angzarrpb.EventBook) testState {
	return testState{exists: true}
}

func makeContextualCommand(typeURL string, events *angzarrpb.EventBook) *angzarrpb.ContextualCommand {
	return &angzarrpb.ContextualCommand{
		Command: &angzarrpb.CommandBook{
			Cover: &angzarrpb.Cover{Domain: domainTest},
			Pages: []*angzarrpb.CommandPage{
				{Command: &anypb.Any{TypeUrl: typeURL, Value: []byte{}}},
			},
		},
		Events: events,
	}
}

func handlerA(cb *angzarrpb.CommandBook, data []byte, state *testState, seq uint32) (*angzarrpb.EventBook, error) {
	return &angzarrpb.EventBook{
		Pages: []*angzarrpb.EventPage{
			{Event: &anypb.Any{TypeUrl: fmt.Sprintf("handled_a:seq=%d", seq), Value: []byte{}}},
		},
	}, nil
}

func handlerB(_ *angzarrpb.CommandBook, _ []byte, _ *testState, _ uint32) (*angzarrpb.EventBook, error) {
	return &angzarrpb.EventBook{}, nil
}

func handlerReject(_ *angzarrpb.CommandBook, _ []byte, state *testState, _ uint32) (*angzarrpb.EventBook, error) {
	if state.exists {
		return nil, fmt.Errorf("already exists")
	}
	return &angzarrpb.EventBook{}, nil
}

// ============================================================================
// CommandRouter tests
// ============================================================================

func TestCommandRouterDispatchesCorrectHandler(t *testing.T) {
	router := NewCommandRouter(domainTest, dummyRebuild).
		On(suffixCommandA, handlerA).
		On(suffixCommandB, handlerB)

	cmd := makeContextualCommand(typeURLCommandA, nil)

	resp, err := router.Dispatch(cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	events := resp.GetEvents()
	if events == nil || len(events.Pages) != 1 {
		t.Fatal("expected 1 event page")
	}
	if events.Pages[0].Event.TypeUrl != "handled_a:seq=0" {
		t.Errorf("expected 'handled_a:seq=0', got %q", events.Pages[0].Event.TypeUrl)
	}
}

func TestCommandRouterDispatchesSecondHandler(t *testing.T) {
	router := NewCommandRouter(domainTest, dummyRebuild).
		On(suffixCommandA, handlerA).
		On(suffixCommandB, handlerB)

	cmd := makeContextualCommand(typeURLCommandB, nil)

	resp, err := router.Dispatch(cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	events := resp.GetEvents()
	if events == nil {
		t.Fatal("expected events in response")
	}
}

func TestCommandRouterRebuildReceivesPriorEvents(t *testing.T) {
	priorEvents := &angzarrpb.EventBook{
		Pages: []*angzarrpb.EventPage{
			{Event: &anypb.Any{TypeUrl: "event1"}},
			{Event: &anypb.Any{TypeUrl: "event2"}},
			{Event: &anypb.Any{TypeUrl: "event3"}},
		},
	}

	cmd := makeContextualCommand(typeURLCommandA, priorEvents)

	resp, err := NewCommandRouter(domainTest, dummyRebuild).
		On(suffixCommandA, handlerA).
		Dispatch(cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	events := resp.GetEvents()
	if events.Pages[0].Event.TypeUrl != "handled_a:seq=3" {
		t.Errorf("expected seq=3 (from 3 prior events), got %q", events.Pages[0].Event.TypeUrl)
	}
}

func TestCommandRouterUnknownCommand(t *testing.T) {
	router := NewCommandRouter(domainTest, dummyRebuild).
		On(suffixCommandA, handlerA)

	cmd := makeContextualCommand(typeURLUnknown, nil)

	_, err := router.Dispatch(cmd)
	if err == nil {
		t.Fatal("expected error for unknown command")
	}
	want := fmt.Sprintf("%s: %s", ErrMsgUnknownCommand, typeURLUnknown)
	if err.Error() != want {
		t.Errorf("expected error %q, got %q", want, err.Error())
	}
}

func TestCommandRouterHandlerError(t *testing.T) {
	router := NewCommandRouter(domainTest, existsRebuild).
		On(suffixCreate, handlerReject)

	cmd := makeContextualCommand(typeURLCreate, nil)

	_, err := router.Dispatch(cmd)
	if err == nil {
		t.Fatal("expected error from handler")
	}
	if err.Error() != "already exists" {
		t.Errorf("expected 'already exists', got %q", err.Error())
	}
}

func TestCommandRouterNoCommandPages(t *testing.T) {
	router := NewCommandRouter(domainTest, dummyRebuild).
		On(suffixCommandA, handlerA)

	cmd := &angzarrpb.ContextualCommand{
		Command: &angzarrpb.CommandBook{Pages: []*angzarrpb.CommandPage{}},
	}

	_, err := router.Dispatch(cmd)
	if err == nil {
		t.Fatal("expected error for empty pages")
	}
	if err.Error() != ErrMsgNoCommandPages {
		t.Errorf("expected %q, got %q", ErrMsgNoCommandPages, err.Error())
	}
}

func TestCommandRouterSuffixMatching(t *testing.T) {
	router := NewCommandRouter(domainTest, dummyRebuild).
		On(suffixCreateOrder, handlerA)

	cmd := makeContextualCommand(typeURLFullCreate, nil)

	resp, err := router.Dispatch(cmd)
	if err != nil {
		t.Fatalf("suffix match failed: %v", err)
	}
	if resp == nil {
		t.Fatal("expected non-nil response")
	}
}

func TestCommandRouterDomain(t *testing.T) {
	router := NewCommandRouter(domainOrder, dummyRebuild)
	if router.Domain() != domainOrder {
		t.Errorf("expected domain %q, got %q", domainOrder, router.Domain())
	}
}

func TestCommandRouterTypes(t *testing.T) {
	router := NewCommandRouter(domainOrder, dummyRebuild).
		On(suffixCreateOrder, handlerA).
		On(suffixCancelOrder, handlerB)

	types := router.Types()
	if len(types) != 2 {
		t.Fatalf("expected 2 types, got %d", len(types))
	}
	if types[0] != suffixCreateOrder {
		t.Errorf("expected types[0]=%q, got %q", suffixCreateOrder, types[0])
	}
	if types[1] != suffixCancelOrder {
		t.Errorf("expected types[1]=%q, got %q", suffixCancelOrder, types[1])
	}
}

func TestCommandRouterDescriptor(t *testing.T) {
	router := NewCommandRouter(domainOrder, dummyRebuild).
		On(suffixCreateOrder, handlerA).
		On(suffixCancelOrder, handlerB)

	desc := router.Descriptor()
	if desc.Name != domainOrder {
		t.Errorf("expected name %q, got %q", domainOrder, desc.Name)
	}
	if desc.ComponentType != ComponentAggregate {
		t.Errorf("expected component_type %q, got %q", ComponentAggregate, desc.ComponentType)
	}
	if len(desc.Inputs) != 1 {
		t.Fatalf("expected 1 input, got %d", len(desc.Inputs))
	}
	if desc.Inputs[0].Domain != domainOrder {
		t.Errorf("expected input domain %q, got %q", domainOrder, desc.Inputs[0].Domain)
	}
	if len(desc.Inputs[0].EventTypes) != 2 {
		t.Fatalf("expected 2 event types, got %d", len(desc.Inputs[0].EventTypes))
	}
}

// ============================================================================
// NextSequence tests
// ============================================================================

func TestNextSequenceNilEvents(t *testing.T) {
	if NextSequence(nil) != 0 {
		t.Error("expected 0 for nil events")
	}
}

func TestNextSequenceEmptyPages(t *testing.T) {
	if NextSequence(&angzarrpb.EventBook{}) != 0 {
		t.Error("expected 0 for empty pages")
	}
}

func TestNextSequenceWithPages(t *testing.T) {
	events := &angzarrpb.EventBook{
		Pages: []*angzarrpb.EventPage{{}, {}, {}},
	}
	if NextSequence(events) != 3 {
		t.Errorf("expected 3, got %d", NextSequence(events))
	}
}

// ============================================================================
// EventRouter tests
// ============================================================================

func sagaHandler(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	return []*angzarrpb.CommandBook{
		{
			Cover: &angzarrpb.Cover{
				Domain:        domainFulfillment,
				Root:          root,
				CorrelationId: correlationID,
			},
		},
	}
}

func multiCommandHandler(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	return []*angzarrpb.CommandBook{
		{Cover: &angzarrpb.Cover{Domain: domainInventory, Root: root, CorrelationId: correlationID}},
		{Cover: &angzarrpb.Cover{Domain: domainInventory, Root: root, CorrelationId: correlationID}},
	}
}

func makeEventBook(typeURL, correlationID string, rootBytes []byte) *angzarrpb.EventBook {
	return &angzarrpb.EventBook{
		Cover: &angzarrpb.Cover{
			Domain:        domainOrder,
			Root:          &angzarrpb.UUID{Value: rootBytes},
			CorrelationId: correlationID,
		},
		Pages: []*angzarrpb.EventPage{
			{Event: &anypb.Any{TypeUrl: typeURL, Value: []byte{1, 2, 3}}},
		},
	}
}

func TestEventRouterDispatches(t *testing.T) {
	router := NewEventRouter(sagaTestSaga, domainOrder).
		Output(domainFulfillment).
		On(suffixOrderComplete, sagaHandler)

	book := makeEventBook(typeURLOrderDone, corrID1, []byte{4, 5, 6})
	commands := router.Dispatch(book)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}
	if commands[0].Cover.Domain != domainFulfillment {
		t.Errorf("expected domain %q, got %q", domainFulfillment, commands[0].Cover.Domain)
	}
	if commands[0].Cover.CorrelationId != corrID1 {
		t.Errorf("expected correlation_id %q, got %q", corrID1, commands[0].Cover.CorrelationId)
	}
	if string(commands[0].Cover.Root.Value) != string([]byte{4, 5, 6}) {
		t.Error("expected root bytes to be passed through")
	}
}

func TestEventRouterSkipsUnmatched(t *testing.T) {
	router := NewEventRouter(sagaTestSaga, domainOrder).
		On(suffixOrderComplete, sagaHandler)

	book := makeEventBook(typeURLOtherEvent, corrID1, []byte{4, 5, 6})
	commands := router.Dispatch(book)

	if len(commands) != 0 {
		t.Errorf("expected 0 commands for unmatched event, got %d", len(commands))
	}
}

func TestEventRouterMultipleCommands(t *testing.T) {
	router := NewEventRouter(sagaInventoryReserve, domainCart).
		Output(domainInventory).
		On(suffixQtyUpdated, multiCommandHandler)

	book := makeEventBook(typeURLQtyUpdated, corrID2, []byte{1, 2})
	commands := router.Dispatch(book)

	if len(commands) != 2 {
		t.Fatalf("expected 2 commands, got %d", len(commands))
	}
}

func TestEventRouterMultiplePages(t *testing.T) {
	router := NewEventRouter(sagaTestSaga, domainOrder).
		On(suffixOrderComplete, sagaHandler)

	book := &angzarrpb.EventBook{
		Cover: &angzarrpb.Cover{Domain: domainOrder, Root: &angzarrpb.UUID{Value: []byte{1}}, CorrelationId: corrID1},
		Pages: []*angzarrpb.EventPage{
			{Event: &anypb.Any{TypeUrl: typeURLOtherEvent}},
			{Event: &anypb.Any{TypeUrl: typeURLOrderDone}},
		},
	}
	commands := router.Dispatch(book)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command (only second page matches), got %d", len(commands))
	}
}

func TestEventRouterMetadata(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		Output(domainFulfillment).
		Output(domainInventory).
		On(suffixOrderComplete, sagaHandler)

	if router.Name() != sagaFulfillment {
		t.Errorf("expected name %q, got %q", sagaFulfillment, router.Name())
	}
	if router.InputDomain() != domainOrder {
		t.Errorf("expected input domain %q, got %q", domainOrder, router.InputDomain())
	}
	if len(router.OutputDomains()) != 2 {
		t.Fatalf("expected 2 output domains, got %d", len(router.OutputDomains()))
	}
	if router.OutputDomains()[0] != domainFulfillment {
		t.Errorf("expected output[0]=%q, got %q", domainFulfillment, router.OutputDomains()[0])
	}
}

func TestEventRouterTypes(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		On(suffixOrderComplete, sagaHandler).
		On(suffixOrderCancel, sagaHandler)

	types := router.Types()
	if len(types) != 2 {
		t.Fatalf("expected 2 types, got %d", len(types))
	}
	if types[0] != suffixOrderComplete {
		t.Errorf("expected types[0]=%q, got %q", suffixOrderComplete, types[0])
	}
	if types[1] != suffixOrderCancel {
		t.Errorf("expected types[1]=%q, got %q", suffixOrderCancel, types[1])
	}
}

func TestEventRouterDescriptor(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		Output(domainFulfillment).
		On(suffixOrderComplete, sagaHandler)

	desc := router.Descriptor()
	if desc.Name != sagaFulfillment {
		t.Errorf("expected name %q, got %q", sagaFulfillment, desc.Name)
	}
	if desc.ComponentType != ComponentSaga {
		t.Errorf("expected component_type %q, got %q", ComponentSaga, desc.ComponentType)
	}
	if len(desc.Inputs) != 1 {
		t.Fatalf("expected 1 input, got %d", len(desc.Inputs))
	}
	if desc.Inputs[0].Domain != domainOrder {
		t.Errorf("expected input domain %q, got %q", domainOrder, desc.Inputs[0].Domain)
	}
	if len(desc.Inputs[0].EventTypes) != 1 {
		t.Fatalf("expected 1 event type, got %d", len(desc.Inputs[0].EventTypes))
	}
	if desc.Inputs[0].EventTypes[0] != suffixOrderComplete {
		t.Errorf("expected event_types[0]=%q, got %q", suffixOrderComplete, desc.Inputs[0].EventTypes[0])
	}
}
