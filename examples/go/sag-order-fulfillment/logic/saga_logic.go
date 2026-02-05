package logic

import (
	"encoding/hex"
	"strings"

	angzarrpb "angzarr/proto/angzarr"

	"angzarr/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName     = "sag-order-fulfillment"
	SourceDomain = "order"
	TargetDomain = "fulfillment"
)

// HandleOrderCompleted processes a single OrderCompleted event.
// Used as a SagaEventHandler for the EventRouter (simple mode without destination state).
func HandleOrderCompleted(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	var evt examples.OrderCompleted
	if err := event.UnmarshalTo(&evt); err != nil {
		return nil
	}
	if root == nil {
		return nil
	}

	orderID := hex.EncodeToString(root.Value)
	if orderID == "" {
		return nil
	}

	cmd := &examples.CreateShipment{OrderId: orderID, Items: evt.Items}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return nil
	}

	return []*angzarrpb.CommandBook{{
		Cover: &angzarrpb.Cover{
			Domain:        TargetDomain,
			Root:          root,
			CorrelationId: correlationID,
		},
		Pages: []*angzarrpb.CommandPage{
			{Command: cmdAny},
		},
	}}
}

// Prepare examines source events and returns destination covers needed.
// Returns the fulfillment aggregate cover for optimistic concurrency.
func Prepare(source *angzarrpb.EventBook) []*angzarrpb.Cover {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

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

	if source.Cover != nil && source.Cover.Root != nil {
		return []*angzarrpb.Cover{
			{Domain: TargetDomain, Root: source.Cover.Root},
		}
	}

	return nil
}

// Execute processes source events with destination state and produces CreateShipment commands.
// Uses destination state to calculate the target sequence for optimistic concurrency.
func Execute(source *angzarrpb.EventBook, destinations []*angzarrpb.EventBook) []*angzarrpb.CommandBook {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	var targetSequence uint32
	if len(destinations) > 0 && destinations[0] != nil {
		targetSequence = uint32(len(destinations[0].Pages))
	}

	var commands []*angzarrpb.CommandBook

	for _, page := range source.Pages {
		if page.Event == nil {
			continue
		}
		if !strings.HasSuffix(page.Event.TypeUrl, "OrderCompleted") {
			continue
		}

		var event examples.OrderCompleted
		if err := page.Event.UnmarshalTo(&event); err != nil {
			continue
		}

		orderID := ""
		if source.Cover != nil && source.Cover.Root != nil {
			orderID = hex.EncodeToString(source.Cover.Root.Value)
		}
		if orderID == "" {
			continue
		}

		cmd := &examples.CreateShipment{OrderId: orderID, Items: event.Items}
		cmdAny, err := anypb.New(cmd)
		if err != nil {
			continue
		}

		commands = append(commands, &angzarrpb.CommandBook{
			Cover: &angzarrpb.Cover{
				Domain:        TargetDomain,
				Root:          source.Cover.Root,
				CorrelationId: source.Cover.CorrelationId,
			},
			Pages: []*angzarrpb.CommandPage{
				{Sequence: targetSequence, Command: cmdAny},
			},
		})
	}

	return commands
}
