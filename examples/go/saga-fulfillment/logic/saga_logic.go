package logic

import (
	"encoding/hex"
	"strings"

	"saga-fulfillment/proto/angzarr"
	"saga-fulfillment/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName     = "fulfillment"
	SourceDomain = "order"
	TargetDomain = "fulfillment"
)

// FulfillmentSagaLogic processes OrderCompleted events to create shipments.
type FulfillmentSagaLogic interface {
	ProcessEvents(eventBook *angzarr.EventBook) []*angzarr.CommandBook
}

type DefaultFulfillmentSagaLogic struct{}

func NewFulfillmentSagaLogic() FulfillmentSagaLogic {
	return &DefaultFulfillmentSagaLogic{}
}

func (l *DefaultFulfillmentSagaLogic) ProcessEvents(eventBook *angzarr.EventBook) []*angzarr.CommandBook {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

	var commands []*angzarr.CommandBook

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		// Only process OrderCompleted events
		if !strings.HasSuffix(page.Event.TypeUrl, "OrderCompleted") {
			continue
		}

		// Verify it decodes
		var event examples.OrderCompleted
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

		// Create shipment command
		cmd := &examples.CreateShipment{
			OrderId: orderID,
		}

		cmdAny, err := anypb.New(cmd)
		if err != nil {
			continue
		}

		cmdBook := &angzarr.CommandBook{
			Cover: &angzarr.Cover{
				Domain: TargetDomain,
				Root:   eventBook.Cover.Root,
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
