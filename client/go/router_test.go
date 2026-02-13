package angzarr

import (
	"testing"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
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
		router := NewCommandRouter("test", rebuildTestState).
			On("TestCommand", func(cb *pb.CommandBook, cmd *anypb.Any, state testState, seq uint32) (*pb.EventBook, error) {
				called = true
				if seq != 0 {
					t.Errorf("expected seq 0, got %d", seq)
				}
				return &pb.EventBook{}, nil
			})

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Command: &anypb.Any{TypeUrl: "type.googleapis.com/examples.TestCommand"}},
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
			On("KnownCommand", func(cb *pb.CommandBook, cmd *anypb.Any, state testState, seq uint32) (*pb.EventBook, error) {
				return &pb.EventBook{}, nil
			})

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Command: &anypb.Any{TypeUrl: "type.googleapis.com/examples.UnknownCommand"}},
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
			On("TestCommand", func(cb *pb.CommandBook, cmd *anypb.Any, state testState, seq uint32) (*pb.EventBook, error) {
				capturedState = state
				return &pb.EventBook{}, nil
			})

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Command: &anypb.Any{TypeUrl: "type.googleapis.com/examples.TestCommand"}},
				},
			},
			Events: &pb.EventBook{
				Pages: []*pb.EventPage{{}, {}, {}}, // 3 prior events
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

func TestCommandRouter_Descriptor(t *testing.T) {
	router := NewCommandRouter("cart", rebuildTestState).
		On("CreateCart", nil).
		On("AddItem", nil)

	desc := router.Descriptor()

	if desc.Name != "cart" {
		t.Errorf("expected name 'cart', got %s", desc.Name)
	}
	if desc.ComponentType != ComponentAggregate {
		t.Errorf("expected component type 'aggregate', got %s", desc.ComponentType)
	}
	if len(desc.Inputs) != 1 {
		t.Fatalf("expected 1 input, got %d", len(desc.Inputs))
	}
	if desc.Inputs[0].Domain != "cart" {
		t.Errorf("expected input domain 'cart', got %s", desc.Inputs[0].Domain)
	}
	types := desc.Inputs[0].Types
	if len(types) != 2 {
		t.Fatalf("expected 2 types, got %d", len(types))
	}
	if types[0] != "CreateCart" || types[1] != "AddItem" {
		t.Errorf("unexpected types: %v", types)
	}
}

func TestCommandRouter_Types(t *testing.T) {
	router := NewCommandRouter("test", rebuildTestState).
		On("Type1", nil).
		On("Type2", nil).
		On("Type3", nil)

	types := router.Types()
	if len(types) != 3 {
		t.Fatalf("expected 3 types, got %d", len(types))
	}
	expected := []string{"Type1", "Type2", "Type3"}
	for i, typ := range types {
		if typ != expected[i] {
			t.Errorf("expected type %s at index %d, got %s", expected[i], i, typ)
		}
	}
}

func TestEventRouter_Dispatch(t *testing.T) {
	t.Run("dispatches to matching handler", func(t *testing.T) {
		called := false
		router := NewEventRouter("saga-test", "source").
			Sends("target", "TargetCommand").
			On("TestEvent", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				called = true
				return []*pb.CommandBook{{}}, nil
			})

		book := &pb.EventBook{
			Cover: &pb.Cover{
				Root:          &pb.UUID{Value: []byte("test-root")},
				CorrelationId: "corr-123",
			},
			Pages: []*pb.EventPage{
				{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.TestEvent"}},
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
		router := NewEventRouter("saga-test", "source").
			On("Event1", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				callCount++
				return []*pb.CommandBook{{}}, nil
			}).
			On("Event2", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				callCount++
				return []*pb.CommandBook{{}, {}}, nil
			})

		book := &pb.EventBook{
			Pages: []*pb.EventPage{
				{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.Event1"}},
				{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.Event2"}},
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
		router := NewEventRouter("saga-test", "source").
			On("KnownEvent", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				return []*pb.CommandBook{{}}, nil
			})

		book := &pb.EventBook{
			Pages: []*pb.EventPage{
				{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.UnknownEvent"}},
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

func TestEventRouter_Descriptor(t *testing.T) {
	router := NewEventRouter("saga-order-fulfillment", "order").
		Sends("fulfillment", "CreateShipment").
		On("OrderCompleted", nil).
		On("OrderCancelled", nil)

	desc := router.Descriptor()

	if desc.Name != "saga-order-fulfillment" {
		t.Errorf("expected name 'saga-order-fulfillment', got %s", desc.Name)
	}
	if desc.ComponentType != ComponentSaga {
		t.Errorf("expected component type 'saga', got %s", desc.ComponentType)
	}
	if len(desc.Inputs) != 1 {
		t.Fatalf("expected 1 input, got %d", len(desc.Inputs))
	}
	if desc.Inputs[0].Domain != "order" {
		t.Errorf("expected input domain 'order', got %s", desc.Inputs[0].Domain)
	}
}
