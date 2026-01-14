// Package main provides the Transaction Log Projector - Go Implementation.
// Logs transaction events using structured logging.
package main

import (
	"context"
	"encoding/hex"
	"fmt"
	"net"
	"os"
	"strings"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/emptypb"

	"projector-log-transaction/proto/angzarr"
	"projector-log-transaction/proto/examples"
)

const ProjectorName = "log-transaction"

var logger *zap.Logger

// server implements the ProjectorCoordinator gRPC service.
type server struct {
	angzarr.UnimplementedProjectorCoordinatorServer
}

// Handle processes events asynchronously (fire-and-forget).
func (s *server) Handle(ctx context.Context, req *angzarr.EventBook) (*emptypb.Empty, error) {
	logEvents(req)
	return &emptypb.Empty{}, nil
}

// HandleSync processes events and returns projection synchronously.
func (s *server) HandleSync(ctx context.Context, req *angzarr.EventBook) (*angzarr.Projection, error) {
	logEvents(req)
	// Log projector doesn't produce a projection
	return nil, nil
}

// logEvents logs all events in the event book.
func logEvents(eventBook *angzarr.EventBook) {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return
	}

	domain := "transaction"
	if eventBook.Cover != nil {
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

		// Log base event info
		eventLogger := logger.With(
			zap.String("domain", domain),
			zap.String("root_id", shortID),
			zap.Uint32("sequence", sequence),
			zap.String("event_type", eventType),
		)

		// Add event-specific fields
		logEventDetails(eventLogger, eventType, page.Event.Value)
	}
}

// logEventDetails adds event-specific fields to the log.
func logEventDetails(eventLogger *zap.Logger, eventType string, data []byte) {
	switch eventType {
	case "TransactionCreated":
		var event examples.TransactionCreated
		if err := goproto.Unmarshal(data, &event); err == nil {
			custID := event.CustomerId
			if len(custID) > 16 {
				custID = custID[:16]
			}
			eventLogger.Info("event",
				zap.String("customer_id", custID),
				zap.Int("item_count", len(event.Items)),
				zap.Int32("subtotal_cents", event.SubtotalCents))
		} else {
			eventLogger.Info("event", zap.Error(err))
		}

	case "DiscountApplied":
		var event examples.DiscountApplied
		if err := goproto.Unmarshal(data, &event); err == nil {
			eventLogger.Info("event",
				zap.String("discount_type", event.DiscountType),
				zap.Int32("value", event.Value),
				zap.Int32("discount_cents", event.DiscountCents),
				zap.String("coupon_code", event.CouponCode))
		} else {
			eventLogger.Info("event", zap.Error(err))
		}

	case "TransactionCompleted":
		var event examples.TransactionCompleted
		if err := goproto.Unmarshal(data, &event); err == nil {
			eventLogger.Info("event",
				zap.Int32("final_total_cents", event.FinalTotalCents),
				zap.String("payment_method", event.PaymentMethod),
				zap.Int32("loyalty_points_earned", event.LoyaltyPointsEarned))
		} else {
			eventLogger.Info("event", zap.Error(err))
		}

	case "TransactionCancelled":
		var event examples.TransactionCancelled
		if err := goproto.Unmarshal(data, &event); err == nil {
			eventLogger.Info("event",
				zap.String("reason", event.Reason))
		} else {
			eventLogger.Info("event", zap.Error(err))
		}

	default:
		eventLogger.Info("event",
			zap.Int("raw_bytes", len(data)))
	}
}

func main() {
	var err error
	logger, err = zap.NewProduction()
	if err != nil {
		panic(err)
	}
	defer logger.Sync()

	port := os.Getenv("PORT")
	if port == "" {
		port = "50057"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen",
			zap.String("port", port),
			zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterProjectorCoordinatorServer(s, &server{})

	// Register gRPC health service
	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(s, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	logger.Info("projector server started",
		zap.String("name", ProjectorName),
		zap.String("port", port),
		zap.String("listens_to", "transaction domain"))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
