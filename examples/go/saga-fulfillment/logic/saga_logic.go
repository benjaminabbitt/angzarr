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
// Two-phase protocol: Prepare declares needed destinations, Execute produces commands.
type FulfillmentSagaLogic interface {
	// Prepare examines source events and returns destination covers needed.
	// Returns the fulfillment aggregate cover for optimistic concurrency.
	Prepare(source *angzarr.EventBook) []*angzarr.Cover

	// Execute produces commands given source events and destination state.
	Execute(source *angzarr.EventBook, destinations []*angzarr.EventBook) []*angzarr.CommandBook
}

type DefaultFulfillmentSagaLogic struct{}

func NewFulfillmentSagaLogic() FulfillmentSagaLogic {
	return &DefaultFulfillmentSagaLogic{}
}

// Prepare returns the fulfillment aggregate cover for optimistic concurrency.
func (l *DefaultFulfillmentSagaLogic) Prepare(source *angzarr.EventBook) []*angzarr.Cover {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	// Check if any page has OrderCompleted event
	hasOrderCompleted := false
	for _, page := range source.Pages {
		if page.Event != nil && strings.HasSuffix(page.Event.TypeUrl, "OrderCompleted") {
			hasOrderCompleted = true
			break
		}
	}

	if !hasOrderCompleted {
		return nil
	}

	// Request the fulfillment aggregate state (same root as order)
	if source.Cover != nil && source.Cover.Root != nil {
		return []*angzarr.Cover{
			{
				Domain: TargetDomain,
				Root:   source.Cover.Root,
			},
		}
	}

	return nil
}

// Execute processes source events and produces CreateShipment commands.
func (l *DefaultFulfillmentSagaLogic) Execute(source *angzarr.EventBook, destinations []*angzarr.EventBook) []*angzarr.CommandBook {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	// Calculate target sequence from destination state
	var targetSequence uint32 = 0
	if len(destinations) > 0 && destinations[0] != nil {
		targetSequence = uint32(len(destinations[0].Pages))
	}

	var commands []*angzarr.CommandBook

	for _, page := range source.Pages {
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
		if source.Cover != nil && source.Cover.Root != nil {
			orderID = hex.EncodeToString(source.Cover.Root.Value)
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
				Root:   source.Cover.Root,
			},
			Pages: []*angzarr.CommandPage{
				{
					Sequence: targetSequence,
					Command:  cmdAny,
				},
			},
			CorrelationId: source.CorrelationId,
		}

		commands = append(commands, cmdBook)
	}

	return commands
}
