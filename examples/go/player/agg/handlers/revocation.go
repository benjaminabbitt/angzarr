package handlers

import (
	"encoding/hex"
	"log"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	examples "github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// docs:start:rejected_handler

// HandleTableJoinRejected handles compensation when a table join fails.
//
// Called when a saga/PM command targeting the table aggregate's JoinTable
// command is rejected. This releases the funds that were reserved for the
// failed table join.
func HandleTableJoinRejected(notification *pb.Notification, state PlayerState) *pb.BusinessResponse {
	ctx := angzarr.NewCompensationContext(notification)

	log.Printf("Player compensation for JoinTable rejection: reason=%s",
		ctx.RejectionReason)

	// Extract table_root from the rejected command
	var tableRoot []byte
	if ctx.RejectedCommand != nil && ctx.RejectedCommand.Cover != nil && ctx.RejectedCommand.Cover.Root != nil {
		tableRoot = ctx.RejectedCommand.Cover.Root.Value
	}

	// Release the funds that were reserved for this table
	tableKey := hex.EncodeToString(tableRoot)
	reservedAmount := state.TableReservations[tableKey]
	newReserved := state.ReservedFunds - reservedAmount
	newAvailable := state.Bankroll - newReserved

	event := &examples.FundsReleased{
		Amount:              &examples.Currency{Amount: reservedAmount, CurrencyCode: "CHIPS"},
		TableRoot:           tableRoot,
		NewAvailableBalance: &examples.Currency{Amount: newAvailable, CurrencyCode: "CHIPS"},
		NewReservedBalance:  &examples.Currency{Amount: newReserved, CurrencyCode: "CHIPS"},
		ReleasedAt:          timestamppb.Now(),
	}

	eventAny, _ := anypb.New(event)
	eventBook := &pb.EventBook{
		Cover: notification.Cover,
		Pages: []*pb.EventPage{
			{
				Payload: &pb.EventPage_Event{Event: eventAny},
			},
		},
	}
	return angzarr.EmitCompensationEvents(eventBook)
}

// docs:end:rejected_handler
