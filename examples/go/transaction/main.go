// Package main provides the Transaction bounded context business logic.
// Handles purchases, discounts, and transaction lifecycle.
package main

/*
#include <stdlib.h>
*/
import "C"

import (
	"fmt"
	"unsafe"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"

	"transaction/proto/evented"
	"transaction/proto/examples"
)

const Domain = "transaction"

// TransactionState represents the current state of a transaction.
type TransactionState struct {
	CustomerID    string
	Items         []*examples.LineItem
	SubtotalCents int32
	DiscountCents int32
	DiscountType  string
	Status        string // "pending", "completed", "cancelled"
}

// rebuildState reconstructs transaction state from events.
func rebuildState(eventBook *evented.EventBook) *TransactionState {
	state := &TransactionState{
		Status: "new",
	}

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

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

// calculateSubtotal computes the subtotal from line items.
func calculateSubtotal(items []*examples.LineItem) int32 {
	var total int32
	for _, item := range items {
		total += item.Quantity * item.UnitPriceCents
	}
	return total
}

// calculateLoyaltyPoints determines points earned (1 point per dollar).
func calculateLoyaltyPoints(totalCents int32) int32 {
	return totalCents / 100
}

// Handle processes a contextual command and returns resulting events.
//
//export Handle
func Handle(data *C.char, length C.int) (*C.char, C.int) {
	inputBytes := C.GoBytes(unsafe.Pointer(data), length)

	var contextualCommand evented.ContextualCommand
	if err := goproto.Unmarshal(inputBytes, &contextualCommand); err != nil {
		return nil, 0
	}

	eventBook, err := handle(&contextualCommand)
	if err != nil {
		// Return error as a rejected event (in real impl, use proper error handling)
		return nil, 0
	}

	outputBytes, err := goproto.Marshal(eventBook)
	if err != nil {
		return nil, 0
	}

	return (*C.char)(C.CBytes(outputBytes)), C.int(len(outputBytes))
}

// Domains returns the list of domains this logic handles.
//
//export Domains
func Domains() (*C.char, C.int) {
	domains := Domain
	return C.CString(domains), C.int(len(domains))
}

func handle(contextualCommand *evented.ContextualCommand) (*evented.EventBook, error) {
	commandBook := contextualCommand.Command
	priorEvents := contextualCommand.Events

	// Rebuild current state
	state := rebuildState(priorEvents)

	// Get command from first page
	if len(commandBook.Pages) == 0 {
		return nil, fmt.Errorf("CommandBook has no pages")
	}

	commandAny := commandBook.Pages[0].Command
	if commandAny == nil {
		return nil, fmt.Errorf("Command is nil")
	}

	// Route to handler based on command type
	switch {
	case commandAny.MessageIs(&examples.CreateTransaction{}):
		return handleCreateTransaction(commandBook, commandAny, state)
	case commandAny.MessageIs(&examples.ApplyDiscount{}):
		return handleApplyDiscount(commandBook, commandAny, state)
	case commandAny.MessageIs(&examples.CompleteTransaction{}):
		return handleCompleteTransaction(commandBook, commandAny, state)
	case commandAny.MessageIs(&examples.CancelTransaction{}):
		return handleCancelTransaction(commandBook, commandAny, state)
	default:
		return nil, fmt.Errorf("unknown command type: %s", commandAny.TypeUrl)
	}
}

func handleCreateTransaction(commandBook *evented.CommandBook, commandAny *anypb.Any, state *TransactionState) (*evented.EventBook, error) {
	// Validate: transaction shouldn't already exist
	if state.Status != "new" {
		return nil, fmt.Errorf("transaction already exists")
	}

	var cmd examples.CreateTransaction
	if err := commandAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Validate
	if cmd.CustomerId == "" {
		return nil, fmt.Errorf("customer_id is required")
	}
	if len(cmd.Items) == 0 {
		return nil, fmt.Errorf("at least one item is required")
	}

	subtotal := calculateSubtotal(cmd.Items)

	event := &examples.TransactionCreated{
		CustomerId:    cmd.CustomerId,
		Items:         cmd.Items,
		SubtotalCents: subtotal,
		CreatedAt:     timestamppb.Now(),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &evented.EventBook{
		Cover: commandBook.Cover,
		Pages: []*evented.EventPage{
			{
				Sequence:  &evented.EventPage_Num{Num: 0},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

func handleApplyDiscount(commandBook *evented.CommandBook, commandAny *anypb.Any, state *TransactionState) (*evented.EventBook, error) {
	// Validate: transaction must be pending
	if state.Status != "pending" {
		return nil, fmt.Errorf("can only apply discount to pending transaction")
	}

	var cmd examples.ApplyDiscount
	if err := commandAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Calculate discount
	var discountCents int32
	switch cmd.DiscountType {
	case "percentage":
		if cmd.Value < 0 || cmd.Value > 100 {
			return nil, fmt.Errorf("percentage must be 0-100")
		}
		discountCents = (state.SubtotalCents * cmd.Value) / 100
	case "fixed":
		discountCents = cmd.Value
		if discountCents > state.SubtotalCents {
			discountCents = state.SubtotalCents
		}
	case "coupon":
		// Coupon codes give fixed discounts (simplified)
		discountCents = 500 // $5 off
	default:
		return nil, fmt.Errorf("unknown discount type: %s", cmd.DiscountType)
	}

	event := &examples.DiscountApplied{
		DiscountType:  cmd.DiscountType,
		Value:         cmd.Value,
		DiscountCents: discountCents,
		CouponCode:    cmd.CouponCode,
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &evented.EventBook{
		Cover: commandBook.Cover,
		Pages: []*evented.EventPage{
			{
				Sequence:  &evented.EventPage_Num{Num: 0},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

func handleCompleteTransaction(commandBook *evented.CommandBook, commandAny *anypb.Any, state *TransactionState) (*evented.EventBook, error) {
	// Validate: transaction must be pending
	if state.Status != "pending" {
		return nil, fmt.Errorf("can only complete pending transaction")
	}

	var cmd examples.CompleteTransaction
	if err := commandAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	finalTotal := state.SubtotalCents - state.DiscountCents
	if finalTotal < 0 {
		finalTotal = 0
	}

	loyaltyPoints := calculateLoyaltyPoints(finalTotal)

	event := &examples.TransactionCompleted{
		FinalTotalCents:      finalTotal,
		PaymentMethod:        cmd.PaymentMethod,
		LoyaltyPointsEarned:  loyaltyPoints,
		CompletedAt:          timestamppb.Now(),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &evented.EventBook{
		Cover: commandBook.Cover,
		Pages: []*evented.EventPage{
			{
				Sequence:  &evented.EventPage_Num{Num: 0},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

func handleCancelTransaction(commandBook *evented.CommandBook, commandAny *anypb.Any, state *TransactionState) (*evented.EventBook, error) {
	// Validate: transaction must be pending
	if state.Status != "pending" {
		return nil, fmt.Errorf("can only cancel pending transaction")
	}

	var cmd examples.CancelTransaction
	if err := commandAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	event := &examples.TransactionCancelled{
		Reason:      cmd.Reason,
		CancelledAt: timestamppb.Now(),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &evented.EventBook{
		Cover: commandBook.Cover,
		Pages: []*evented.EventPage{
			{
				Sequence:  &evented.EventPage_Num{Num: 0},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

func main() {
	// Required for cgo shared library
	fmt.Println("Transaction business logic loaded")
}
