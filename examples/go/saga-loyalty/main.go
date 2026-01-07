// Package main provides the Loyalty Points Saga - Go Implementation.
// Listens to TransactionCompleted events and sends AddLoyaltyPoints
// commands to the customer domain.
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

	"saga-loyalty/proto/evented"
	"saga-loyalty/proto/examples"
)

const SagaName = "loyalty_points"

// Handle processes an EventBook and returns CommandBooks.
//
//export Handle
func Handle(data *C.char, length C.int) (*C.char, C.int) {
	inputBytes := C.GoBytes(unsafe.Pointer(data), length)

	var eventBook evented.EventBook
	if err := goproto.Unmarshal(inputBytes, &eventBook); err != nil {
		return nil, 0
	}

	commandBooks := handle(&eventBook)
	if len(commandBooks) == 0 {
		return nil, 0
	}

	// For simplicity, return the first command book
	// In a real implementation, you'd return all of them
	outputBytes, err := goproto.Marshal(commandBooks[0])
	if err != nil {
		return nil, 0
	}

	return (*C.char)(C.CBytes(outputBytes)), C.int(len(outputBytes))
}

// Name returns the saga name.
//
//export Name
func Name() (*C.char, C.int) {
	return C.CString(SagaName), C.int(len(SagaName))
}

// Domains returns the domains this saga listens to.
//
//export Domains
func Domains() (*C.char, C.int) {
	domains := "transaction"
	return C.CString(domains), C.int(len(domains))
}

// IsSynchronous returns whether this saga is synchronous.
//
//export IsSynchronous
func IsSynchronous() C.int {
	return 1 // true
}

func handle(eventBook *evented.EventBook) []*evented.CommandBook {
	var commands []*evented.CommandBook

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		// Check if this is a TransactionCompleted event
		if !page.Event.MessageIs(&examples.TransactionCompleted{}) {
			continue
		}

		var event examples.TransactionCompleted
		if err := page.Event.UnmarshalTo(&event); err != nil {
			continue
		}

		points := event.LoyaltyPointsEarned
		if points <= 0 {
			continue
		}

		// Get customer_id from the transaction cover
		customerID := eventBook.Cover.Root
		if customerID == nil {
			continue
		}

		transactionID := fmt.Sprintf("%x", customerID.Value)

		fmt.Printf("[%s] Awarding %d loyalty points for transaction %s...\n",
			SagaName, points, transactionID[:min(16, len(transactionID))])

		// Create AddLoyaltyPoints command
		addPointsCmd := &examples.AddLoyaltyPoints{
			Points: points,
			Reason: fmt.Sprintf("transaction:%s", transactionID),
		}

		cmdAny, err := anypb.New(addPointsCmd)
		if err != nil {
			continue
		}

		commandBook := &evented.CommandBook{
			Cover: &evented.Cover{
				Domain: "customer",
				Root:   customerID,
			},
			Pages: []*evented.CommandPage{
				{
					Sequence:    0,
					Synchronous: false,
					Command:     cmdAny,
				},
			},
		}

		commands = append(commands, commandBook)
	}

	return commands
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func main() {
	fmt.Println("Loyalty Points Saga (Go) loaded")
}
