// Package angzarr provides compensation flow helpers for saga revocation handling.
//
// When a saga/PM command is rejected by a target aggregate, the framework sends
// a Notification with RejectionNotification payload to the triggering aggregate.
// These helpers make it easy to implement compensation logic.
//
// Example in aggregate:
//
//	router := NewCommandRouter("order", rebuildState).
//	    On("CreateOrder", handleCreateOrder).
//	    OnRejected("fulfillment", "CreateShipment", handleRevocation)
//
//	func handleRevocation(notification *pb.Notification, state OrderState) *pb.BusinessResponse {
//	    ctx := NewCompensationContext(notification)
//
//	    // Option 1: Emit compensation events
//	    event := &OrderCancelled{
//	        OrderId: state.OrderId,
//	        Reason:  fmt.Sprintf("Fulfillment failed: %s", ctx.RejectionReason),
//	    }
//	    return EmitCompensationEvents(PackEvents(event))
//
//	    // Option 2: Delegate to framework
//	    return DelegateToFramework("No custom compensation for " + ctx.IssuerName)
//	}
package angzarr

import (
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
)

// NotificationSuffix is used to detect rejection notifications.
const NotificationSuffix = "Notification"

// CompensationContext provides easy access to rejection details.
type CompensationContext struct {
	// IssuerName is the name of the saga/PM that issued the rejected command.
	IssuerName string

	// IssuerType is "saga" or "process_manager".
	IssuerType string

	// SourceEventSequence is the sequence of the event that triggered the saga/PM.
	SourceEventSequence uint32

	// RejectionReason is why the command was rejected.
	RejectionReason string

	// RejectedCommand is the command that was rejected (may be nil).
	RejectedCommand *pb.CommandBook

	// SourceAggregate is the cover of the aggregate that triggered the flow.
	SourceAggregate *pb.Cover
}

// NewCompensationContext extracts context from a Notification.
func NewCompensationContext(notification *pb.Notification) *CompensationContext {
	ctx := &CompensationContext{}

	if notification.Payload != nil {
		var rejection pb.RejectionNotification
		if err := proto.Unmarshal(notification.Payload.Value, &rejection); err == nil {
			ctx.IssuerName = rejection.IssuerName
			ctx.IssuerType = rejection.IssuerType
			ctx.SourceEventSequence = rejection.SourceEventSequence
			ctx.RejectionReason = rejection.RejectionReason
			ctx.RejectedCommand = rejection.RejectedCommand
			ctx.SourceAggregate = rejection.SourceAggregate
		}
	}

	return ctx
}

// RejectedCommandType returns the type URL of the rejected command, if available.
func (c *CompensationContext) RejectedCommandType() string {
	if c.RejectedCommand != nil && len(c.RejectedCommand.Pages) > 0 {
		page := c.RejectedCommand.Pages[0]
		if page.Command != nil {
			return page.Command.TypeUrl
		}
	}
	return ""
}

// --- Aggregate helpers ---

// DelegateToFramework creates a response that delegates compensation to the framework.
//
// Use when the aggregate doesn't have custom compensation logic for a saga.
// The framework will emit a SagaCompensationFailed event to the fallback domain.
func DelegateToFramework(reason string) *pb.BusinessResponse {
	return &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Revocation{
			Revocation: &pb.RevocationResponse{
				EmitSystemRevocation: true,
				Reason:               reason,
			},
		},
	}
}

// DelegateToFrameworkWithOptions creates a response with custom revocation flags.
func DelegateToFrameworkWithOptions(reason string, emitSystemEvent, sendToDLQ, escalate, abort bool) *pb.BusinessResponse {
	return &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Revocation{
			Revocation: &pb.RevocationResponse{
				EmitSystemRevocation:   emitSystemEvent,
				SendToDeadLetterQueue:  sendToDLQ,
				Escalate:               escalate,
				Abort:                  abort,
				Reason:                 reason,
			},
		},
	}
}

// EmitCompensationEvents creates a response containing compensation events.
//
// Use when the aggregate emits events to record compensation.
// The framework will persist these events and NOT emit a system event.
func EmitCompensationEvents(events *pb.EventBook) *pb.BusinessResponse {
	return &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Events{Events: events},
	}
}

// --- Process Manager helpers ---

// PMRevocationResponse holds PM compensation results.
type PMRevocationResponse struct {
	// ProcessEvents contains PM events to persist (may be nil).
	ProcessEvents *pb.EventBook

	// RevocationResponse contains framework action flags.
	Revocation *pb.RevocationResponse
}

// PMDelegateToFramework creates a PM response that delegates compensation.
//
// Use when the PM doesn't have custom compensation logic.
func PMDelegateToFramework(reason string) *PMRevocationResponse {
	return &PMRevocationResponse{
		ProcessEvents: nil,
		Revocation: &pb.RevocationResponse{
			EmitSystemRevocation: true,
			Reason:               reason,
		},
	}
}

// PMEmitCompensationEvents creates a PM response with compensation events.
//
// Use when the PM emits events to record the failure in its state.
func PMEmitCompensationEvents(events *pb.EventBook, alsoEmitSystemEvent bool, reason string) *PMRevocationResponse {
	return &PMRevocationResponse{
		ProcessEvents: events,
		Revocation: &pb.RevocationResponse{
			EmitSystemRevocation: alsoEmitSystemEvent,
			Reason:               reason,
		},
	}
}

// --- Helper functions ---

// IsNotification checks if a command is a rejection Notification.
func IsNotification(typeURL string) bool {
	return strings.HasSuffix(typeURL, NotificationSuffix)
}
