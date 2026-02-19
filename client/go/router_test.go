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
			On("KnownCommand", func(cb *pb.CommandBook, cmd *anypb.Any, state testState, seq uint32) (*pb.EventBook, error) {
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
			On("TestCommand", func(cb *pb.CommandBook, cmd *anypb.Any, state testState, seq uint32) (*pb.EventBook, error) {
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
		router := NewEventRouter("saga-test").
			Domain("source").
			On("TestEvent", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
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
			On("Event1", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
				callCount++
				return []*pb.CommandBook{{}}, nil
			}).
			On("Event2", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
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
			On("KnownEvent", func(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
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

