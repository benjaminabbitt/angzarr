// Package main provides the Loyalty Points Saga - Go Implementation.
// Listens to TransactionCompleted events and sends AddLoyaltyPoints
// commands to the customer domain.
package main

import (
	"context"
	"encoding/hex"
	"fmt"
	"net"
	"os"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/emptypb"

	"saga-loyalty/proto/evented"
	"saga-loyalty/proto/examples"
)

const SagaName = "loyalty_points"

var logger *zap.Logger

// server implements the Saga gRPC service.
type server struct {
	evented.UnimplementedSagaServer
}

// Handle processes events asynchronously (fire-and-forget).
func (s *server) Handle(ctx context.Context, req *evented.EventBook) (*emptypb.Empty, error) {
	// Saga always needs to return commands, so use HandleSync internally
	_, _ = s.HandleSync(ctx, req)
	return &emptypb.Empty{}, nil
}

// HandleSync processes events and returns commands synchronously.
func (s *server) HandleSync(ctx context.Context, req *evented.EventBook) (*evented.SynchronousProcessingResponse, error) {
	commandBooks := processEvents(req)

	return &evented.SynchronousProcessingResponse{
		Commands: commandBooks,
	}, nil
}

// processEvents extracts TransactionCompleted events and generates AddLoyaltyPoints commands.
func processEvents(eventBook *evented.EventBook) []*evented.CommandBook {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

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
			logger.Error("failed to unmarshal TransactionCompleted",
				zap.Error(err))
			continue
		}

		points := event.LoyaltyPointsEarned
		if points <= 0 {
			continue
		}

		// Get customer_id from the transaction cover
		customerID := eventBook.Cover.Root
		if customerID == nil {
			logger.Warn("transaction has no root ID, skipping loyalty points")
			continue
		}

		transactionID := hex.EncodeToString(customerID.Value)
		shortID := transactionID
		if len(shortID) > 16 {
			shortID = shortID[:16]
		}

		logger.Info("awarding loyalty points",
			zap.Int32("points", points),
			zap.String("transaction_id", shortID))

		// Create AddLoyaltyPoints command
		addPointsCmd := &examples.AddLoyaltyPoints{
			Points: points,
			Reason: fmt.Sprintf("transaction:%s", transactionID),
		}

		cmdAny, err := anypb.New(addPointsCmd)
		if err != nil {
			logger.Error("failed to create Any for AddLoyaltyPoints",
				zap.Error(err))
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

func main() {
	var err error
	logger, err = zap.NewProduction()
	if err != nil {
		panic(err)
	}
	defer logger.Sync()

	port := os.Getenv("PORT")
	if port == "" {
		port = "50054"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen",
			zap.String("port", port),
			zap.Error(err))
	}

	s := grpc.NewServer()
	evented.RegisterSagaServer(s, &server{})

	logger.Info("saga server started",
		zap.String("name", SagaName),
		zap.String("port", port),
		zap.String("listens_to", "transaction domain"))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
