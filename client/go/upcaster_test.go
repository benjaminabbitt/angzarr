package angzarr

import (
	"testing"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
)

func TestUpcasterRouter_New(t *testing.T) {
	router := NewUpcasterRouter("order")

	if router.Domain() != "order" {
		t.Errorf("expected domain 'order', got '%s'", router.Domain())
	}
}

func TestUpcasterRouter_On_Chains(t *testing.T) {
	// Just verify chaining works
	_ = NewUpcasterRouter("order").
		On("OrderCreatedV1", func(old *anypb.Any) *anypb.Any { return old }).
		On("OrderShippedV1", func(old *anypb.Any) *anypb.Any { return old })
}

func TestUpcasterRouter_Upcast_TransformsMatching(t *testing.T) {
	router := NewUpcasterRouter("order").
		On("TestEventV1", func(old *anypb.Any) *anypb.Any {
			return &anypb.Any{
				TypeUrl: "type.googleapis.com/test.TestEventV2",
				Value:   []byte{1, 2, 3},
			}
		})

	oldEvents := []*pb.EventPage{
		{
			Payload: &pb.EventPage_Event{Event: &anypb.Any{
				TypeUrl: "type.googleapis.com/test.TestEventV1",
				Value:   []byte{},
			}},
		},
	}

	newEvents := router.Upcast(oldEvents)

	if len(newEvents) != 1 {
		t.Fatalf("expected 1 event, got %d", len(newEvents))
	}

	event := newEvents[0].GetEvent()
	if event == nil {
		t.Fatal("expected event payload")
	}

	if event.TypeUrl != "type.googleapis.com/test.TestEventV2" {
		t.Errorf("expected V2 type_url, got '%s'", event.TypeUrl)
	}

	if len(event.Value) != 3 {
		t.Errorf("expected value [1,2,3], got %v", event.Value)
	}
}

func TestUpcasterRouter_Upcast_PassesThrough(t *testing.T) {
	router := NewUpcasterRouter("order").
		On("OrderCreatedV1", func(old *anypb.Any) *anypb.Any { return old })

	events := []*pb.EventPage{
		{
			Payload: &pb.EventPage_Event{Event: &anypb.Any{
				TypeUrl: "type.googleapis.com/test.OtherEvent",
				Value:   []byte{42},
			}},
		},
	}

	result := router.Upcast(events)

	if len(result) != 1 {
		t.Fatalf("expected 1 event, got %d", len(result))
	}

	event := result[0].GetEvent()
	if event.TypeUrl != "type.googleapis.com/test.OtherEvent" {
		t.Errorf("expected original type_url, got '%s'", event.TypeUrl)
	}
}

func TestUpcasterRouter_Upcast_EmptyInput(t *testing.T) {
	router := NewUpcasterRouter("order")
	result := router.Upcast([]*pb.EventPage{})

	if len(result) != 0 {
		t.Errorf("expected empty result, got %d events", len(result))
	}
}
