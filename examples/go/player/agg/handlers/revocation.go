package handlers

import (
	"fmt"
	"log"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
)

// HandleRevocation handles saga/PM compensation for player-related failures.
//
// Called when a saga/PM command targeting another aggregate is rejected.
// For example, if ReserveFunds succeeded but the subsequent table join failed,
// this method handles compensation.
//
// Currently delegates to framework (emits SagaCompensationFailed).
// Override for custom compensation like auto-releasing funds.
func HandleRevocation(notification *pb.Notification, state PlayerState) *pb.BusinessResponse {
	ctx := angzarr.NewCompensationContext(notification)

	log.Printf("Player compensation: issuer=%s reason=%s seq=%d",
		ctx.IssuerName, ctx.RejectionReason, ctx.SourceEventSequence)

	// Example: Auto-release funds if a table join saga failed
	// if strings.Contains(strings.ToLower(ctx.IssuerName), "table") {
	//     // Find which table's funds to release from state
	//     // For each table with reserved funds, emit a FundsReleased event
	//     var events []*anypb.Any
	//     for tableKey, amount := range state.TableReservations {
	//         tableRoot, _ := hex.DecodeString(tableKey)
	//         event := &examples.FundsReleased{
	//             TableRoot: tableRoot,
	//             Amount:    &examples.Currency{Amount: amount, CurrencyCode: "CHIPS"},
	//             ReleasedAt: timestamppb.Now(),
	//         }
	//         eventAny, _ := anypb.New(event)
	//         events = append(events, eventAny)
	//     }
	//     if len(events) > 0 {
	//         return angzarr.EmitCompensationEvents(&pb.EventBook{
	//             Pages: makePages(events),
	//         })
	//     }
	// }

	// Default: delegate to framework
	return angzarr.DelegateToFramework(
		fmt.Sprintf("Player aggregate: no custom compensation for %s", ctx.IssuerName),
	)
}
