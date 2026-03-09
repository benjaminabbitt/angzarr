package angzarr

import (
	"testing"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// TestOOState is a simple state type for testing OO aggregates.
type TestOOState struct {
	Value          string
	ReservedAmount int64
}

// TestOOAggregate is a test aggregate for verifying rejection handling.
type TestOOAggregate struct {
	CommandHandlerBase[TestOOState]
	rejectionHandlerCalled bool
	lastNotification       *pb.Notification
}

func NewTestOOAggregate(eventBook *pb.EventBook) *TestOOAggregate {
	agg := &TestOOAggregate{}
	agg.Init(eventBook, func() TestOOState { return TestOOState{} })
	agg.SetDomain("test")
	return agg
}

func (a *TestOOAggregate) handlePaymentRejected(notification *pb.Notification) *pb.BusinessResponse {
	a.rejectionHandlerCalled = true
	a.lastNotification = notification
	// Return compensation events
	return EmitCompensationEvents(&pb.EventBook{
		Cover: &pb.Cover{Domain: "test"},
		Pages: []*pb.EventPage{
			{
				Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				Payload: &pb.EventPage_Event{
					Event: &anypb.Any{TypeUrl: "type.googleapis.com/test.FundsReleased"},
				},
			},
		},
	})
}

func (a *TestOOAggregate) handleInventoryRejected(notification *pb.Notification) *pb.BusinessResponse {
	a.rejectionHandlerCalled = true
	a.lastNotification = notification
	// Delegate to framework
	return DelegateToFramework("No custom compensation for inventory")
}

func TestCommandHandlerBase_HandlesRejection(t *testing.T) {
	t.Run("registers rejection handler", func(t *testing.T) {
		agg := NewTestOOAggregate(nil)
		agg.HandlesRejection("payment", "ProcessPayment", agg.handlePaymentRejected)

		if len(agg.rejectionHandlers) != 1 {
			t.Errorf("expected 1 rejection handler, got %d", len(agg.rejectionHandlers))
		}

		if _, ok := agg.rejectionHandlers["payment/ProcessPayment"]; !ok {
			t.Error("expected handler for 'payment/ProcessPayment'")
		}
	})

	t.Run("registers multiple rejection handlers", func(t *testing.T) {
		agg := NewTestOOAggregate(nil)
		agg.HandlesRejection("payment", "ProcessPayment", agg.handlePaymentRejected)
		agg.HandlesRejection("inventory", "ReserveStock", agg.handleInventoryRejected)

		if len(agg.rejectionHandlers) != 2 {
			t.Errorf("expected 2 rejection handlers, got %d", len(agg.rejectionHandlers))
		}
	})
}

func TestCommandHandlerBase_DispatchRejection(t *testing.T) {
	t.Run("routes to matching handler and returns compensation events", func(t *testing.T) {
		agg := NewTestOOAggregate(nil)
		// Register with full type name (package.TypeName) as extracted from type URL
		agg.HandlesRejection("payment", "payment.ProcessPayment", agg.handlePaymentRejected)

		// Create a rejection notification
		rejection := &pb.RejectionNotification{
			RejectionReason: "insufficient_funds",
			RejectedCommand: &pb.CommandBook{
				Cover: &pb.Cover{Domain: "payment"},
				Pages: []*pb.CommandPage{
					{
						Payload: &pb.CommandPage_Command{
							Command: &anypb.Any{TypeUrl: "type.googleapis.com/payment.ProcessPayment"},
						},
					},
				},
			},
		}

		rejectionAny, _ := anypb.New(rejection)
		notification := &pb.Notification{Payload: rejectionAny}
		notificationAny, _ := anypb.New(notification)

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Cover: &pb.Cover{Domain: "test"},
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: notificationAny}},
				},
			},
		}

		resp, err := agg.Handle(cmd)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if !agg.rejectionHandlerCalled {
			t.Error("rejection handler was not called")
		}

		if agg.lastNotification == nil {
			t.Error("notification was not passed to handler")
		}

		// Should return events (compensation)
		events := resp.GetEvents()
		if events == nil {
			t.Fatal("expected events in response")
		}

		if len(events.Pages) != 1 {
			t.Errorf("expected 1 event page, got %d", len(events.Pages))
		}
	})

	t.Run("delegates to framework when no handler matches", func(t *testing.T) {
		agg := NewTestOOAggregate(nil)
		// No handlers registered

		// Create a rejection notification for unknown domain/command
		rejection := &pb.RejectionNotification{
			RejectionReason: "failed",
			RejectedCommand: &pb.CommandBook{
				Cover: &pb.Cover{Domain: "unknown"},
				Pages: []*pb.CommandPage{
					{
						Payload: &pb.CommandPage_Command{
							Command: &anypb.Any{TypeUrl: "type.googleapis.com/unknown.UnknownCommand"},
						},
					},
				},
			},
		}

		rejectionAny, _ := anypb.New(rejection)
		notification := &pb.Notification{Payload: rejectionAny}
		notificationAny, _ := anypb.New(notification)

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Cover: &pb.Cover{Domain: "test"},
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: notificationAny}},
				},
			},
		}

		resp, err := agg.Handle(cmd)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Should return revocation response
		revocation := resp.GetRevocation()
		if revocation == nil {
			t.Fatal("expected revocation response")
		}

		if !revocation.EmitSystemRevocation {
			t.Error("expected EmitSystemRevocation to be true")
		}

		if revocation.Reason == "" {
			t.Error("expected reason to be set")
		}
	})

	t.Run("handler can return framework delegation", func(t *testing.T) {
		agg := NewTestOOAggregate(nil)
		// Register with full type name (package.TypeName) as extracted from type URL
		agg.HandlesRejection("inventory", "inventory.ReserveStock", agg.handleInventoryRejected)

		// Create a rejection notification
		rejection := &pb.RejectionNotification{
			RejectionReason: "out_of_stock",
			RejectedCommand: &pb.CommandBook{
				Cover: &pb.Cover{Domain: "inventory"},
				Pages: []*pb.CommandPage{
					{
						Payload: &pb.CommandPage_Command{
							Command: &anypb.Any{TypeUrl: "type.googleapis.com/inventory.ReserveStock"},
						},
					},
				},
			},
		}

		rejectionAny, _ := anypb.New(rejection)
		notification := &pb.Notification{Payload: rejectionAny}
		notificationAny, _ := anypb.New(notification)

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Cover: &pb.Cover{Domain: "test"},
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: notificationAny}},
				},
			},
		}

		resp, err := agg.Handle(cmd)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if !agg.rejectionHandlerCalled {
			t.Error("rejection handler was not called")
		}

		// Should return revocation response from handler
		revocation := resp.GetRevocation()
		if revocation == nil {
			t.Fatal("expected revocation response from handler")
		}

		if !revocation.EmitSystemRevocation {
			t.Error("expected EmitSystemRevocation to be true")
		}
	})

	t.Run("rebuilds state before calling handler", func(t *testing.T) {
		// Create aggregate with prior events
		priorEvents := &pb.EventBook{
			Cover: &pb.Cover{Domain: "test"},
			Pages: []*pb.EventPage{
				{Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}}},
				{Header: &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 1}}},
			},
		}
		agg := NewTestOOAggregate(priorEvents)

		var capturedState *TestOOState
		// Register with full type name (package.TypeName) as extracted from type URL
		agg.HandlesRejection("payment", "payment.ProcessPayment", func(notification *pb.Notification) *pb.BusinessResponse {
			capturedState = agg.State()
			return DelegateToFramework("test")
		})

		rejection := &pb.RejectionNotification{
			RejectedCommand: &pb.CommandBook{
				Cover: &pb.Cover{Domain: "payment"},
				Pages: []*pb.CommandPage{
					{
						Payload: &pb.CommandPage_Command{
							Command: &anypb.Any{TypeUrl: "type.googleapis.com/payment.ProcessPayment"},
						},
					},
				},
			},
		}

		rejectionAny, _ := anypb.New(rejection)
		notification := &pb.Notification{Payload: rejectionAny}
		notificationAny, _ := anypb.New(notification)

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: notificationAny}},
				},
			},
		}

		_, err := agg.Handle(cmd)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if capturedState == nil {
			t.Error("state was not available in handler")
		}
	})
}

func TestCommandHandlerBase_DispatchRejection_EdgeCases(t *testing.T) {
	t.Run("handles empty rejected_command", func(t *testing.T) {
		agg := NewTestOOAggregate(nil)

		// Create notification with no rejected_command
		rejection := &pb.RejectionNotification{
			RejectionReason: "failed",
			RejectedCommand: nil, // No rejected command
		}

		rejectionAny, _ := anypb.New(rejection)
		notification := &pb.Notification{Payload: rejectionAny}
		notificationAny, _ := anypb.New(notification)

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: notificationAny}},
				},
			},
		}

		resp, err := agg.Handle(cmd)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Should delegate to framework with "/" key (empty domain/command)
		revocation := resp.GetRevocation()
		if revocation == nil {
			t.Fatal("expected revocation response")
		}
	})

	t.Run("handles notification with no payload", func(t *testing.T) {
		agg := NewTestOOAggregate(nil)

		// Create notification with no payload
		notification := &pb.Notification{Payload: nil}
		notificationBytes, _ := proto.Marshal(notification)
		notificationAny := &anypb.Any{
			TypeUrl: TypeURLPrefix + "angzarr.Notification",
			Value:   notificationBytes,
		}

		cmd := &pb.ContextualCommand{
			Command: &pb.CommandBook{
				Pages: []*pb.CommandPage{
					{Payload: &pb.CommandPage_Command{Command: notificationAny}},
				},
			},
		}

		resp, err := agg.Handle(cmd)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Should delegate to framework
		revocation := resp.GetRevocation()
		if revocation == nil {
			t.Fatal("expected revocation response")
		}
	})
}
