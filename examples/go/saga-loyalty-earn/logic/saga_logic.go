package logic

import (
	"fmt"
	"strings"

	"saga-loyalty-earn/proto/angzarr"
	"saga-loyalty-earn/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

const (
	SagaName     = "loyalty-earn"
	SourceDomain = "order"
	TargetDomain = "customer"
)

// LoyaltyEarnSagaLogic processes OrderCompleted events to award loyalty points.
// Two-phase protocol: Prepare declares needed destinations, Execute produces commands.
type LoyaltyEarnSagaLogic interface {
	// Prepare examines source events and returns destination covers needed.
	// Returns customer cover for optimistic concurrency.
	Prepare(source *angzarr.EventBook) []*angzarr.Cover

	// Execute produces commands given source events and destination state.
	Execute(source *angzarr.EventBook, destinations []*angzarr.EventBook) []*angzarr.CommandBook
}

type DefaultLoyaltyEarnSagaLogic struct{}

func NewLoyaltyEarnSagaLogic() LoyaltyEarnSagaLogic {
	return &DefaultLoyaltyEarnSagaLogic{}
}

// Prepare returns customer cover for optimistic concurrency.
func (l *DefaultLoyaltyEarnSagaLogic) Prepare(source *angzarr.EventBook) []*angzarr.Cover {
	if source == nil || len(source.Pages) == 0 {
		return nil
	}

	if source.Cover == nil || source.Cover.Root == nil {
		return nil
	}

	// Check if any page has OrderCompleted with points to award
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

		if event.LoyaltyPointsEarned > 0 {
			// Request customer aggregate state (same root as order)
			return []*angzarr.Cover{
				{
					Domain: TargetDomain,
					Root:   source.Cover.Root,
				},
			}
		}
	}

	return nil
}

// Execute processes source events and produces AddLoyaltyPoints commands.
func (l *DefaultLoyaltyEarnSagaLogic) Execute(source *angzarr.EventBook, destinations []*angzarr.EventBook) []*angzarr.CommandBook {
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

		// Decode the event
		var event examples.OrderCompleted
		if err := page.Event.UnmarshalTo(&event); err != nil {
			continue
		}

		// Skip if no points to award
		if event.LoyaltyPointsEarned <= 0 {
			continue
		}

		if source.Cover == nil || source.Cover.Root == nil {
			continue
		}

		// Create AddLoyaltyPoints command
		cmd := &examples.AddLoyaltyPoints{
			Points: event.LoyaltyPointsEarned,
			Reason: fmt.Sprintf("order:%s", source.CorrelationId),
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
