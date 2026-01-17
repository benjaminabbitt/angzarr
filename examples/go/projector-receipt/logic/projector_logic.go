package logic

import (
	"fmt"
	"strings"

	"projector-receipt/proto/angzarr"
	"projector-receipt/proto/examples"
)

// ReceiptProjectorLogic provides business logic operations for the receipt projector.
type ReceiptProjectorLogic interface {
	// RebuildState reconstructs order state from an event book.
	RebuildState(eventBook *angzarr.EventBook) *OrderState

	// GenerateReceipt creates a receipt from completed order state.
	// Returns nil if order is not completed.
	GenerateReceipt(orderID string, state *OrderState) *examples.Receipt

	// FormatReceipt generates the human-readable receipt text.
	FormatReceipt(orderID string, state *OrderState) string
}

// DefaultReceiptProjectorLogic is the default implementation of ReceiptProjectorLogic.
type DefaultReceiptProjectorLogic struct{}

// NewReceiptProjectorLogic creates a new ReceiptProjectorLogic instance.
func NewReceiptProjectorLogic() ReceiptProjectorLogic {
	return &DefaultReceiptProjectorLogic{}
}

// RebuildState reconstructs order state from events.
func (l *DefaultReceiptProjectorLogic) RebuildState(eventBook *angzarr.EventBook) *OrderState {
	state := EmptyOrderState()

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	// Apply events
	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.OrderCreated{}):
			var event examples.OrderCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.CustomerID = event.CustomerId
				state.Items = event.Items
				state.SubtotalCents = event.SubtotalCents
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
			}

		case page.Event.MessageIs(&examples.OrderCompleted{}):
			var event examples.OrderCompleted
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.FinalTotalCents = event.FinalTotalCents
				state.PaymentReference = event.PaymentReference
				state.LoyaltyPointsEarned = event.LoyaltyPointsEarned
				state.Completed = true
			}
		}
	}

	return state
}

// GenerateReceipt creates a receipt from completed order state.
func (l *DefaultReceiptProjectorLogic) GenerateReceipt(orderID string, state *OrderState) *examples.Receipt {
	if !state.IsComplete() {
		return nil
	}

	receiptText := l.FormatReceipt(orderID, state)

	return &examples.Receipt{
		OrderId:             orderID,
		CustomerId:          state.CustomerID,
		Items:               state.Items,
		SubtotalCents:       state.SubtotalCents,
		DiscountCents:       state.DiscountCents,
		FinalTotalCents:     state.FinalTotalCents,
		PaymentMethod:       state.PaymentMethod,
		LoyaltyPointsEarned: state.LoyaltyPointsEarned,
		FormattedText:       receiptText,
	}
}

// FormatReceipt generates the human-readable receipt text.
func (l *DefaultReceiptProjectorLogic) FormatReceipt(orderID string, state *OrderState) string {
	var lines []string

	shortOrderID := orderID
	if len(shortOrderID) > 16 {
		shortOrderID = shortOrderID[:16]
	}

	shortCustID := state.CustomerID
	if len(shortCustID) > 16 {
		shortCustID = shortCustID[:16]
	}

	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, "           RECEIPT")
	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, fmt.Sprintf("Order: %s...", shortOrderID))
	if state.CustomerID != "" {
		lines = append(lines, fmt.Sprintf("Customer: %s...", shortCustID))
	} else {
		lines = append(lines, "Customer: N/A")
	}
	lines = append(lines, strings.Repeat("─", 40))

	// Items
	for _, item := range state.Items {
		lineTotal := item.Quantity * item.UnitPriceCents
		lines = append(lines, fmt.Sprintf("%d x %s @ $%.2f = $%.2f",
			item.Quantity,
			item.Name,
			float64(item.UnitPriceCents)/100,
			float64(lineTotal)/100))
	}

	lines = append(lines, strings.Repeat("─", 40))
	lines = append(lines, fmt.Sprintf("Subtotal:              $%.2f", float64(state.SubtotalCents)/100))

	if state.DiscountCents > 0 {
		lines = append(lines, fmt.Sprintf("Loyalty Discount:     -$%.2f",
			float64(state.DiscountCents)/100))
		lines = append(lines, fmt.Sprintf("  (Used %d points)", state.LoyaltyPointsUsed))
	}

	lines = append(lines, strings.Repeat("─", 40))
	lines = append(lines, fmt.Sprintf("TOTAL:                 $%.2f", float64(state.FinalTotalCents)/100))
	lines = append(lines, fmt.Sprintf("Payment: %s", state.PaymentMethod))
	lines = append(lines, strings.Repeat("─", 40))
	lines = append(lines, fmt.Sprintf("Loyalty Points Earned: %d", state.LoyaltyPointsEarned))
	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, "     Thank you for your purchase!")
	lines = append(lines, strings.Repeat("═", 40))

	return strings.Join(lines, "\n")
}
