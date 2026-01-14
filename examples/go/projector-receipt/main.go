// Package main provides the Receipt Projector - Go Implementation.
// Generates human-readable receipts when transactions complete.
package main

import (
	"context"
	"encoding/hex"
	"fmt"
	"net"
	"os"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/emptypb"

	"projector-receipt/logic"
	"projector-receipt/proto/angzarr"
)

const ProjectorName = "receipt"

var (
	logger         *zap.Logger
	projectorLogic logic.ReceiptProjectorLogic
)

// server implements the Projector gRPC service.
type server struct {
	angzarr.UnimplementedProjectorServer
}

// Handle processes events asynchronously (fire-and-forget).
func (s *server) Handle(ctx context.Context, req *angzarr.EventBook) (*emptypb.Empty, error) {
	_, _ = s.HandleSync(ctx, req)
	return &emptypb.Empty{}, nil
}

// HandleSync processes events and returns projection synchronously.
func (s *server) HandleSync(ctx context.Context, req *angzarr.EventBook) (*angzarr.Projection, error) {
	return project(req), nil
}

// project rebuilds transaction state and generates a receipt if completed.
func project(eventBook *angzarr.EventBook) *angzarr.Projection {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

	// Rebuild transaction state from all events
	state := projectorLogic.RebuildState(eventBook)

	// Only generate receipt if transaction completed
	if !state.IsComplete() {
		return nil
	}

	transactionID := ""
	if eventBook.Cover != nil && eventBook.Cover.Root != nil {
		transactionID = hex.EncodeToString(eventBook.Cover.Root.Value)
	}

	shortID := transactionID
	if len(shortID) > 16 {
		shortID = shortID[:16]
	}

	// Generate receipt using logic package
	receipt := projectorLogic.GenerateReceipt(transactionID, state)

	logger.Info("generated receipt",
		zap.String("transaction_id", shortID),
		zap.Int32("total_cents", state.FinalTotalCents),
		zap.String("payment_method", state.PaymentMethod))

	receiptAny, err := anypb.New(receipt)
	if err != nil {
		logger.Error("failed to create Any for Receipt",
			zap.Error(err))
		return nil
	}

	// Get sequence from last page
	var sequence uint32
	if len(eventBook.Pages) > 0 {
		lastPage := eventBook.Pages[len(eventBook.Pages)-1]
		if num, ok := lastPage.Sequence.(*angzarr.EventPage_Num); ok {
			sequence = num.Num
		}
	}

	return &angzarr.Projection{
		Cover:      eventBook.Cover,
		Projector:  ProjectorName,
		Sequence:   sequence,
		Projection: receiptAny,
	}
}

func main() {
	var err error
	logger, err = zap.NewProduction()
	if err != nil {
		panic(err)
	}
	defer logger.Sync()

	projectorLogic = logic.NewReceiptProjectorLogic()

	port := os.Getenv("PORT")
	if port == "" {
		port = "50055"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen",
			zap.String("port", port),
			zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterProjectorServer(s, &server{})

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
