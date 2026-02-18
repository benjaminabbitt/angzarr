package handlers

import (
	"fmt"
	"log"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
)

// HandleTableJoinRejected handles compensation when a table join fails.
//
// Called when a saga/PM command targeting the table aggregate's JoinTable
// command is rejected. This typically means funds were reserved but the
// table join failed, so we need to release the reserved funds.
//
// Currently delegates to framework (emits SagaCompensationFailed).
// In a real implementation, this would emit FundsReleased events.
func HandleTableJoinRejected(notification *pb.Notification, state PlayerState) *pb.BusinessResponse {
	ctx := angzarr.NewCompensationContext(notification)

	log.Printf("Player compensation for table join rejection: issuer=%s reason=%s seq=%d",
		ctx.IssuerName, ctx.RejectionReason, ctx.SourceEventSequence)

	// Example: Auto-release funds for the failed table join
	// In a real implementation, we would:
	// 1. Extract which table's funds to release from the rejected command
	// 2. Emit a FundsReleased event for that amount
	//
	// var events []*anypb.Any
	// for tableKey, amount := range state.TableReservations {
	//     tableRoot, _ := hex.DecodeString(tableKey)
	//     event := &examples.FundsReleased{
	//         TableRoot: tableRoot,
	//         Amount:    &examples.Currency{Amount: amount, CurrencyCode: "CHIPS"},
	//         ReleasedAt: timestamppb.Now(),
	//     }
	//     eventAny, _ := anypb.New(event)
	//     events = append(events, eventAny)
	// }
	// if len(events) > 0 {
	//     return angzarr.EmitCompensationEvents(&pb.EventBook{
	//         Pages: makePages(events),
	//     })
	// }

	// Default: delegate to framework
	return angzarr.DelegateToFramework(
		fmt.Sprintf("Player aggregate: delegating table join rejection to framework (%s)", ctx.RejectionReason),
	)
}
