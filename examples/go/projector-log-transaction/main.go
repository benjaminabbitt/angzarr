// Package main provides the Transaction Log Projector - Go Implementation.
// Pretty prints transaction events to terminal.
package main

/*
#include <stdlib.h>
*/
import "C"

import (
	"fmt"
	"unsafe"

	goproto "google.golang.org/protobuf/proto"

	"common"
	"projector-log-transaction/proto/evented"
)

const ProjectorName = "log-transaction"

// Handle processes an EventBook and logs events (returns nil - no projection).
//
//export Handle
func Handle(data *C.char, length C.int) (*C.char, C.int) {
	inputBytes := C.GoBytes(unsafe.Pointer(data), length)

	var eventBook evented.EventBook
	if err := goproto.Unmarshal(inputBytes, &eventBook); err != nil {
		return nil, 0
	}

	logEvents(&eventBook)

	// Log projector doesn't produce a projection
	return nil, 0
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
	return 0 // false
}

func logEvents(eventBook *evented.EventBook) {
	domain := ""
	rootID := ""
	if eventBook.Cover != nil {
		domain = eventBook.Cover.Domain
		if eventBook.Cover.Root != nil {
			rootID = fmt.Sprintf("%x", eventBook.Cover.Root.Value)
			if len(rootID) > 16 {
				rootID = rootID[:16]
			}
		}
	}

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		var sequence uint32
		if num, ok := page.Sequence.(*evented.EventPage_Num); ok {
			sequence = num.Num
		}

		common.LogEvent(domain, rootID, sequence, page.Event.GetTypeUrl(), page.Event.GetValue())
	}
}

func main() {
	fmt.Println("Transaction Log Projector (Go) loaded")
	fmt.Println("Listening to domain: transaction")
}
