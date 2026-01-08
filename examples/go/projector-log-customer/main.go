// Package main provides the Customer Log Projector - Go Implementation.
// Logs customer events using structured logging.
package main

import (
	"context"
	"encoding/hex"
	"fmt"
	"net"
	"os"
	"strings"
	"time"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/emptypb"
	"google.golang.org/protobuf/types/known/timestamppb"

	"projector-log-customer/proto/evented"
	"projector-log-customer/proto/examples"
)

const ProjectorName = "log-customer"

var logger *zap.Logger

// server implements the ProjectorCoordinator gRPC service.
type server struct {
	evented.UnimplementedProjectorCoordinatorServer
}

// Handle processes events asynchronously (fire-and-forget).
func (s *server) Handle(ctx context.Context, req *evented.EventBook) (*emptypb.Empty, error) {
	logEvents(req)
	return &emptypb.Empty{}, nil
}

// HandleSync processes events and returns projection synchronously.
func (s *server) HandleSync(ctx context.Context, req *evented.EventBook) (*evented.Projection, error) {
	logEvents(req)
	// Log projector doesn't produce a projection
	return nil, nil
}

// logEvents logs all events in the event book.
func logEvents(eventBook *evented.EventBook) {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return
	}

	domain := "customer"
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
		if seq, ok := page.Sequence.(*evented.EventPage_Num); ok {
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

// formatTimestamp converts a protobuf Timestamp to RFC 3339 string.
func formatTimestamp(ts *timestamppb.Timestamp) string {
	if ts == nil {
		return ""
	}
	return ts.AsTime().Format(time.RFC3339Nano)
}

// logEventDetails adds event-specific fields to the log.
func logEventDetails(eventLogger *zap.Logger, eventType string, data []byte) {
	switch eventType {
	case "CustomerCreated":
		var event examples.CustomerCreated
		if err := goproto.Unmarshal(data, &event); err == nil {
			fields := []zap.Field{
				zap.String("name", event.Name),
				zap.String("email", event.Email),
			}
			if event.CreatedAt != nil {
				fields = append(fields, zap.String("created_at", formatTimestamp(event.CreatedAt)))
			}
			eventLogger.Info("event", fields...)
		} else {
			eventLogger.Info("event", zap.Error(err))
		}

	case "LoyaltyPointsAdded":
		var event examples.LoyaltyPointsAdded
		if err := goproto.Unmarshal(data, &event); err == nil {
			eventLogger.Info("event",
				zap.Int32("points", event.Points),
				zap.Int32("new_balance", event.NewBalance),
				zap.String("reason", event.Reason))
		} else {
			eventLogger.Info("event", zap.Error(err))
		}

	case "LoyaltyPointsRedeemed":
		var event examples.LoyaltyPointsRedeemed
		if err := goproto.Unmarshal(data, &event); err == nil {
			eventLogger.Info("event",
				zap.Int32("points", event.Points),
				zap.Int32("new_balance", event.NewBalance),
				zap.String("redemption_type", event.RedemptionType))
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
		port = "50056"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen",
			zap.String("port", port),
			zap.Error(err))
	}

	s := grpc.NewServer()
	evented.RegisterProjectorCoordinatorServer(s, &server{})

	logger.Info("projector server started",
		zap.String("name", ProjectorName),
		zap.String("port", port),
		zap.String("listens_to", "customer domain"))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
