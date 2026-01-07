// Package main provides the Receipt Projector - Go Implementation.
// Generates human-readable receipts when transactions complete.
package main

/*
#include <stdlib.h>
*/
import "C"

import (
	"fmt"
	"strings"
	"unsafe"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"

	"projector-receipt/proto/evented"
	"projector-receipt/proto/examples"
)

const ProjectorName = "receipt"

// TransactionState holds the rebuilt state from events.
type TransactionState struct {
	CustomerID          string
	Items               []*examples.LineItem
	SubtotalCents       int32
	DiscountCents       int32
	DiscountType        string
	FinalTotalCents     int32
	PaymentMethod       string
	LoyaltyPointsEarned int32
	Completed           bool
}

// Handle processes an EventBook and returns a Projection.
//
//export Handle
func Handle(data *C.char, length C.int) (*C.char, C.int) {
	inputBytes := C.GoBytes(unsafe.Pointer(data), length)

	var eventBook evented.EventBook
	if err := goproto.Unmarshal(inputBytes, &eventBook); err != nil {
		return nil, 0
	}

	projection := project(&eventBook)
	if projection == nil {
		return nil, 0
	}

	outputBytes, err := goproto.Marshal(projection)
	if err != nil {
		return nil, 0
	}

	return (*C.char)(C.CBytes(outputBytes)), C.int(len(outputBytes))
}

// Name returns the projector name.
//
//export Name
func Name() (*C.char, C.int) {
	return C.CString(ProjectorName), C.int(len(ProjectorName))
}

// Domains returns the domains this projector listens to.
//
//export Domains
func Domains() (*C.char, C.int) {
	domains := "transaction"
	return C.CString(domains), C.int(len(domains))
}

// IsSynchronous returns whether this projector is synchronous.
//
//export IsSynchronous
func IsSynchronous() C.int {
	return 1 // true
}

func project(eventBook *evented.EventBook) *evented.Projection {
	// Rebuild transaction state from all events
	state := &TransactionState{}

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

	// Only generate receipt if transaction completed
	if !state.Completed {
		return nil
	}

	transactionID := ""
	if eventBook.Cover != nil && eventBook.Cover.Root != nil {
		transactionID = fmt.Sprintf("%x", eventBook.Cover.Root.Value)
	}

	// Generate formatted receipt text
	receiptText := formatReceipt(transactionID, state)

	fmt.Printf("[%s] Generated receipt for transaction %s...\n",
		ProjectorName, transactionID[:min(16, len(transactionID))])

	// Create Receipt message
	receipt := &examples.Receipt{
		TransactionId:        transactionID,
		CustomerId:           state.CustomerID,
		Items:                state.Items,
		SubtotalCents:        state.SubtotalCents,
		DiscountCents:        state.DiscountCents,
		FinalTotalCents:      state.FinalTotalCents,
		PaymentMethod:        state.PaymentMethod,
		LoyaltyPointsEarned:  state.LoyaltyPointsEarned,
		FormattedText:        receiptText,
	}

	receiptAny, err := anypb.New(receipt)
	if err != nil {
		return nil
	}

	// Get sequence from last page
	var sequence uint32
	if len(eventBook.Pages) > 0 {
		lastPage := eventBook.Pages[len(eventBook.Pages)-1]
		if num, ok := lastPage.Sequence.(*evented.EventPage_Num); ok {
			sequence = num.Num
		}
	}

	return &evented.Projection{
		Cover:      eventBook.Cover,
		Projector:  ProjectorName,
		Sequence:   sequence,
		Projection: receiptAny,
	}
}

func formatReceipt(transactionID string, state *TransactionState) string {
	var lines []string

	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, "           RECEIPT")
	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, fmt.Sprintf("Transaction: %s...", transactionID[:min(16, len(transactionID))]))
	if state.CustomerID != "" {
		lines = append(lines, fmt.Sprintf("Customer: %s...", state.CustomerID[:min(16, len(state.CustomerID))]))
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

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func main() {
	fmt.Println("Receipt Projector (Go) loaded")
}
