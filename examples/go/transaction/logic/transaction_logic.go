package logic

import (
	"transaction/proto/angzarr"
	"transaction/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// TransactionLogic provides business logic operations for the transaction domain.
type TransactionLogic interface {
	// RebuildState reconstructs transaction state from an event book.
	RebuildState(eventBook *angzarr.EventBook) *TransactionState

	// HandleCreateTransaction handles the CreateTransaction command.
	HandleCreateTransaction(state *TransactionState, customerID string, items []*examples.LineItem) (*examples.TransactionCreated, error)

	// HandleApplyDiscount handles the ApplyDiscount command.
	HandleApplyDiscount(state *TransactionState, discountType string, value int32, couponCode string) (*examples.DiscountApplied, error)

	// HandleCompleteTransaction handles the CompleteTransaction command.
	HandleCompleteTransaction(state *TransactionState, paymentMethod string) (*examples.TransactionCompleted, error)

	// HandleCancelTransaction handles the CancelTransaction command.
	HandleCancelTransaction(state *TransactionState, reason string) (*examples.TransactionCancelled, error)
}

// DefaultTransactionLogic is the default implementation of TransactionLogic.
type DefaultTransactionLogic struct{}

// NewTransactionLogic creates a new TransactionLogic instance.
func NewTransactionLogic() TransactionLogic {
	return &DefaultTransactionLogic{}
}

// RebuildState reconstructs transaction state from events.
func (l *DefaultTransactionLogic) RebuildState(eventBook *angzarr.EventBook) *TransactionState {
	state := EmptyState()

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	// Apply events
	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.TransactionCreated{}):
			var event examples.TransactionCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.CustomerID = event.CustomerId
				state.Items = event.Items
				state.SubtotalCents = event.SubtotalCents
				state.Status = "pending"
			}

		case page.Event.MessageIs(&examples.DiscountApplied{}):
			var event examples.DiscountApplied
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.DiscountCents = event.DiscountCents
				state.DiscountType = event.DiscountType
			}

		case page.Event.MessageIs(&examples.TransactionCompleted{}):
			state.Status = "completed"

		case page.Event.MessageIs(&examples.TransactionCancelled{}):
			state.Status = "cancelled"
		}
	}

	return state
}

// HandleCreateTransaction validates and creates a TransactionCreated event.
func (l *DefaultTransactionLogic) HandleCreateTransaction(state *TransactionState, customerID string, items []*examples.LineItem) (*examples.TransactionCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Transaction already exists")
	}

	if customerID == "" {
		return nil, NewInvalidArgument("customer_id is required")
	}
	if len(items) == 0 {
		return nil, NewInvalidArgument("at least one item is required")
	}

	var subtotal int32
	for _, item := range items {
		subtotal += item.Quantity * item.UnitPriceCents
	}

	return &examples.TransactionCreated{
		CustomerId:    customerID,
		Items:         items,
		SubtotalCents: subtotal,
		CreatedAt:     timestamppb.Now(),
	}, nil
}

// HandleApplyDiscount validates and creates a DiscountApplied event.
func (l *DefaultTransactionLogic) HandleApplyDiscount(state *TransactionState, discountType string, value int32, couponCode string) (*examples.DiscountApplied, error) {
	if !state.IsPending() {
		return nil, NewFailedPrecondition("Can only apply discount to pending transaction")
	}

	var discountCents int32
	switch discountType {
	case "percentage":
		if value < 0 || value > 100 {
			return nil, NewInvalidArgument("Percentage must be 0-100")
		}
		discountCents = (state.SubtotalCents * value) / 100
	case "fixed":
		discountCents = value
		if discountCents > state.SubtotalCents {
			discountCents = state.SubtotalCents
		}
	case "coupon":
		discountCents = 500 // $5 off
	default:
		return nil, NewInvalidArgument("Unknown discount type: " + discountType)
	}

	return &examples.DiscountApplied{
		DiscountType:  discountType,
		Value:         value,
		DiscountCents: discountCents,
		CouponCode:    couponCode,
	}, nil
}

// HandleCompleteTransaction validates and creates a TransactionCompleted event.
func (l *DefaultTransactionLogic) HandleCompleteTransaction(state *TransactionState, paymentMethod string) (*examples.TransactionCompleted, error) {
	if !state.IsPending() {
		return nil, NewFailedPrecondition("Can only complete pending transaction")
	}

	finalTotal := state.SubtotalCents - state.DiscountCents
	if finalTotal < 0 {
		finalTotal = 0
	}
	loyaltyPoints := finalTotal / 100 // 1 point per dollar

	return &examples.TransactionCompleted{
		FinalTotalCents:     finalTotal,
		PaymentMethod:       paymentMethod,
		LoyaltyPointsEarned: loyaltyPoints,
		CompletedAt:         timestamppb.Now(),
	}, nil
}

// HandleCancelTransaction validates and creates a TransactionCancelled event.
func (l *DefaultTransactionLogic) HandleCancelTransaction(state *TransactionState, reason string) (*examples.TransactionCancelled, error) {
	if !state.IsPending() {
		return nil, NewFailedPrecondition("Can only cancel pending transaction")
	}

	return &examples.TransactionCancelled{
		Reason:      reason,
		CancelledAt: timestamppb.Now(),
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
