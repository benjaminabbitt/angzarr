package main

import (
	"context"
	"fmt"
	"net"
	"os"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/protobuf/types/known/emptypb"

	"saga-loyalty-earn/logic"
	"saga-loyalty-earn/proto/angzarr"
)

var (
	logger    *zap.Logger
	sagaLogic logic.LoyaltyEarnSagaLogic
)

type server struct {
	angzarr.UnimplementedSagaServer
}

func (s *server) Handle(ctx context.Context, req *angzarr.EventBook) (*emptypb.Empty, error) {
	// For async processing, we need order context to know customer and points
	// In practice, this would be passed via correlation or looked up
	// For now, this is a placeholder
	return &emptypb.Empty{}, nil
}

func (s *server) HandleSync(ctx context.Context, req *angzarr.EventBook) (*angzarr.SagaResponse, error) {
	// Loyalty earn is typically async, but sync method available for testing
	// Customer ID and points would come from saga context/correlation
	return &angzarr.SagaResponse{Commands: nil}, nil
}

func main() {
	var err error
	logger, err = zap.NewProduction()
	if err != nil {
		panic(err)
	}
	defer logger.Sync()

	sagaLogic = logic.NewLoyaltyEarnSagaLogic()

	port := os.Getenv("PORT")
	if port == "" {
		port = "50208"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen", zap.String("port", port), zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterSagaServer(s, &server{})

	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(s, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	logger.Info("saga server started",
		zap.String("saga", logic.SagaName),
		zap.String("port", port),
		zap.String("source_domain", logic.SourceDomain),
		zap.String("target_domain", logic.TargetDomain))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
