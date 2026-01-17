package logic

import (
	"strings"

	"saga-loyalty-earn/proto/angzarr"
	"saga-loyalty-earn/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName     = "loyalty-earn"
	SourceDomain = "fulfillment"
	TargetDomain = "customer"
)

// LoyaltyEarnSagaLogic processes Delivered events to award loyalty points.
type LoyaltyEarnSagaLogic interface {
	ProcessEvents(eventBook *angzarr.EventBook, customerID string, pointsEarned int32) []*angzarr.CommandBook
}

type DefaultLoyaltyEarnSagaLogic struct{}

func NewLoyaltyEarnSagaLogic() LoyaltyEarnSagaLogic {
	return &DefaultLoyaltyEarnSagaLogic{}
}

func (l *DefaultLoyaltyEarnSagaLogic) ProcessEvents(eventBook *angzarr.EventBook, customerID string, pointsEarned int32) []*angzarr.CommandBook {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

	var commands []*angzarr.CommandBook

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		// Process Delivered events from fulfillment domain
		if !strings.HasSuffix(page.Event.TypeUrl, "Delivered") {
			continue
		}

		// Verify it decodes
		var event examples.Delivered
		if err := page.Event.UnmarshalTo(&event); err != nil {
			continue
		}

		// Points earned passed in from order data (1 point per $1)
		if pointsEarned <= 0 || customerID == "" {
			continue
		}

		// Create AddLoyaltyPoints command
		cmd := &examples.AddLoyaltyPoints{
			Points: pointsEarned,
			Reason: "Order delivered",
		}

		cmdAny, err := anypb.New(cmd)
		if err != nil {
			continue
		}

		// Target the customer aggregate
		customerRoot := &angzarr.Uuid{
			Value: []byte(customerID),
		}

		cmdBook := &angzarr.CommandBook{
			Cover: &angzarr.Cover{
				Domain: TargetDomain,
				Root:   customerRoot,
			},
			Pages: []*angzarr.CommandPage{
				{
					Sequence:    0,
					Synchronous: false,
					Command:     cmdAny,
				},
			},
			CorrelationId: eventBook.CorrelationId,
		}

		commands = append(commands, cmdBook)
	}

	return commands
}
