package logic

import (
	"encoding/hex"

	"angzarr"
	angzarrpb "angzarr/proto/angzarr"

	"angzarr/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName     = "inventory-reservation"
	SourceDomain = "cart"
	TargetDomain = "inventory"
)

// productRoot generates a deterministic UUID for an inventory product aggregate
// and returns it as a proto UUID.
func productRoot(productID string) *angzarrpb.UUID {
	return angzarr.ToProtoUUID(angzarr.InventoryProductRoot(productID))
}

// HandleItemAdded reserves stock when an item is added to a cart.
func HandleItemAdded(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	var evt examples.ItemAdded
	if err := event.UnmarshalTo(&evt); err != nil {
		return nil
	}
	if root == nil {
		return nil
	}

	cartID := hex.EncodeToString(root.Value)

	cmd := &examples.ReserveStock{
		Quantity: evt.Quantity,
		OrderId:  cartID,
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return nil
	}

	return []*angzarrpb.CommandBook{{
		Cover: &angzarrpb.Cover{
			Domain:        TargetDomain,
			Root:          productRoot(evt.ProductId),
			CorrelationId: correlationID,
		},
		Pages: []*angzarrpb.CommandPage{
			{Command: cmdAny},
		},
	}}
}

// HandleItemRemoved releases reservation when an item is removed from a cart.
func HandleItemRemoved(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	var evt examples.ItemRemoved
	if err := event.UnmarshalTo(&evt); err != nil {
		return nil
	}
	if root == nil {
		return nil
	}

	cartID := hex.EncodeToString(root.Value)

	cmd := &examples.ReleaseReservation{OrderId: cartID}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return nil
	}

	return []*angzarrpb.CommandBook{{
		Cover: &angzarrpb.Cover{
			Domain:        TargetDomain,
			Root:          productRoot(evt.ProductId),
			CorrelationId: correlationID,
		},
		Pages: []*angzarrpb.CommandPage{
			{Command: cmdAny},
		},
	}}
}

// HandleQuantityUpdated adjusts reservation when cart item quantity changes.
// Releases the old reservation then reserves the new quantity.
func HandleQuantityUpdated(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	var evt examples.QuantityUpdated
	if err := event.UnmarshalTo(&evt); err != nil {
		return nil
	}
	if root == nil {
		return nil
	}

	cartID := hex.EncodeToString(root.Value)
	targetRoot := productRoot(evt.ProductId)

	releaseCmd := &examples.ReleaseReservation{OrderId: cartID}
	releaseCmdAny, err := anypb.New(releaseCmd)
	if err != nil {
		return nil
	}

	reserveCmd := &examples.ReserveStock{
		Quantity: evt.NewQuantity,
		OrderId:  cartID,
	}
	reserveCmdAny, err := anypb.New(reserveCmd)
	if err != nil {
		return nil
	}

	return []*angzarrpb.CommandBook{
		{
			Cover: &angzarrpb.Cover{
				Domain:        TargetDomain,
				Root:          targetRoot,
				CorrelationId: correlationID,
			},
			Pages: []*angzarrpb.CommandPage{
				{Command: releaseCmdAny},
			},
		},
		{
			Cover: &angzarrpb.Cover{
				Domain:        TargetDomain,
				Root:          targetRoot,
				CorrelationId: correlationID,
			},
			Pages: []*angzarrpb.CommandPage{
				{Command: reserveCmdAny},
			},
		},
	}
}

// HandleCartCleared releases reservations for all items when a cart is cleared.
func HandleCartCleared(event *anypb.Any, root *angzarrpb.UUID, correlationID string) []*angzarrpb.CommandBook {
	var evt examples.CartCleared
	if err := event.UnmarshalTo(&evt); err != nil {
		return nil
	}
	if root == nil {
		return nil
	}

	cartID := hex.EncodeToString(root.Value)

	var commands []*angzarrpb.CommandBook
	for _, item := range evt.Items {
		cmd := &examples.ReleaseReservation{OrderId: cartID}
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
