package logic

import (
	"encoding/hex"
	"strings"
	"time"

	"go.uber.org/zap"
	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"

	"projector-log-customer/proto/angzarr"
	"projector-log-customer/proto/examples"
)

// LogResult contains the result of processing an event for logging.
type LogResult struct {
	Domain    string
	RootID    string
	Sequence  uint32
	EventType string
	Fields    map[string]interface{}
	Unknown   bool
}

// ProjectorLogic provides business logic for the log projector.
type ProjectorLogic interface {
	// ProcessEventBook processes all events in an event book and returns log results.
	ProcessEventBook(eventBook *angzarr.EventBook) []LogResult

	// ProcessEventPage processes a single event page and returns a log result.
	ProcessEventPage(domain, rootID string, page *angzarr.EventPage) LogResult
}

// DefaultProjectorLogic is the default implementation of ProjectorLogic.
type DefaultProjectorLogic struct{}

// NewProjectorLogic creates a new ProjectorLogic instance.
func NewProjectorLogic() ProjectorLogic {
	return &DefaultProjectorLogic{}
}

// ProcessEventBook processes all events in an event book.
func (l *DefaultProjectorLogic) ProcessEventBook(eventBook *angzarr.EventBook) []LogResult {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

	domain := "customer"
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

	var results []LogResult
	for _, page := range eventBook.Pages {
		result := l.ProcessEventPage(domain, shortID, page)
		results = append(results, result)
	}

	return results
}

// ProcessEventPage processes a single event page.
func (l *DefaultProjectorLogic) ProcessEventPage(domain, rootID string, page *angzarr.EventPage) LogResult {
	result := LogResult{
		Domain: domain,
		RootID: rootID,
		Fields: make(map[string]interface{}),
	}

	if page == nil || page.Event == nil {
		result.Unknown = true
		return result
	}

	if seq, ok := page.Sequence.(*angzarr.EventPage_Num); ok {
		result.Sequence = seq.Num
	}

	eventType := page.Event.TypeUrl
	if idx := strings.LastIndex(eventType, "."); idx >= 0 {
		eventType = eventType[idx+1:]
	}
	result.EventType = eventType

	l.extractEventFields(&result, eventType, page.Event.Value)

	return result
}

// extractEventFields extracts event-specific fields into the result.
func (l *DefaultProjectorLogic) extractEventFields(result *LogResult, eventType string, data []byte) {
	switch eventType {
	case "CustomerCreated":
		var event examples.CustomerCreated
		if err := goproto.Unmarshal(data, &event); err == nil {
			result.Fields["name"] = event.Name
			result.Fields["email"] = event.Email
			if event.CreatedAt != nil {
				result.Fields["created_at"] = formatTimestamp(event.CreatedAt)
			}
		}

	case "LoyaltyPointsAdded":
		var event examples.LoyaltyPointsAdded
		if err := goproto.Unmarshal(data, &event); err == nil {
			result.Fields["points"] = event.Points
			result.Fields["new_balance"] = event.NewBalance
			result.Fields["reason"] = event.Reason
		}

	case "LoyaltyPointsRedeemed":
		var event examples.LoyaltyPointsRedeemed
		if err := goproto.Unmarshal(data, &event); err == nil {
			result.Fields["points"] = event.Points
			result.Fields["new_balance"] = event.NewBalance
			result.Fields["redemption_type"] = event.RedemptionType
		}

	case "TransactionCreated":
		var event examples.TransactionCreated
		if err := goproto.Unmarshal(data, &event); err == nil {
			result.Fields["customer_id"] = event.CustomerId
			result.Fields["subtotal_cents"] = event.SubtotalCents
			if event.CreatedAt != nil {
				result.Fields["created_at"] = formatTimestamp(event.CreatedAt)
			}
		}

	case "TransactionCompleted":
		var event examples.TransactionCompleted
		if err := goproto.Unmarshal(data, &event); err == nil {
			result.Fields["final_total_cents"] = event.FinalTotalCents
			result.Fields["payment_method"] = event.PaymentMethod
			result.Fields["loyalty_points_earned"] = event.LoyaltyPointsEarned
			if event.CompletedAt != nil {
				result.Fields["completed_at"] = formatTimestamp(event.CompletedAt)
			}
		}

	default:
		result.Unknown = true
		result.Fields["raw_bytes"] = len(data)
	}
}

// formatTimestamp converts a protobuf Timestamp to RFC 3339 string.
func formatTimestamp(ts *timestamppb.Timestamp) string {
	if ts == nil {
		return ""
	}
	return ts.AsTime().Format(time.RFC3339Nano)
}

// LogEvents logs all events from the log results using the provided logger.
func LogEvents(logger *zap.Logger, results []LogResult) {
	for _, r := range results {
		eventLogger := logger.With(
			zap.String("domain", r.Domain),
			zap.String("root_id", r.RootID),
			zap.Uint32("sequence", r.Sequence),
			zap.String("event_type", r.EventType),
		)

		fields := make([]zap.Field, 0, len(r.Fields))
		for k, v := range r.Fields {
			switch val := v.(type) {
			case string:
				fields = append(fields, zap.String(k, val))
			case int32:
				fields = append(fields, zap.Int32(k, val))
			case int:
				fields = append(fields, zap.Int(k, val))
			default:
				fields = append(fields, zap.Any(k, val))
			}
		}

		eventLogger.Info("event", fields...)
	}
}
