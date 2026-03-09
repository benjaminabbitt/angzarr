package angzarr

import (
	"testing"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// Simple test state
type testState struct {
	value int
}

func rebuildTestState(events *pb.EventBook) testState {
	if events == nil {
		return testState{value: 0}
	}
	return testState{value: len(events.Pages)}
}

func TestCommandRouter_Dispatch(t *testing.T) {
	t.Run("dispatches to matching handler", func(t *testing.T) {
		called := false
		// Use fully qualified type name for reflection-based matching
		router := NewCommandRouter("test", rebuildTestState).
			On("examples.TestCommand", func(cb *pb.CommandBook, cmd *anypb.Any, state testState, seq uint32) (*pb.EventBook, error) {
				called = true
				if seq != 0 {
					t.Errorf("expected seq 0, got %d", seq)
				}
				return &pb.EventBook{}, nil
			})

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: &anypb.Any{TypeUrl: "type.googleapis.com/examples.TestCommand"}}},
				},
			},
		}

		_, err := router.Dispatch(cmd)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if !called {
			t.Error("handler not called")
		}
	})

	t.Run("returns error for unknown command", func(t *testing.T) {
		router := NewCommandRouter("test", rebuildTestState).
			On("examples.KnownCommand", func(cb *pb.CommandBook, cmd *anypb.Any, state testState, seq uint32) (*pb.EventBook, error) {
				return &pb.EventBook{}, nil
			})

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: &anypb.Any{TypeUrl: "type.googleapis.com/examples.UnknownCommand"}}},
				},
			},
		}

		_, err := router.Dispatch(cmd)
		if err == nil {
			t.Fatal("expected error for unknown command")
		}
	})

	t.Run("returns error for empty command pages", func(t *testing.T) {
		router := NewCommandRouter("test", rebuildTestState)

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{},
			},
		}

		_, err := router.Dispatch(cmd)
		if err == nil {
			t.Fatal("expected error for empty pages")
		}
	})

	t.Run("rebuilds state from prior events", func(t *testing.T) {
		var capturedState testState
		router := NewCommandRouter("test", rebuildTestState).
			On("examples.TestCommand", func(cb *pb.CommandBook, cmd *anypb.Any, state testState, seq uint32) (*pb.EventBook, error) {
				capturedState = state
				return &pb.EventBook{}, nil
			})

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: &anypb.Any{TypeUrl: "type.googleapis.com/examples.TestCommand"}}},
				},
			},
			Events: &pb.EventBook{
				Pages: []*pb.EventPage{
					{Payload: &pb.EventPage_Event{Event: &anypb.Any{}}},
					{Payload: &pb.EventPage_Event{Event: &anypb.Any{}}},
					{Payload: &pb.EventPage_Event{Event: &anypb.Any{}}},
				}, // 3 prior events
			},
		}

		_, err := router.Dispatch(cmd)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if capturedState.value != 3 {
			t.Errorf("expected state.value=3, got %d", capturedState.value)
		}
	})
}

func TestEventRouter_Dispatch(t *testing.T) {
	t.Run("dispatches to matching handler", func(t *testing.T) {
		called := false
		// Use fully qualified type name for reflection-based matching
		router := NewEventRouter("saga-test").
			Domain("source").
			On("examples.TestEvent", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				called = true
				return []*pb.CommandBook{{}}, nil
			})

		book := &pb.EventBook{
			Cover: &pb.Cover{
				Domain:        "source",
				Root:          &pb.UUID{Value: []byte("test-root")},
				CorrelationId: "corr-123",
			},
			Pages: []*pb.EventPage{
				{Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.TestEvent"}}},
			},
		}

		commands, err := router.Dispatch(book, nil)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if !called {
			t.Error("handler not called")
		}
		if len(commands) != 1 {
			t.Errorf("expected 1 command, got %d", len(commands))
		}
	})

	t.Run("handles multiple events", func(t *testing.T) {
		callCount := 0
		router := NewEventRouter("saga-test").
			Domain("source").
			On("examples.Event1", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				callCount++
				return []*pb.CommandBook{{}}, nil
			}).
			On("examples.Event2", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				callCount++
				return []*pb.CommandBook{{}, {}}, nil
			})

		book := &pb.EventBook{
			Cover: &pb.Cover{Domain: "source"},
			Pages: []*pb.EventPage{
				{Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.Event1"}}},
				{Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.Event2"}}},
			},
		}

		commands, err := router.Dispatch(book, nil)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if callCount != 2 {
			t.Errorf("expected 2 calls, got %d", callCount)
		}
		if len(commands) != 3 {
			t.Errorf("expected 3 commands, got %d", len(commands))
		}
	})

	t.Run("skips unmatched events", func(t *testing.T) {
		router := NewEventRouter("saga-test").
			Domain("source").
			On("examples.KnownEvent", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				return []*pb.CommandBook{{}}, nil
			})

		book := &pb.EventBook{
			Cover: &pb.Cover{Domain: "source"},
			Pages: []*pb.EventPage{
				{Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.UnknownEvent"}}},
			},
		}

		commands, err := router.Dispatch(book, nil)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if len(commands) != 0 {
			t.Errorf("expected 0 commands, got %d", len(commands))
		}
	})
}

// ============================================================================
// Trait-Based Router Tests (Unified Routers)
// ============================================================================

// TestState is a simple state type for testing aggregates.
type TestState struct {
	Value   string
	Counter int
}

// MockCHHandler implements CommandHandlerDomainHandler for testing.
type MockCHHandler struct {
	commandTypes []string
	handleCalls  int
}

func NewMockCHHandler(types ...string) *MockCHHandler {
	return &MockCHHandler{commandTypes: types}
}

func (h *MockCHHandler) CommandTypes() []string {
	return h.commandTypes
}

func (h *MockCHHandler) Rebuild(events *pb.EventBook) *TestState {
	return &TestState{Value: "rebuilt", Counter: len(events.GetPages())}
}

func (h *MockCHHandler) Handle(
	cmd *pb.CommandBook,
	payload *anypb.Any,
	state *TestState,
	seq uint32,
) (*pb.EventBook, error) {
	h.handleCalls++
	return &pb.EventBook{
		Cover: cmd.Cover,
		Pages: []*pb.EventPage{
			{
				Header:  &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: seq}},
				Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "test.TestEvent"}},
			},
		},
	}, nil
}

func (h *MockCHHandler) OnRejected(
	notification *pb.Notification,
	state *TestState,
	targetDomain string,
	targetCommand string,
) (*RejectionHandlerResponse, error) {
	return &RejectionHandlerResponse{}, nil
}

// MockSagaHandler implements SagaDomainHandler for testing.
type MockSagaHandler struct {
	eventTypes   []string
	prepareCalls int
	executeCalls int
}

func NewMockSagaHandler(types ...string) *MockSagaHandler {
	return &MockSagaHandler{eventTypes: types}
}

func (h *MockSagaHandler) EventTypes() []string {
	return h.eventTypes
}

func (h *MockSagaHandler) Prepare(source *pb.EventBook, event *anypb.Any) []*pb.Cover {
	h.prepareCalls++
	return []*pb.Cover{
		{Domain: "destination", Root: &pb.UUID{Value: make([]byte, 16)}},
	}
}

func (h *MockSagaHandler) Execute(
	source *pb.EventBook,
	event *anypb.Any,
	destinations []*pb.EventBook,
) (*SagaHandlerResponse, error) {
	h.executeCalls++
	return &SagaHandlerResponse{
		Commands: []*pb.CommandBook{
			{
				Cover: &pb.Cover{Domain: "destination"},
				Pages: []*pb.CommandPage{
					{
						Header:  &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
						Payload: &pb.CommandPage_Command{Command: &anypb.Any{TypeUrl: "test.TestCommand"}},
					},
				},
			},
		},
	}, nil
}

func (h *MockSagaHandler) OnRejected(
	notification *pb.Notification,
	targetDomain string,
	targetCommand string,
) (*RejectionHandlerResponse, error) {
	return &RejectionHandlerResponse{}, nil
}

// MockPMHandler implements ProcessManagerDomainHandler for testing.
type MockPMHandler struct {
	eventTypes   []string
	prepareCalls int
	handleCalls  int
}

func NewMockPMHandler(types ...string) *MockPMHandler {
	return &MockPMHandler{eventTypes: types}
}

func (h *MockPMHandler) EventTypes() []string {
	return h.eventTypes
}

func (h *MockPMHandler) Prepare(trigger *pb.EventBook, state *TestState, event *anypb.Any) []*pb.Cover {
	h.prepareCalls++
	return nil
}

func (h *MockPMHandler) Handle(
	trigger *pb.EventBook,
	state *TestState,
	event *anypb.Any,
	destinations []*pb.EventBook,
) (*ProcessManagerResponse, error) {
	h.handleCalls++
	return &ProcessManagerResponse{
		Commands: []*pb.CommandBook{
			{
				Cover: &pb.Cover{Domain: "target"},
				Pages: []*pb.CommandPage{
					{
						Header:  &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
						Payload: &pb.CommandPage_Command{Command: &anypb.Any{TypeUrl: "test.PMCommand"}},
					},
				},
			},
		},
	}, nil
}

func (h *MockPMHandler) OnRejected(
	notification *pb.Notification,
	state *TestState,
	targetDomain string,
	targetCommand string,
) (*RejectionHandlerResponse, error) {
	return &RejectionHandlerResponse{}, nil
}

// MockProjectorHandler implements ProjectorDomainHandler for testing.
type MockProjectorHandler struct {
	eventTypes   []string
	projectCalls int
}

func NewMockProjectorHandler(types ...string) *MockProjectorHandler {
	return &MockProjectorHandler{eventTypes: types}
}

func (h *MockProjectorHandler) EventTypes() []string {
	return h.eventTypes
}

func (h *MockProjectorHandler) Project(events *pb.EventBook) (*pb.Projection, error) {
	h.projectCalls++
	return &pb.Projection{
		Cover:     events.Cover,
		Projector: "test-projector",
		Sequence:  uint32(len(events.GetPages())),
	}, nil
}

// ============================================================================
// CommandHandlerRouter Tests
// ============================================================================

func TestCommandHandlerRouterCreation(t *testing.T) {
	handler := NewMockCHHandler("test.CreateThing", "test.UpdateThing")
	router := NewCommandHandlerRouter("test-ch", "things", handler)

	if router.Name() != "test-ch" {
		t.Errorf("expected name 'test-ch', got '%s'", router.Name())
	}

	if router.Domain() != "things" {
		t.Errorf("expected domain 'things', got '%s'", router.Domain())
	}

	types := router.CommandTypes()
	if len(types) != 2 {
		t.Errorf("expected 2 command types, got %d", len(types))
	}
}

func TestCommandHandlerRouterSubscriptions(t *testing.T) {
	handler := NewMockCHHandler("test.CreateThing", "test.UpdateThing")
	router := NewCommandHandlerRouter("test-ch", "things", handler)

	subs := router.Subscriptions()
	if len(subs) != 1 {
		t.Errorf("expected 1 subscription domain, got %d", len(subs))
	}

	if types, ok := subs["things"]; !ok {
		t.Error("expected 'things' domain in subscriptions")
	} else if len(types) != 2 {
		t.Errorf("expected 2 types for 'things' domain, got %d", len(types))
	}
}

func TestCommandHandlerRouterRebuildState(t *testing.T) {
	handler := NewMockCHHandler("test.CreateThing")
	router := NewCommandHandlerRouter[*TestState]("test-ch", "things", handler)

	events := &pb.EventBook{
		Pages: []*pb.EventPage{
			{Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}}},
			{Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 1}}},
		},
	}

	state := router.RebuildState(events)
	if state.Value != "rebuilt" {
		t.Errorf("expected state.Value 'rebuilt', got '%s'", state.Value)
	}
	if state.Counter != 2 {
		t.Errorf("expected state.Counter 2, got %d", state.Counter)
	}
}

func TestCommandHandlerRouterDispatch(t *testing.T) {
	handler := NewMockCHHandler("test.CreateThing")
	router := NewCommandHandlerRouter[*TestState]("test-ch", "things", handler)

	cmd := &pb.ContextualCommand{
		Command: &pb.CommandBook{
			Cover: &pb.Cover{Domain: "things"},
			Pages: []*pb.CommandPage{
				{
					Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
					Payload: &pb.CommandPage_Command{
						Command: &anypb.Any{TypeUrl: "type.googleapis.com/test.CreateThing"},
					},
				},
			},
		},
		Events: &pb.EventBook{NextSequence: 5},
	}

	resp, err := router.Dispatch(cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if resp == nil {
		t.Fatal("expected non-nil response")
	}

	events := resp.GetEvents()
	if events == nil {
		t.Fatal("expected events in response")
	}

	if handler.handleCalls != 1 {
		t.Errorf("expected 1 handle call, got %d", handler.handleCalls)
	}
}

// ============================================================================
// SagaRouter Tests
// ============================================================================

func TestSagaRouterCreation(t *testing.T) {
	handler := NewMockSagaHandler("test.OrderCreated")
	router := NewSagaRouter("saga-order-fulfillment", "order", handler)

	if router.Name() != "saga-order-fulfillment" {
		t.Errorf("expected name 'saga-order-fulfillment', got '%s'", router.Name())
	}

	if router.InputDomain() != "order" {
		t.Errorf("expected input domain 'order', got '%s'", router.InputDomain())
	}
}

func TestSagaRouterSubscriptions(t *testing.T) {
	handler := NewMockSagaHandler("test.OrderCreated", "test.OrderCancelled")
	router := NewSagaRouter("saga-order-fulfillment", "order", handler)

	subs := router.Subscriptions()
	if len(subs) != 1 {
		t.Errorf("expected 1 subscription domain, got %d", len(subs))
	}

	if types, ok := subs["order"]; !ok {
		t.Error("expected 'order' domain in subscriptions")
	} else if len(types) != 2 {
		t.Errorf("expected 2 types for 'order' domain, got %d", len(types))
	}
}

func TestSagaRouterPrepareDestinations(t *testing.T) {
	handler := NewMockSagaHandler("test.OrderCreated")
	router := NewSagaRouter("saga-order-fulfillment", "order", handler)

	source := &pb.EventBook{
		Cover: &pb.Cover{Domain: "order"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.OrderCreated"},
				},
			},
		},
	}

	covers := router.PrepareDestinations(source)
	if len(covers) != 1 {
		t.Errorf("expected 1 cover, got %d", len(covers))
	}

	if handler.prepareCalls != 1 {
		t.Errorf("expected 1 prepare call, got %d", handler.prepareCalls)
	}
}

func TestSagaRouterDispatch(t *testing.T) {
	handler := NewMockSagaHandler("test.OrderCreated")
	router := NewSagaRouter("saga-order-fulfillment", "order", handler)

	source := &pb.EventBook{
		Cover: &pb.Cover{Domain: "order"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.OrderCreated"},
				},
			},
		},
	}

	resp, err := router.Dispatch(source, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if resp == nil {
		t.Fatal("expected non-nil response")
	}

	if len(resp.Commands) != 1 {
		t.Errorf("expected 1 command, got %d", len(resp.Commands))
	}

	if handler.executeCalls != 1 {
		t.Errorf("expected 1 execute call, got %d", handler.executeCalls)
	}
}

// ============================================================================
// ProcessManagerRouter Tests
// ============================================================================

func TestProcessManagerRouterCreation(t *testing.T) {
	rebuild := func(events *pb.EventBook) *TestState {
		return &TestState{Value: "pm-state"}
	}
	router := NewProcessManagerRouter[*TestState]("pmg-order-flow", "order-flow", rebuild)

	if router.Name() != "pmg-order-flow" {
		t.Errorf("expected name 'pmg-order-flow', got '%s'", router.Name())
	}

	if router.PMDomain() != "order-flow" {
		t.Errorf("expected PM domain 'order-flow', got '%s'", router.PMDomain())
	}
}

func TestProcessManagerRouterMultiDomain(t *testing.T) {
	orderHandler := NewMockPMHandler("test.OrderCreated")
	inventoryHandler := NewMockPMHandler("test.StockReserved")

	rebuild := func(events *pb.EventBook) *TestState {
		return &TestState{}
	}

	router := NewProcessManagerRouter[*TestState]("pmg-order-flow", "order-flow", rebuild).
		Domain("order", orderHandler).
		Domain("inventory", inventoryHandler)

	subs := router.Subscriptions()
	if len(subs) != 2 {
		t.Errorf("expected 2 subscription domains, got %d", len(subs))
	}

	if _, ok := subs["order"]; !ok {
		t.Error("expected 'order' domain in subscriptions")
	}

	if _, ok := subs["inventory"]; !ok {
		t.Error("expected 'inventory' domain in subscriptions")
	}
}

func TestProcessManagerRouterDispatch(t *testing.T) {
	handler := NewMockPMHandler("test.OrderCreated")

	rebuild := func(events *pb.EventBook) *TestState {
		return &TestState{Value: "rebuilt"}
	}

	router := NewProcessManagerRouter[*TestState]("pmg-order-flow", "order-flow", rebuild).
		Domain("order", handler)

	trigger := &pb.EventBook{
		Cover: &pb.Cover{Domain: "order"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.OrderCreated"},
				},
			},
		},
	}

	resp, err := router.Dispatch(trigger, nil, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if resp == nil {
		t.Fatal("expected non-nil response")
	}

	if len(resp.Commands) != 1 {
		t.Errorf("expected 1 command, got %d", len(resp.Commands))
	}

	if handler.handleCalls != 1 {
		t.Errorf("expected 1 handle call, got %d", handler.handleCalls)
	}
}

// ============================================================================
// ProjectorRouter Tests
// ============================================================================

func TestProjectorRouterCreation(t *testing.T) {
	router := NewProjectorRouter("prj-output")

	if router.Name() != "prj-output" {
		t.Errorf("expected name 'prj-output', got '%s'", router.Name())
	}
}

func TestProjectorRouterMultiDomain(t *testing.T) {
	playerHandler := NewMockProjectorHandler("test.PlayerRegistered")
	handHandler := NewMockProjectorHandler("test.CardsDealt")

	router := NewProjectorRouter("prj-output").
		Domain("player", playerHandler).
		Domain("hand", handHandler)

	subs := router.Subscriptions()
	if len(subs) != 2 {
		t.Errorf("expected 2 subscription domains, got %d", len(subs))
	}

	if _, ok := subs["player"]; !ok {
		t.Error("expected 'player' domain in subscriptions")
	}

	if _, ok := subs["hand"]; !ok {
		t.Error("expected 'hand' domain in subscriptions")
	}
}

func TestProjectorRouterDispatch(t *testing.T) {
	handler := NewMockProjectorHandler("test.PlayerRegistered")

	router := NewProjectorRouter("prj-output").
		Domain("player", handler)

	events := &pb.EventBook{
		Cover: &pb.Cover{Domain: "player"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.PlayerRegistered"},
				},
			},
		},
	}

	projection, err := router.Dispatch(events)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if projection == nil {
		t.Fatal("expected non-nil projection")
	}

	if projection.Projector != "test-projector" {
		t.Errorf("expected projector 'test-projector', got '%s'", projection.Projector)
	}

	if handler.projectCalls != 1 {
		t.Errorf("expected 1 project call, got %d", handler.projectCalls)
	}
}

// ============================================================================
// Error Handling Tests
// ============================================================================

func TestCommandHandlerRouterDispatchMissingCommand(t *testing.T) {
	handler := NewMockCHHandler("test.CreateThing")
	router := NewCommandHandlerRouter[*TestState]("test-ch", "things", handler)

	cmd := &pb.ContextualCommand{
		Command: nil,
	}

	_, err := router.Dispatch(cmd)
	if err == nil {
		t.Error("expected error for missing command book")
	}
}

func TestSagaRouterDispatchEmptySource(t *testing.T) {
	handler := NewMockSagaHandler("test.OrderCreated")
	router := NewSagaRouter("saga-order-fulfillment", "order", handler)

	_, err := router.Dispatch(&pb.EventBook{}, nil)
	if err == nil {
		t.Error("expected error for empty source")
	}
}

func TestProcessManagerRouterDispatchUnknownDomain(t *testing.T) {
	rebuild := func(events *pb.EventBook) *TestState {
		return &TestState{}
	}

	router := NewProcessManagerRouter[*TestState]("pmg-test", "test-flow", rebuild)
	// No handlers registered

	trigger := &pb.EventBook{
		Cover: &pb.Cover{Domain: "unknown"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.Event"},
				},
			},
		},
	}

	_, err := router.Dispatch(trigger, nil, nil)
	if err == nil {
		t.Error("expected error for unknown domain")
	}
}

func TestProjectorRouterDispatchUnknownDomain(t *testing.T) {
	router := NewProjectorRouter("prj-test")
	// No handlers registered

	events := &pb.EventBook{
		Cover: &pb.Cover{Domain: "unknown"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.Event"},
				},
			},
		},
	}

	_, err := router.Dispatch(events)
	if err == nil {
		t.Error("expected error for unknown domain")
	}
}

// ============================================================================
// Notification Handling Tests
// ============================================================================

// MockCHHandlerWithRejection tracks rejection handler calls.
type MockCHHandlerWithRejection struct {
	*MockCHHandler
	onRejectedCalls  int
	lastTargetDomain string
	lastTargetCmd    string
	returnEvents     *pb.EventBook
}

func NewMockCHHandlerWithRejection(types ...string) *MockCHHandlerWithRejection {
	return &MockCHHandlerWithRejection{
		MockCHHandler: NewMockCHHandler(types...),
	}
}

func (h *MockCHHandlerWithRejection) OnRejected(
	notification *pb.Notification,
	state *TestState,
	targetDomain string,
	targetCommand string,
) (*RejectionHandlerResponse, error) {
	h.onRejectedCalls++
	h.lastTargetDomain = targetDomain
	h.lastTargetCmd = targetCommand
	if h.returnEvents != nil {
		return &RejectionHandlerResponse{Events: h.returnEvents}, nil
	}
	return &RejectionHandlerResponse{}, nil
}

func TestCommandHandlerRouterDispatchNotification(t *testing.T) {
	handler := NewMockCHHandlerWithRejection("test.CreateThing")
	handler.returnEvents = &pb.EventBook{
		Cover: &pb.Cover{Domain: "things"},
		Pages: []*pb.EventPage{
			{Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}}, Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "test.Compensated"}}},
		},
	}
	router := NewCommandHandlerRouter[*TestState]("test-ch", "things", handler)

	// Create a Notification command
	notification := &pb.Notification{}
	notificationBytes, _ := proto.Marshal(notification)
	notificationAny := &anypb.Any{
		TypeUrl: "type.googleapis.com/angzarr.Notification",
		Value:   notificationBytes,
	}

	cmd := &pb.ContextualCommand{
		Command: &pb.CommandBook{
			Cover: &pb.Cover{Domain: "things"},
			Pages: []*pb.CommandPage{
				{
					Header:  &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
					Payload: &pb.CommandPage_Command{Command: notificationAny},
				},
			},
		},
		Events: &pb.EventBook{NextSequence: 5},
	}

	resp, err := router.Dispatch(cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if resp == nil {
		t.Fatal("expected non-nil response")
	}

	// Should have called OnRejected
	if handler.onRejectedCalls != 1 {
		t.Errorf("expected 1 OnRejected call, got %d", handler.onRejectedCalls)
	}

	// Should return events
	events := resp.GetEvents()
	if events == nil {
		t.Fatal("expected events in response for compensation")
	}
}

func TestCommandHandlerRouterDispatchNotificationReturnsRevocation(t *testing.T) {
	handler := NewMockCHHandlerWithRejection("test.CreateThing")
	// returnEvents is nil, so OnRejected returns empty response
	router := NewCommandHandlerRouter[*TestState]("test-ch", "things", handler)

	notification := &pb.Notification{}
	notificationBytes, _ := proto.Marshal(notification)
	notificationAny := &anypb.Any{
		TypeUrl: "type.googleapis.com/angzarr.Notification",
		Value:   notificationBytes,
	}

	cmd := &pb.ContextualCommand{
		Command: &pb.CommandBook{
			Cover: &pb.Cover{Domain: "things"},
			Pages: []*pb.CommandPage{
				{
					Header:  &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
					Payload: &pb.CommandPage_Command{Command: notificationAny},
				},
			},
		},
		Events: &pb.EventBook{NextSequence: 5},
	}

	resp, err := router.Dispatch(cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Should return revocation response when handler returns empty
	revocation := resp.GetRevocation()
	if revocation == nil {
		t.Fatal("expected revocation response for empty handler response")
	}

	if !revocation.EmitSystemRevocation {
		t.Error("expected EmitSystemRevocation to be true")
	}
}

// MockPMHandlerWithRejection tracks PM rejection handler calls.
type MockPMHandlerWithRejection struct {
	*MockPMHandler
	onRejectedCalls  int
	lastTargetDomain string
	lastTargetCmd    string
	returnEvents     *pb.EventBook
}

func NewMockPMHandlerWithRejection(types ...string) *MockPMHandlerWithRejection {
	return &MockPMHandlerWithRejection{
		MockPMHandler: NewMockPMHandler(types...),
	}
}

func (h *MockPMHandlerWithRejection) OnRejected(
	notification *pb.Notification,
	state *TestState,
	targetDomain string,
	targetCommand string,
) (*RejectionHandlerResponse, error) {
	h.onRejectedCalls++
	h.lastTargetDomain = targetDomain
	h.lastTargetCmd = targetCommand
	if h.returnEvents != nil {
		return &RejectionHandlerResponse{Events: h.returnEvents}, nil
	}
	return &RejectionHandlerResponse{}, nil
}

func TestProcessManagerRouterDispatchNotification(t *testing.T) {
	handler := NewMockPMHandlerWithRejection("test.OrderCreated")
	handler.returnEvents = &pb.EventBook{
		Cover: &pb.Cover{Domain: "order-flow"},
		Pages: []*pb.EventPage{
			{Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}}, Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "test.FlowCompensated"}}},
		},
	}

	rebuild := func(events *pb.EventBook) *TestState {
		return &TestState{Value: "rebuilt"}
	}

	router := NewProcessManagerRouter[*TestState]("pmg-order-flow", "order-flow", rebuild).
		Domain("order", handler)

	// Create a Notification event
	notification := &pb.Notification{}
	notificationBytes, _ := proto.Marshal(notification)
	notificationAny := &anypb.Any{
		TypeUrl: "type.googleapis.com/angzarr.Notification",
		Value:   notificationBytes,
	}

	trigger := &pb.EventBook{
		Cover: &pb.Cover{Domain: "order"},
		Pages: []*pb.EventPage{
			{
				Header:  &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{Event: notificationAny},
			},
		},
	}

	resp, err := router.Dispatch(trigger, &pb.EventBook{}, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if resp == nil {
		t.Fatal("expected non-nil response")
	}

	// Should have called OnRejected
	if handler.onRejectedCalls != 1 {
		t.Errorf("expected 1 OnRejected call, got %d", handler.onRejectedCalls)
	}

	// Should return process events
	if resp.ProcessEvents == nil {
		t.Fatal("expected process events in response for PM compensation")
	}
}

// ============================================================================
// PrepareDestinations Tests
// ============================================================================

func TestProcessManagerRouterPrepareDestinations(t *testing.T) {
	handler := NewMockPMHandler("test.OrderCreated")

	rebuild := func(events *pb.EventBook) *TestState {
		return &TestState{Value: "rebuilt"}
	}

	router := NewProcessManagerRouter[*TestState]("pmg-order-flow", "order-flow", rebuild).
		Domain("order", handler)

	trigger := &pb.EventBook{
		Cover: &pb.Cover{Domain: "order"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.OrderCreated"},
				},
			},
		},
	}

	processState := &pb.EventBook{
		Cover: &pb.Cover{Domain: "order-flow"},
		Pages: []*pb.EventPage{},
	}

	covers := router.PrepareDestinations(trigger, processState)

	// MockPMHandler.Prepare returns nil by default
	if covers != nil && len(covers) > 0 {
		t.Errorf("expected empty covers from mock, got %d", len(covers))
	}

	// Verify Prepare was called
	if handler.prepareCalls != 1 {
		t.Errorf("expected 1 prepare call, got %d", handler.prepareCalls)
	}
}

func TestProcessManagerRouterPrepareDestinationsNilTrigger(t *testing.T) {
	rebuild := func(events *pb.EventBook) *TestState {
		return &TestState{}
	}

	router := NewProcessManagerRouter[*TestState]("pmg-test", "test-flow", rebuild)

	covers := router.PrepareDestinations(nil, nil)

	if covers != nil {
		t.Error("expected nil covers for nil trigger")
	}
}

func TestProcessManagerRouterPrepareDestinationsUnknownDomain(t *testing.T) {
	rebuild := func(events *pb.EventBook) *TestState {
		return &TestState{}
	}

	router := NewProcessManagerRouter[*TestState]("pmg-test", "test-flow", rebuild)
	// No handlers registered

	trigger := &pb.EventBook{
		Cover: &pb.Cover{Domain: "unknown"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.Event"},
				},
			},
		},
	}

	covers := router.PrepareDestinations(trigger, nil)

	if covers != nil {
		t.Error("expected nil covers for unknown domain")
	}
}

// ============================================================================
// Edge Case Tests
// ============================================================================

func TestSagaRouterPrepareDestinationsNilSource(t *testing.T) {
	handler := NewMockSagaHandler("test.OrderCreated")
	router := NewSagaRouter("saga-order-fulfillment", "order", handler)

	covers := router.PrepareDestinations(nil)

	if covers != nil {
		t.Error("expected nil covers for nil source")
	}

	if handler.prepareCalls != 0 {
		t.Errorf("expected 0 prepare calls for nil source, got %d", handler.prepareCalls)
	}
}

func TestSagaRouterPrepareDestinationsEmptyPages(t *testing.T) {
	handler := NewMockSagaHandler("test.OrderCreated")
	router := NewSagaRouter("saga-order-fulfillment", "order", handler)

	source := &pb.EventBook{
		Cover: &pb.Cover{Domain: "order"},
		Pages: []*pb.EventPage{}, // Empty pages
	}

	covers := router.PrepareDestinations(source)

	if covers != nil {
		t.Error("expected nil covers for empty pages")
	}

	if handler.prepareCalls != 0 {
		t.Errorf("expected 0 prepare calls for empty pages, got %d", handler.prepareCalls)
	}
}

func TestCommandHandlerRouterDispatchNoPages(t *testing.T) {
	handler := NewMockCHHandler("test.CreateThing")
	router := NewCommandHandlerRouter[*TestState]("test-ch", "things", handler)

	cmd := &pb.ContextualCommand{
		Command: &pb.CommandBook{
			Cover: &pb.Cover{Domain: "things"},
			Pages: []*pb.CommandPage{}, // Empty pages
		},
		Events: &pb.EventBook{NextSequence: 0},
	}

	_, err := router.Dispatch(cmd)
	if err == nil {
		t.Error("expected error for empty command pages")
	}
}

func TestCommandHandlerRouterDispatchNilEvents(t *testing.T) {
	handler := NewMockCHHandler("test.CreateThing")
	router := NewCommandHandlerRouter[*TestState]("test-ch", "things", handler)

	cmd := &pb.ContextualCommand{
		Command: &pb.CommandBook{
			Cover: &pb.Cover{Domain: "things"},
			Pages: []*pb.CommandPage{
				{
					Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
					Payload: &pb.CommandPage_Command{
						Command: &anypb.Any{TypeUrl: "type.googleapis.com/test.CreateThing"},
					},
				},
			},
		},
		Events: nil, // Nil events
	}

	// Should still work - events initialized to empty EventBook
	resp, err := router.Dispatch(cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if resp == nil {
		t.Fatal("expected non-nil response")
	}
}

func TestProjectorRouterDispatchNilCover(t *testing.T) {
	handler := NewMockProjectorHandler("test.PlayerRegistered")
	router := NewProjectorRouter("prj-output").
		Domain("player", handler)

	events := &pb.EventBook{
		Cover: nil, // Nil cover
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.PlayerRegistered"},
				},
			},
		},
	}

	_, err := router.Dispatch(events)
	if err == nil {
		t.Error("expected error for nil cover")
	}
}
