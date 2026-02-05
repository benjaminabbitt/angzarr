package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"

	"angzarr/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName     = "sag-fulfillment-inventory"
	SourceDomain = "fulfillment"
	TargetDomain = "inventory"
)

// productRoot generates a deterministic UUID for an inventory product aggregate
// and returns it as a proto UUID.
func productRoot(productID string) *angzarrpb.UUID {
	return angzarr.ToProtoUUID(angzarr.InventoryProductRoot(productID))
}

// HandleShipped commits inventory reservations for each line item when an order ships.
func HandleShipped(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	var evt examples.Shipped
	if err := event.UnmarshalTo(&evt); err != nil {
		return nil
	}

	var commands []*angzarrpb.CommandBook
	for _, item := range evt.Items {
		cmd := &examples.CommitReservation{
			OrderId: evt.OrderId,
		}
		cmdAny, err := anypb.New(cmd)
		if err != nil {
			continue
		}

		commands = append(commands, &angzarrpb.CommandBook{
			Cover: &angzarrpb.Cover{
				Domain:        TargetDomain,
				Root:          productRoot(item.ProductId),
				CorrelationId: correlationID,
			},
			Pages: []*angzarrpb.CommandPage{
				{Command: cmdAny},
			},
		})
	}

	return commands
}
