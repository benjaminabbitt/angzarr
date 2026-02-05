package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

// LineItem represents an item in the order.
type LineItem struct {
	ProductID      string
	Name           string
	Quantity       int32
	UnitPriceCents int32
}

// OrderState represents the current state of an order aggregate.
type OrderState struct {
	CustomerID        string
	Items             []LineItem
	SubtotalCents     int32
	DiscountCents     int32
	LoyaltyPointsUsed int32
	PaymentMethod     string
	PaymentReference  string
	Status            OrderStatus
	CustomerRoot      []byte
	CartRoot          []byte
}

// Exists returns true if the order has been created.
func (s *OrderState) Exists() bool {
	return s.CustomerID != ""
}

// IsPending returns true if the order is in pending state.
func (s *OrderState) IsPending() bool {
	return s.Status == OrderStatusPending
}

// IsPaymentSubmitted returns true if payment has been submitted.
func (s *OrderState) IsPaymentSubmitted() bool {
	return s.Status == OrderStatusPaymentSubmitted
}

// IsCompleted returns true if the order is completed.
func (s *OrderState) IsCompleted() bool {
	return s.Status == OrderStatusCompleted
}

// IsCancelled returns true if the order is cancelled.
func (s *OrderState) IsCancelled() bool {
	return s.Status == OrderStatusCancelled
}

// TotalAfterDiscount returns the subtotal minus any discount.
func (s *OrderState) TotalAfterDiscount() int32 {
	return s.SubtotalCents - s.DiscountCents
}

// ============================================================================
// Named event appliers
// ============================================================================

func applyOrderCreated(state *OrderState, event *anypb.Any) {
	var e examples.OrderCreated
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.CustomerID = e.CustomerId
	state.SubtotalCents = e.SubtotalCents
	state.Status = OrderStatusPending
	state.CustomerRoot = e.CustomerRoot
	state.CartRoot = e.CartRoot
	state.Items = make([]LineItem, 0, len(e.Items))
	for _, item := range e.Items {
		state.Items = append(state.Items, LineItem{
			ProductID:      item.ProductId,
			Name:           item.Name,
			Quantity:       item.Quantity,
			UnitPriceCents: item.UnitPriceCents,
		})
	}
}

func applyLoyaltyDiscount(state *OrderState, event *anypb.Any) {
	var e examples.LoyaltyDiscountApplied
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.LoyaltyPointsUsed = e.PointsUsed
	state.DiscountCents = e.DiscountCents
}

func applyPaymentSubmitted(state *OrderState, event *anypb.Any) {
	var e examples.PaymentSubmitted
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.PaymentMethod = e.PaymentMethod
	state.Status = OrderStatusPaymentSubmitted
}

func applyOrderCompleted(state *OrderState, event *anypb.Any) {
	var e examples.OrderCompleted
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.PaymentReference = e.PaymentReference
	state.Status = OrderStatusCompleted
}

func applyOrderCancelled(state *OrderState, _ *anypb.Any) {
	state.Status = OrderStatusCancelled
}

func loadOrderSnapshot(state *OrderState, snapshot *anypb.Any) {
	var snapState examples.OrderState
	if err := snapshot.UnmarshalTo(&snapState); err != nil {
		return
	}
	state.CustomerID = snapState.CustomerId
	state.SubtotalCents = snapState.SubtotalCents
	state.DiscountCents = snapState.DiscountCents
	state.LoyaltyPointsUsed = snapState.LoyaltyPointsUsed
	state.PaymentMethod = snapState.PaymentMethod
	state.PaymentReference = snapState.PaymentReference
	state.Status = OrderStatus(snapState.Status)
	state.CustomerRoot = snapState.CustomerRoot
	state.CartRoot = snapState.CartRoot
	for _, item := range snapState.Items {
		state.Items = append(state.Items, LineItem{
			ProductID:      item.ProductId,
			Name:           item.Name,
			Quantity:       item.Quantity,
			UnitPriceCents: item.UnitPriceCents,
		})
	}
}

// ============================================================================
// State rebuilding
// ============================================================================

// stateBuilder is the single source of truth for event type → applier mapping.
var stateBuilder = angzarr.NewStateBuilder(func() OrderState { return OrderState{} }).
	WithSnapshot(loadOrderSnapshot).
	On(angzarr.Name(&examples.OrderCreated{}), applyOrderCreated).
	On(angzarr.Name(&examples.LoyaltyDiscountApplied{}), applyLoyaltyDiscount).
	On(angzarr.Name(&examples.PaymentSubmitted{}), applyPaymentSubmitted).
	On(angzarr.Name(&examples.OrderCompleted{}), applyOrderCompleted).
	On(angzarr.Name(&examples.OrderCancelled{}), applyOrderCancelled)

// RebuildState reconstructs order state from an event book.
func RebuildState(eventBook *angzarrpb.EventBook) OrderState {
	return stateBuilder.Rebuild(eventBook)
}

// ApplyEvent applies a single event to the order state.
func ApplyEvent(state *OrderState, event *anypb.Any) {
	stateBuilder.Apply(state, event)
}
