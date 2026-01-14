package logic

import (
	"fmt"
	"strings"

	"projector-receipt/proto/angzarr"
	"projector-receipt/proto/examples"
)

// ReceiptProjectorLogic provides business logic operations for the receipt projector.
type ReceiptProjectorLogic interface {
	// RebuildState reconstructs transaction state from an event book.
	RebuildState(eventBook *angzarr.EventBook) *TransactionState

	// GenerateReceipt creates a receipt from completed transaction state.
	// Returns nil if transaction is not completed.
	GenerateReceipt(transactionID string, state *TransactionState) *examples.Receipt

	// FormatReceipt generates the human-readable receipt text.
	FormatReceipt(transactionID string, state *TransactionState) string
}

// DefaultReceiptProjectorLogic is the default implementation of ReceiptProjectorLogic.
type DefaultReceiptProjectorLogic struct{}

// NewReceiptProjectorLogic creates a new ReceiptProjectorLogic instance.
func NewReceiptProjectorLogic() ReceiptProjectorLogic {
	return &DefaultReceiptProjectorLogic{}
}

// RebuildState reconstructs transaction state from events.
func (l *DefaultReceiptProjectorLogic) RebuildState(eventBook *angzarr.EventBook) *TransactionState {
	state := EmptyTransactionState()

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
			}

		case page.Event.MessageIs(&examples.DiscountApplied{}):
			var event examples.DiscountApplied
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.DiscountType = event.DiscountType
				state.DiscountCents = event.DiscountCents
			}

		case page.Event.MessageIs(&examples.TransactionCompleted{}):
			var event examples.TransactionCompleted
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.FinalTotalCents = event.FinalTotalCents
				state.PaymentMethod = event.PaymentMethod
				state.LoyaltyPointsEarned = event.LoyaltyPointsEarned
				state.Completed = true
			}
		}
	}

	return state
}

// GenerateReceipt creates a receipt from completed transaction state.
func (l *DefaultReceiptProjectorLogic) GenerateReceipt(transactionID string, state *TransactionState) *examples.Receipt {
	if !state.IsComplete() {
		return nil
	}

	receiptText := l.FormatReceipt(transactionID, state)

	return &examples.Receipt{
		TransactionId:       transactionID,
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
func (l *DefaultReceiptProjectorLogic) FormatReceipt(transactionID string, state *TransactionState) string {
	var lines []string

	shortTxID := transactionID
	if len(shortTxID) > 16 {
		shortTxID = shortTxID[:16]
	}

	shortCustID := state.CustomerID
	if len(shortCustID) > 16 {
		shortCustID = shortCustID[:16]
	}

	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, "           RECEIPT")
	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, fmt.Sprintf("Transaction: %s...", shortTxID))
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
		lines = append(lines, fmt.Sprintf("Discount (%s):       -$%.2f",
			state.DiscountType,
			float64(state.DiscountCents)/100))
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
