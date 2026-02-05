package logic

import (
	"encoding/hex"

	"angzarr"
	angzarrpb "angzarr/proto/angzarr"

	"angzarr/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName     = "sag-order-inventory"
	SourceDomain = "order"
	TargetDomain = "inventory"
)

// productRoot generates a deterministic UUID for an inventory product aggregate
// and returns it as a proto UUID.
func productRoot(productID string) *angzarrpb.UUID {
	return angzarr.ToProtoUUID(angzarr.InventoryProductRoot(productID))
}

// HandleOrderCreated reserves stock for each line item when an order is created.
func HandleOrderCreated(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	var evt examples.OrderCreated
	if err := event.UnmarshalTo(&evt); err != nil {
		return nil
	}
	if root == nil {
		return nil
	}

	orderID := hex.EncodeToString(root.Value)

	var commands []*angzarrpb.CommandBook
	for _, item := range evt.Items {
		cmd := &examples.ReserveStock{
			Quantity: item.Quantity,
			OrderId:  orderID,
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
