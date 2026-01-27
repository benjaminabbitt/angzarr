package logic

import (
	"bytes"
	"encoding/hex"
	"strings"

	"saga-cancellation/proto/angzarr"
	"saga-cancellation/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName        = "cancellation"
	SourceDomain    = "order"
	InventoryDomain = "inventory"
	CustomerDomain  = "customer"
)

// CancellationSagaLogic processes OrderCancelled events to release inventory
// and potentially reverse loyalty points.
// Two-phase protocol: Prepare declares needed destinations, Execute produces commands.
type CancellationSagaLogic interface {
	// Prepare examines source events and returns destination covers needed.
	// Returns inventory and optionally customer covers for optimistic concurrency.
	Prepare(source *angzarr.EventBook) []*angzarr.Cover

	// Execute produces commands given source events and destination state.
	Execute(source *angzarr.EventBook, destinations []*angzarr.EventBook) []*angzarr.CommandBook
}

type DefaultCancellationSagaLogic struct{}

func NewCancellationSagaLogic() CancellationSagaLogic {
	return &DefaultCancellationSagaLogic{}
}

// Prepare returns destination covers for optimistic concurrency.
// Always requests inventory; requests customer if loyalty points were used.
func (l *DefaultCancellationSagaLogic) Prepare(source *angzarr.EventBook) []*angzarr.Cover {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	if source.Cover == nil || source.Cover.Root == nil {
		return nil
	}

	var covers []*angzarr.Cover
	needsCustomer := false

	for _, page := range source.Pages {
		if page.Event == nil {
			continue
		}

		if !strings.HasSuffix(page.Event.TypeUrl, "OrderCancelled") {
			continue
		}

		// Always need inventory for ReleaseReservation
		covers = append(covers, &angzarr.Cover{
			Domain: InventoryDomain,
			Root:   source.Cover.Root,
		})

		// Check if we need customer (loyalty points refund)
		var event examples.OrderCancelled
		if err := page.Event.UnmarshalTo(&event); err == nil {
			if event.LoyaltyPointsUsed > 0 {
				needsCustomer = true
			}
		}
	}

	if needsCustomer {
		covers = append(covers, &angzarr.Cover{
			Domain: CustomerDomain,
			Root:   source.Cover.Root,
		})
	}

	return covers
}

// getSequenceForDomain finds the sequence for a specific domain from destinations.
func getSequenceForDomain(destinations []*angzarr.EventBook, domain string, root []byte) uint32 {
	for _, dest := range destinations {
		if dest == nil || dest.Cover == nil {
			continue
		}
		if dest.Cover.Domain == domain {
			if dest.Cover.Root != nil && bytes.Equal(dest.Cover.Root.Value, root) {
				return uint32(len(dest.Pages))
			}
		}
	}
	return 0
}

// Execute processes source events and produces compensation commands.
func (l *DefaultCancellationSagaLogic) Execute(source *angzarr.EventBook, destinations []*angzarr.EventBook) []*angzarr.CommandBook {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	var commands []*angzarr.CommandBook

	for _, page := range source.Pages {
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
		var rootBytes []byte
		if source.Cover != nil && source.Cover.Root != nil {
			orderID = hex.EncodeToString(source.Cover.Root.Value)
			rootBytes = source.Cover.Root.Value
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

		inventorySeq := getSequenceForDomain(destinations, InventoryDomain, rootBytes)

		releaseCmdBook := &angzarr.CommandBook{
			Cover: &angzarr.Cover{
				Domain: InventoryDomain,
				Root:   source.Cover.Root,
			},
			Pages: []*angzarr.CommandPage{
				{
					Sequence: inventorySeq,
					Command:  releaseCmdAny,
				},
			},
			CorrelationId: source.CorrelationId,
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

			customerSeq := getSequenceForDomain(destinations, CustomerDomain, rootBytes)

			addPointsCmdBook := &angzarr.CommandBook{
				Cover: &angzarr.Cover{
					Domain: CustomerDomain,
					Root:   source.Cover.Root,
				},
				Pages: []*angzarr.CommandPage{
					{
						Sequence: customerSeq,
						Command:  addPointsCmdAny,
					},
				},
				CorrelationId: source.CorrelationId,
			}

			commands = append(commands, addPointsCmdBook)
		}
	}

	return commands
}
