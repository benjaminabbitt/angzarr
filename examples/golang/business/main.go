// Package main provides a C-shared library for Go business logic integration with evented-rs.
//
// Build with: go build -buildmode=c-shared -o libbusiness.so
package main

/*
#include <stdlib.h>
*/
import "C"
import (
	"fmt"
	"unsafe"

	pb "github.com/benjaminabbitt/evented-rs/examples/golang/business/proto/evented"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// Handle processes a command and returns resulting events.
// Called from Rust via FFI.
//
//export Handle
func Handle(domain *C.char, cmdPtr *C.char, cmdLen C.int) (*C.char, C.int) {
	domainStr := C.GoString(domain)
	cmdBytes := C.GoBytes(unsafe.Pointer(cmdPtr), cmdLen)

	resultBytes, err := handleCommand(domainStr, cmdBytes)
	if err != nil {
		// Return error as negative length with error message
		errMsg := err.Error()
		return C.CString(errMsg), C.int(-len(errMsg))
	}

	// Allocate C memory for result (caller must free)
	cResult := C.CBytes(resultBytes)
	return (*C.char)(cResult), C.int(len(resultBytes))
}

// FreeResult frees memory allocated by Handle.
//
//export FreeResult
func FreeResult(ptr *C.char) {
	C.free(unsafe.Pointer(ptr))
}

// handleCommand routes commands to domain-specific handlers.
func handleCommand(domain string, cmdBytes []byte) ([]byte, error) {
	// Parse ContextualCommand
	cmd := &pb.ContextualCommand{}
	if err := proto.Unmarshal(cmdBytes, cmd); err != nil {
		return nil, fmt.Errorf("failed to unmarshal command: %w", err)
	}

	// Route to domain handler
	var result *pb.EventBook
	var err error

	switch domain {
	case "orders":
		result, err = handleOrders(cmd)
	case "inventory":
		result, err = handleInventory(cmd)
	case "discounts":
		result, err = handleDiscounts(cmd)
	default:
		return nil, fmt.Errorf("unknown domain: %s", domain)
	}

	if err != nil {
		return nil, err
	}

	// Serialize result
	return proto.Marshal(result)
}

// handleOrders processes orders domain commands.
func handleOrders(cmd *pb.ContextualCommand) (*pb.EventBook, error) {
	if cmd.Command == nil || len(cmd.Command.Pages) == 0 {
		return nil, fmt.Errorf("command must have at least one page")
	}

	cover := cmd.Command.Cover
	commandPage := cmd.Command.Pages[0]
	priorEvents := cmd.Events.GetPages()
	nextSeq := uint32(len(priorEvents))

	// Get command type from type URL
	cmdType := getTypeName(commandPage.Command.GetTypeUrl())

	switch cmdType {
	case "CreateOrder":
		return createEvent(cover, nextSeq, "orders.OrderCreated", commandPage.Command.GetValue())

	case "AddItem":
		if nextSeq == 0 {
			return nil, fmt.Errorf("cannot add item to non-existent order")
		}
		return createEvent(cover, nextSeq, "orders.ItemAdded", commandPage.Command.GetValue())

	case "RemoveItem":
		if nextSeq == 0 {
			return nil, fmt.Errorf("cannot remove item from non-existent order")
		}
		return createEvent(cover, nextSeq, "orders.ItemRemoved", commandPage.Command.GetValue())

	case "CompleteOrder":
		if nextSeq == 0 {
			return nil, fmt.Errorf("cannot complete non-existent order")
		}
		// Check order has items
		hasItems := false
		for _, e := range priorEvents {
			if getTypeName(e.Event.GetTypeUrl()) == "ItemAdded" {
				hasItems = true
				break
			}
		}
		if !hasItems {
			return nil, fmt.Errorf("cannot complete order with no items")
		}
		return createEvent(cover, nextSeq, "orders.OrderCompleted", commandPage.Command.GetValue())

	default:
		return nil, fmt.Errorf("unknown orders command: %s", cmdType)
	}
}

// handleInventory processes inventory domain commands.
func handleInventory(cmd *pb.ContextualCommand) (*pb.EventBook, error) {
	if cmd.Command == nil || len(cmd.Command.Pages) == 0 {
		return nil, fmt.Errorf("command must have at least one page")
	}

	cover := cmd.Command.Cover
	commandPage := cmd.Command.Pages[0]
	priorEvents := cmd.Events.GetPages()
	nextSeq := uint32(len(priorEvents))

	cmdType := getTypeName(commandPage.Command.GetTypeUrl())

	switch cmdType {
	case "ReserveStock":
		return createEvent(cover, nextSeq, "inventory.StockReserved", commandPage.Command.GetValue())

	case "ReleaseStock":
		return createEvent(cover, nextSeq, "inventory.StockReleased", commandPage.Command.GetValue())

	default:
		return nil, fmt.Errorf("unknown inventory command: %s", cmdType)
	}
}

// createEvent creates an EventBook with a single event.
func createEvent(cover *pb.Cover, sequence uint32, eventType string, data []byte) (*pb.EventBook, error) {
	return &pb.EventBook{
		Cover: cover,
		Pages: []*pb.EventPage{
			{
				Sequence: &pb.EventPage_Num{Num: sequence},
				Event: &anypb.Any{
					TypeUrl: eventType,
					Value:   data,
				},
				CreatedAt:   timestamppb.Now(),
				Synchronous: false,
			},
		},
	}, nil
}

// handleDiscounts processes discount domain commands.
// Implements the same business rules as the Python discount_logic module.
func handleDiscounts(cmd *pb.ContextualCommand) (*pb.EventBook, error) {
	if cmd.Command == nil || len(cmd.Command.Pages) == 0 {
		return nil, fmt.Errorf("command must have at least one page")
	}

	cover := cmd.Command.Cover
	commandPage := cmd.Command.Pages[0]
	priorEvents := cmd.Events.GetPages()
	nextSeq := uint32(len(priorEvents))

	cmdType := getTypeName(commandPage.Command.GetTypeUrl())

	switch cmdType {
	case "ApplyPercentageDiscount":
		return applyPercentageDiscount(cover, priorEvents, nextSeq, commandPage.Command.GetValue())

	case "ApplyCoupon":
		return applyCoupon(cover, priorEvents, nextSeq, commandPage.Command.GetValue())

	case "RemoveDiscount":
		return removeDiscount(cover, priorEvents, nextSeq, commandPage.Command.GetValue())

	case "CalculateBulkDiscount":
		return calculateBulkDiscount(cover, priorEvents, nextSeq, commandPage.Command.GetValue())

	default:
		return nil, fmt.Errorf("unknown discount command: %s", cmdType)
	}
}

// applyPercentageDiscount applies a percentage discount.
// Rules: order must exist, must have items, no existing discount.
func applyPercentageDiscount(cover *pb.Cover, events []*pb.EventPage, seq uint32, data []byte) (*pb.EventBook, error) {
	if !hasOrder(events) {
		return nil, fmt.Errorf("cannot apply discount: no order exists")
	}
	if !hasItems(events) {
		return nil, fmt.Errorf("cannot apply discount: order has no items")
	}
	if hasActiveDiscount(events) {
		return nil, fmt.Errorf("cannot apply discount: order already has a discount")
	}
	return createEvent(cover, seq, "discounts.DiscountApplied", data)
}

// applyCoupon applies a coupon code. Coupons can stack with other discounts.
func applyCoupon(cover *pb.Cover, events []*pb.EventPage, seq uint32, data []byte) (*pb.EventBook, error) {
	if !hasOrder(events) {
		return nil, fmt.Errorf("cannot apply coupon: no order exists")
	}
	return createEvent(cover, seq, "discounts.CouponApplied", data)
}

// removeDiscount removes an active discount.
func removeDiscount(cover *pb.Cover, events []*pb.EventPage, seq uint32, data []byte) (*pb.EventBook, error) {
	if !hasActiveDiscount(events) {
		return nil, fmt.Errorf("cannot remove discount: no active discount")
	}
	return createEvent(cover, seq, "discounts.DiscountRemoved", data)
}

// calculateBulkDiscount calculates bulk discount for 5+ items.
func calculateBulkDiscount(cover *pb.Cover, events []*pb.EventPage, seq uint32, data []byte) (*pb.EventBook, error) {
	if !hasOrder(events) {
		return nil, fmt.Errorf("cannot calculate bulk discount: no order exists")
	}
	itemCount := countItems(events)
	if itemCount < 5 {
		return nil, fmt.Errorf("cannot apply bulk discount: need 5+ items, have %d", itemCount)
	}
	return createEvent(cover, seq, "discounts.BulkDiscountCalculated", data)
}

// hasOrder checks if OrderCreated event exists.
func hasOrder(events []*pb.EventPage) bool {
	for _, e := range events {
		if getTypeName(e.Event.GetTypeUrl()) == "OrderCreated" {
			return true
		}
	}
	return false
}

// hasItems checks if any ItemAdded events exist.
func hasItems(events []*pb.EventPage) bool {
	for _, e := range events {
		if getTypeName(e.Event.GetTypeUrl()) == "ItemAdded" {
			return true
		}
	}
	return false
}

// hasActiveDiscount checks if there's an active percentage discount.
func hasActiveDiscount(events []*pb.EventPage) bool {
	discountCount := 0
	removedCount := 0
	for _, e := range events {
		switch getTypeName(e.Event.GetTypeUrl()) {
		case "DiscountApplied":
			discountCount++
		case "DiscountRemoved":
			removedCount++
		}
	}
	return discountCount > removedCount
}

// countItems counts ItemAdded events.
func countItems(events []*pb.EventPage) int {
	count := 0
	for _, e := range events {
		if getTypeName(e.Event.GetTypeUrl()) == "ItemAdded" {
			count++
		}
	}
	return count
}

// getTypeName extracts the type name from a protobuf type URL.
func getTypeName(typeUrl string) string {
	for i := len(typeUrl) - 1; i >= 0; i-- {
		if typeUrl[i] == '.' || typeUrl[i] == '/' {
			return typeUrl[i+1:]
		}
	}
	return typeUrl
}

func main() {}
