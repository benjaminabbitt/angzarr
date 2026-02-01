package logic

import (
	"customer/proto/angzarr"
	"customer/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// CustomerLogic provides client logic operations for the customer domain.
type CustomerLogic interface {
	// RebuildState reconstructs customer state from an event book.
	RebuildState(eventBook *angzarr.EventBook) *CustomerState

	// HandleCreateCustomer handles the CreateCustomer command.
	HandleCreateCustomer(state *CustomerState, name, email string) (*examples.CustomerCreated, error)

	// HandleAddLoyaltyPoints handles the AddLoyaltyPoints command.
	HandleAddLoyaltyPoints(state *CustomerState, points int32, reason string) (*examples.LoyaltyPointsAdded, error)

	// HandleRedeemLoyaltyPoints handles the RedeemLoyaltyPoints command.
	HandleRedeemLoyaltyPoints(state *CustomerState, points int32, redemptionType string) (*examples.LoyaltyPointsRedeemed, error)
}

// DefaultCustomerLogic is the default implementation of CustomerLogic.
type DefaultCustomerLogic struct{}

// NewCustomerLogic creates a new CustomerLogic instance.
func NewCustomerLogic() CustomerLogic {
	return &DefaultCustomerLogic{}
}

// RebuildState reconstructs customer state from events.
func (l *DefaultCustomerLogic) RebuildState(eventBook *angzarr.EventBook) *CustomerState {
	state := EmptyState()

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	// Start from snapshot if present
	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		var snapState examples.CustomerState
		if err := eventBook.Snapshot.State.UnmarshalTo(&snapState); err == nil {
			state.Name = snapState.Name
			state.Email = snapState.Email
			state.LoyaltyPoints = snapState.LoyaltyPoints
			state.LifetimePoints = snapState.LifetimePoints
		}
	}

	// Apply events
	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.CustomerCreated{}):
			var event examples.CustomerCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.Name = event.Name
				state.Email = event.Email
			}

		case page.Event.MessageIs(&examples.LoyaltyPointsAdded{}):
			var event examples.LoyaltyPointsAdded
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.LoyaltyPoints = event.NewBalance
				state.LifetimePoints += event.Points
			}

		case page.Event.MessageIs(&examples.LoyaltyPointsRedeemed{}):
			var event examples.LoyaltyPointsRedeemed
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.LoyaltyPoints = event.NewBalance
			}
		}
	}

	return state
}

// HandleCreateCustomer validates and creates a CustomerCreated event.
func (l *DefaultCustomerLogic) HandleCreateCustomer(state *CustomerState, name, email string) (*examples.CustomerCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Customer already exists")
	}

	if name == "" {
		return nil, NewInvalidArgument("Customer name is required")
	}
	if email == "" {
		return nil, NewInvalidArgument("Customer email is required")
	}

	return &examples.CustomerCreated{
		Name:      name,
		Email:     email,
		CreatedAt: timestamppb.Now(),
	}, nil
}

// HandleAddLoyaltyPoints validates and creates a LoyaltyPointsAdded event.
func (l *DefaultCustomerLogic) HandleAddLoyaltyPoints(state *CustomerState, points int32, reason string) (*examples.LoyaltyPointsAdded, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Customer does not exist")
	}

	if points <= 0 {
		return nil, NewInvalidArgument("Points must be positive")
	}

	newBalance := state.LoyaltyPoints + points

	return &examples.LoyaltyPointsAdded{
		Points:     points,
		NewBalance: newBalance,
		Reason:     reason,
	}, nil
}

// HandleRedeemLoyaltyPoints validates and creates a LoyaltyPointsRedeemed event.
func (l *DefaultCustomerLogic) HandleRedeemLoyaltyPoints(state *CustomerState, points int32, redemptionType string) (*examples.LoyaltyPointsRedeemed, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Customer does not exist")
	}

	if points <= 0 {
		return nil, NewInvalidArgument("Points must be positive")
	}
	if points > state.LoyaltyPoints {
		return nil, NewFailedPreconditionf("Insufficient points: have %d, need %d", state.LoyaltyPoints, points)
	}

	newBalance := state.LoyaltyPoints - points

	return &examples.LoyaltyPointsRedeemed{
		Points:         points,
		NewBalance:     newBalance,
		RedemptionType: redemptionType,
	}, nil
}

// PackEvent wraps an event into an EventBook.
func PackEvent(cover *angzarr.Cover, event proto.Message, seq uint32) (*angzarr.EventBook, error) {
	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &angzarr.EventBook{
		Cover: cover,
		Pages: []*angzarr.EventPage{
			{
				Sequence:  &angzarr.EventPage_Num{Num: seq},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

// NextSequence returns the next event sequence number.
func NextSequence(priorEvents *angzarr.EventBook) uint32 {
	if priorEvents == nil || len(priorEvents.Pages) == 0 {
		return 0
	}
	return uint32(len(priorEvents.Pages))
}
