package logic

import (
	"encoding/hex"
	"strings"

	"saga-cancellation/proto/angzarr"
	"saga-cancellation/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName     = "cancellation"
	SourceDomain = "order"
)

// CancellationSagaLogic processes OrderCancelled events to release inventory
// and potentially reverse loyalty points.
type CancellationSagaLogic interface {
	ProcessEvents(eventBook *angzarr.EventBook) []*angzarr.CommandBook
}

type DefaultCancellationSagaLogic struct{}

func NewCancellationSagaLogic() CancellationSagaLogic {
	return &DefaultCancellationSagaLogic{}
}

func (l *DefaultCancellationSagaLogic) ProcessEvents(eventBook *angzarr.EventBook) []*angzarr.CommandBook {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

	var commands []*angzarr.CommandBook

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		// Only process OrderCancelled events
		if !strings.HasSuffix(page.Event.TypeUrl, "OrderCancelled") {
			continue
		}

		// Decode the event
		var event examples.OrderCancelled
		if err := page.Event.UnmarshalTo(&event); err != nil {
			continue
		}

		// Get order ID from root
		orderID := ""
		if eventBook.Cover != nil && eventBook.Cover.Root != nil {
			orderID = hex.EncodeToString(eventBook.Cover.Root.Value)
		}
		if orderID == "" {
			continue
		}

		// Release inventory reservation
		releaseCmd := &examples.ReleaseReservation{
			OrderId: orderID,
		}

		releaseCmdAny, err := anypb.New(releaseCmd)
		if err != nil {
			continue
		}

		// Target inventory domain with order ID as root
		releaseCmdBook := &angzarr.CommandBook{
			Cover: &angzarr.Cover{
				Domain: "inventory",
				Root:   eventBook.Cover.Root,
			},
			Pages: []*angzarr.CommandPage{
				{
					Sequence:    0,
					SyncMode: angzarr.SyncMode_SYNC_MODE_NONE,
					Command:     releaseCmdAny,
				},
			},
			CorrelationId: eventBook.CorrelationId,
		}

		commands = append(commands, releaseCmdBook)

		// If loyalty points were used, return them
		if event.LoyaltyPointsUsed > 0 {
			addPointsCmd := &examples.AddLoyaltyPoints{
				Points: event.LoyaltyPointsUsed,
				Reason: "Order cancellation refund",
			}

			addPointsCmdAny, err := anypb.New(addPointsCmd)
			if err != nil {
				continue
			}

			// Note: In a real system, we'd need the customer ID
			// This would come from saga context or be looked up
			addPointsCmdBook := &angzarr.CommandBook{
				Cover: &angzarr.Cover{
					Domain: "customer",
					// Root would be customer ID, needs to be passed in context
				},
				Pages: []*angzarr.CommandPage{
					{
						Sequence:    0,
						SyncMode: angzarr.SyncMode_SYNC_MODE_NONE,
						Command:     addPointsCmdAny,
					},
				},
				CorrelationId: eventBook.CorrelationId,
			}

			commands = append(commands, addPointsCmdBook)
		}
	}

	return commands
}
