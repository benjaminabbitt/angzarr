package angzarr

import (
	"testing"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// =============================================================================
// RejectionHandlerResponse Tests
// =============================================================================

func TestRejectionHandlerResponse_EmptyResponse(t *testing.T) {
	response := &RejectionHandlerResponse{}

	if response.Events != nil {
		t.Error("empty response should have nil events")
	}
	if response.Notification != nil {
		t.Error("empty response should have nil notification")
	}
}

func TestRejectionHandlerResponse_EventsOnly(t *testing.T) {
	eventBook := &pb.EventBook{
		Pages: []*pb.EventPage{
			{Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.CompensationEvent"}},
		},
	}

	response := &RejectionHandlerResponse{Events: eventBook}

	if response.Events == nil {
		t.Error("response should have events")
	}
	if len(response.Events.Pages) != 1 {
		t.Errorf("expected 1 event page, got %d", len(response.Events.Pages))
	}
	if response.Notification != nil {
		t.Error("response should have nil notification")
	}
}

func TestRejectionHandlerResponse_NotificationOnly(t *testing.T) {
	notification := &pb.Notification{
		Payload: &anypb.Any{TypeUrl: "type.googleapis.com/angzarr.RejectionNotification"},
	}

	response := &RejectionHandlerResponse{Notification: notification}

	if response.Events != nil {
		t.Error("response should have nil events")
	}
	if response.Notification == nil {
		t.Error("response should have notification")
	}
}

func TestRejectionHandlerResponse_BothEventsAndNotification(t *testing.T) {
	eventBook := &pb.EventBook{
		Pages: []*pb.EventPage{
			{Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.CompensationEvent"}},
		},
	}
	notification := &pb.Notification{
		Payload: &anypb.Any{TypeUrl: "type.googleapis.com/angzarr.RejectionNotification"},
	}

	response := &RejectionHandlerResponse{
		Events:       eventBook,
		Notification: notification,
	}

	if response.Events == nil {
		t.Error("response should have events")
	}
	if response.Notification == nil {
		t.Error("response should have notification")
	}
}

// =============================================================================
// Additional RejectionHandlerResponse Tests
// =============================================================================

func TestRejectionHandlerResponse_MultipleEvents(t *testing.T) {
	eventBook := &pb.EventBook{
		Pages: []*pb.EventPage{
			{Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.Event1"}},
			{Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.Event2"}},
		},
	}

	response := &RejectionHandlerResponse{Events: eventBook}

	if response.Events == nil {
		t.Error("response should have events")
	}
	if len(response.Events.Pages) != 2 {
		t.Errorf("expected 2 event pages, got %d", len(response.Events.Pages))
	}
}

func TestRejectionHandlerResponse_NotificationPayloadAccessible(t *testing.T) {
	rejection := &pb.RejectionNotification{
		IssuerName:      "test-saga",
		IssuerType:      "saga",
		RejectionReason: "test reason",
	}
	rejectionBytes, _ := proto.Marshal(rejection)

	notification := &pb.Notification{
		Payload: &anypb.Any{
			TypeUrl: "type.googleapis.com/angzarr.RejectionNotification",
			Value:   rejectionBytes,
		},
	}

	response := &RejectionHandlerResponse{Notification: notification}

	if response.Notification == nil {
		t.Error("response should have notification")
	}
	if response.Notification.Payload == nil {
		t.Error("notification should have payload")
	}
}

// =============================================================================
// Helper Function Tests
// =============================================================================

func TestIsNotification(t *testing.T) {
	tests := []struct {
		typeURL  string
		expected bool
	}{
		{"type.googleapis.com/angzarr.Notification", true},
		{"type.googleapis.com/test.SomeNotification", true},
		{"type.googleapis.com/test.SomeCommand", false},
		{"type.googleapis.com/test.SomeEvent", false},
		{"Notification", true},
		{"NotificationEvent", false},
	}

	for _, tc := range tests {
		result := IsNotification(tc.typeURL)
		if result != tc.expected {
			t.Errorf("IsNotification(%q) = %v, expected %v", tc.typeURL, result, tc.expected)
		}
	}
}

func TestCompensationContext(t *testing.T) {
	rejectedCmd := &pb.CommandBook{
		Cover: &pb.Cover{Domain: "inventory"},
		Pages: []*pb.CommandPage{
			{Command: &anypb.Any{TypeUrl: "type.googleapis.com/test.ReserveStock"}},
		},
	}
	rejection := &pb.RejectionNotification{
		IssuerName:           "saga-order-inventory",
		IssuerType:           "saga",
		SourceEventSequence:  5,
		RejectionReason:      "insufficient stock",
		RejectedCommand:      rejectedCmd,
		SourceAggregate:      &pb.Cover{Domain: "order"},
	}
	rejectionBytes, _ := proto.Marshal(rejection)

	notification := &pb.Notification{
		Payload: &anypb.Any{
			TypeUrl: "type.googleapis.com/angzarr.RejectionNotification",
			Value:   rejectionBytes,
		},
	}

	ctx := NewCompensationContext(notification)

	if ctx.IssuerName != "saga-order-inventory" {
		t.Errorf("expected issuer name 'saga-order-inventory', got %q", ctx.IssuerName)
	}
	if ctx.IssuerType != "saga" {
		t.Errorf("expected issuer type 'saga', got %q", ctx.IssuerType)
	}
	if ctx.SourceEventSequence != 5 {
		t.Errorf("expected source event sequence 5, got %d", ctx.SourceEventSequence)
	}
	if ctx.RejectionReason != "insufficient stock" {
		t.Errorf("expected rejection reason 'insufficient stock', got %q", ctx.RejectionReason)
	}
	if ctx.RejectedCommandType() != "type.googleapis.com/test.ReserveStock" {
		t.Errorf("expected rejected command type, got %q", ctx.RejectedCommandType())
	}
}
