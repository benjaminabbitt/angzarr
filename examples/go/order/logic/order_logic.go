package logic

import (
	"order/proto/angzarr"
	"order/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

type OrderLogic interface {
	RebuildState(eventBook *angzarr.EventBook) *OrderState
	HandleCreateOrder(state *OrderState, customerID string, items []*examples.LineItem) (*examples.OrderCreated, error)
	HandleApplyLoyaltyDiscount(state *OrderState, points, discountCents int32) (*examples.LoyaltyDiscountApplied, error)
	HandleSubmitPayment(state *OrderState, paymentMethod string, amountCents int32) (*examples.PaymentSubmitted, error)
	HandleConfirmPayment(state *OrderState, paymentReference string) (*examples.OrderCompleted, error)
	HandleCancelOrder(state *OrderState, reason string) (*examples.OrderCancelled, error)
}

type DefaultOrderLogic struct{}

func NewOrderLogic() OrderLogic {
	return &DefaultOrderLogic{}
}

func (l *DefaultOrderLogic) RebuildState(eventBook *angzarr.EventBook) *OrderState {
	state := EmptyState()

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		var snapState examples.OrderState
		if err := eventBook.Snapshot.State.UnmarshalTo(&snapState); err == nil {
			state.CustomerID = snapState.CustomerId
			state.SubtotalCents = snapState.SubtotalCents
			state.DiscountCents = snapState.DiscountCents
			state.LoyaltyPointsUsed = snapState.LoyaltyPointsUsed
			state.PaymentMethod = snapState.PaymentMethod
			state.PaymentReference = snapState.PaymentReference
			state.Status = snapState.Status
			for _, item := range snapState.Items {
				state.Items = append(state.Items, LineItem{
					ProductID:      item.ProductId,
					Name:           item.Name,
					Quantity:       item.Quantity,
					UnitPriceCents: item.UnitPriceCents,
				})
			}
		}
	}

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.OrderCreated{}):
			var event examples.OrderCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.CustomerID = event.CustomerId
				state.SubtotalCents = event.SubtotalCents
				state.Status = "pending"
				state.Items = make([]LineItem, 0, len(event.Items))
				for _, item := range event.Items {
					state.Items = append(state.Items, LineItem{
						ProductID:      item.ProductId,
						Name:           item.Name,
						Quantity:       item.Quantity,
						UnitPriceCents: item.UnitPriceCents,
					})
				}
			}

		case page.Event.MessageIs(&examples.LoyaltyDiscountApplied{}):
			var event examples.LoyaltyDiscountApplied
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.LoyaltyPointsUsed = event.PointsUsed
				state.DiscountCents = event.DiscountCents
			}

		case page.Event.MessageIs(&examples.PaymentSubmitted{}):
			var event examples.PaymentSubmitted
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.PaymentMethod = event.PaymentMethod
				state.Status = "payment_submitted"
			}

		case page.Event.MessageIs(&examples.OrderCompleted{}):
			var event examples.OrderCompleted
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.PaymentReference = event.PaymentReference
				state.Status = "completed"
			}

		case page.Event.MessageIs(&examples.OrderCancelled{}):
			if err := page.Event.UnmarshalTo(&examples.OrderCancelled{}); err == nil {
				state.Status = "cancelled"
			}
		}
	}

	return state
}

func (l *DefaultOrderLogic) HandleCreateOrder(state *OrderState, customerID string, items []*examples.LineItem) (*examples.OrderCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Order already exists")
	}
	if customerID == "" {
		return nil, NewInvalidArgument("Customer ID is required")
	}
	if len(items) == 0 {
		return nil, NewInvalidArgument("Order must have at least one item")
	}

	var subtotal int32
	for _, item := range items {
		if item.Quantity <= 0 {
			return nil, NewInvalidArgument("Item quantity must be positive")
		}
		subtotal += item.Quantity * item.UnitPriceCents
	}

	return &examples.OrderCreated{
		CustomerId:    customerID,
		Items:         items,
		SubtotalCents: subtotal,
		CreatedAt:     timestamppb.Now(),
	}, nil
}

func (l *DefaultOrderLogic) HandleApplyLoyaltyDiscount(state *OrderState, points, discountCents int32) (*examples.LoyaltyDiscountApplied, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Order does not exist")
	}
	if !state.IsPending() {
		return nil, NewFailedPrecondition("Order is not in pending state")
	}
	if state.LoyaltyPointsUsed > 0 {
		return nil, NewFailedPrecondition("Loyalty discount already applied")
	}
	if points <= 0 {
		return nil, NewInvalidArgument("Points must be positive")
	}
	if discountCents <= 0 {
		return nil, NewInvalidArgument("Discount must be positive")
	}
	if discountCents > state.SubtotalCents {
		return nil, NewInvalidArgument("Discount cannot exceed subtotal")
	}

	return &examples.LoyaltyDiscountApplied{
		PointsUsed:    points,
		DiscountCents: discountCents,
		AppliedAt:     timestamppb.Now(),
	}, nil
}

func (l *DefaultOrderLogic) HandleSubmitPayment(state *OrderState, paymentMethod string, amountCents int32) (*examples.PaymentSubmitted, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Order does not exist")
	}
	if !state.IsPending() {
		return nil, NewFailedPrecondition("Order is not in pending state")
	}
	if paymentMethod == "" {
		return nil, NewInvalidArgument("Payment method is required")
	}
	expectedTotal := state.TotalAfterDiscount()
	if amountCents != expectedTotal {
		return nil, NewInvalidArgument("Payment amount must match order total")
	}

	return &examples.PaymentSubmitted{
		PaymentMethod: paymentMethod,
		AmountCents:   amountCents,
		SubmittedAt:   timestamppb.Now(),
	}, nil
}

func (l *DefaultOrderLogic) HandleConfirmPayment(state *OrderState, paymentReference string) (*examples.OrderCompleted, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Order does not exist")
	}
	if !state.IsPaymentSubmitted() {
		return nil, NewFailedPrecondition("Payment not submitted")
	}
	if paymentReference == "" {
		return nil, NewInvalidArgument("Payment reference is required")
	}

	// 1 point per dollar spent
	loyaltyPointsEarned := state.TotalAfterDiscount() / 100

	return &examples.OrderCompleted{
		FinalTotalCents:     state.TotalAfterDiscount(),
		PaymentMethod:       state.PaymentMethod,
		PaymentReference:    paymentReference,
		LoyaltyPointsEarned: loyaltyPointsEarned,
		CompletedAt:         timestamppb.Now(),
	}, nil
}

func (l *DefaultOrderLogic) HandleCancelOrder(state *OrderState, reason string) (*examples.OrderCancelled, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Order does not exist")
	}
	if state.IsCompleted() {
		return nil, NewFailedPrecondition("Cannot cancel completed order")
	}
	if state.IsCancelled() {
		return nil, NewFailedPrecondition("Order already cancelled")
	}
	if reason == "" {
		return nil, NewInvalidArgument("Cancellation reason is required")
	}

	return &examples.OrderCancelled{
		Reason:           reason,
		CancelledAt:      timestamppb.Now(),
		LoyaltyPointsUsed: state.LoyaltyPointsUsed,
	}, nil
}

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

func NextSequence(priorEvents *angzarr.EventBook) uint32 {
	if priorEvents == nil || len(priorEvents.Pages) == 0 {
		return 0
	}
	return uint32(len(priorEvents.Pages))
}
