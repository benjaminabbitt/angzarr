// Package logic provides pure business logic for the transaction log projector.
// This package has no gRPC dependencies and can be tested in isolation.
package logic

import (
	"encoding/hex"
	"strings"

	"projector-log-transaction/proto/angzarr"
	"projector-log-transaction/proto/examples"

	goproto "google.golang.org/protobuf/proto"
)

// LogEntry represents a structured log entry for an event.
type LogEntry struct {
	Domain    string
	RootID    string
	Sequence  uint32
	EventType string
	Fields    map[string]interface{}
	IsUnknown bool
}

// LogProjectorLogic provides business logic operations for the transaction log projector.
type LogProjectorLogic interface {
	// ProcessEventBook processes all events in an event book and returns log entries.
	ProcessEventBook(eventBook *angzarr.EventBook) []LogEntry
}

// DefaultLogProjectorLogic is the default implementation of LogProjectorLogic.
type DefaultLogProjectorLogic struct{}

// NewLogProjectorLogic creates a new LogProjectorLogic instance.
func NewLogProjectorLogic() LogProjectorLogic {
	return &DefaultLogProjectorLogic{}
}

// ProcessEventBook processes all events in an event book and returns log entries.
func (l *DefaultLogProjectorLogic) ProcessEventBook(eventBook *angzarr.EventBook) []LogEntry {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

	domain := "transaction"
	if eventBook.Cover != nil && eventBook.Cover.Domain != "" {
		domain = eventBook.Cover.Domain
	}

	rootID := ""
	if eventBook.Cover != nil && eventBook.Cover.Root != nil {
		rootID = hex.EncodeToString(eventBook.Cover.Root.Value)
	}
	shortID := rootID
	if len(shortID) > 16 {
		shortID = shortID[:16]
	}

	var entries []LogEntry

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		var sequence uint32
		if seq, ok := page.Sequence.(*angzarr.EventPage_Num); ok {
			sequence = seq.Num
		}

		eventType := page.Event.TypeUrl
		if idx := strings.LastIndex(eventType, "."); idx >= 0 {
			eventType = eventType[idx+1:]
		}

		entry := LogEntry{
			Domain:    domain,
			RootID:    shortID,
			Sequence:  sequence,
			EventType: eventType,
			Fields:    make(map[string]interface{}),
		}

		l.extractEventDetails(&entry, eventType, page.Event.Value)
		entries = append(entries, entry)
	}

	return entries
}

// extractEventDetails populates event-specific fields in the log entry.
func (l *DefaultLogProjectorLogic) extractEventDetails(entry *LogEntry, eventType string, data []byte) {
	switch eventType {
	case "TransactionCreated":
		var event examples.TransactionCreated
		if err := goproto.Unmarshal(data, &event); err == nil {
			custID := event.CustomerId
			if len(custID) > 16 {
				custID = custID[:16]
			}
			entry.Fields["customer_id"] = custID
			entry.Fields["item_count"] = len(event.Items)
			entry.Fields["subtotal_cents"] = event.SubtotalCents
		}

	case "DiscountApplied":
		var event examples.DiscountApplied
		if err := goproto.Unmarshal(data, &event); err == nil {
			entry.Fields["discount_type"] = event.DiscountType
			entry.Fields["value"] = event.Value
			entry.Fields["discount_cents"] = event.DiscountCents
			entry.Fields["coupon_code"] = event.CouponCode
		}

	case "TransactionCompleted":
		var event examples.TransactionCompleted
		if err := goproto.Unmarshal(data, &event); err == nil {
			entry.Fields["final_total_cents"] = event.FinalTotalCents
			entry.Fields["payment_method"] = event.PaymentMethod
			entry.Fields["loyalty_points_earned"] = event.LoyaltyPointsEarned
		}

	case "TransactionCancelled":
		var event examples.TransactionCancelled
		if err := goproto.Unmarshal(data, &event); err == nil {
			entry.Fields["reason"] = event.Reason
		}

	default:
		entry.Fields["raw_bytes"] = len(data)
		entry.IsUnknown = true
	}
}
