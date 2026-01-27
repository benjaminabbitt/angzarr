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

	"saga-cancellation/logic"
	"saga-cancellation/proto/angzarr"
)

var (
	logger    *zap.Logger
	sagaLogic logic.CancellationSagaLogic
)

type server struct {
	angzarr.UnimplementedSagaServer
}

// Prepare: Phase 1 - declare which destination aggregates are needed.
func (s *server) Prepare(ctx context.Context, req *angzarr.SagaPrepareRequest) (*angzarr.SagaPrepareResponse, error) {
	destinations := sagaLogic.Prepare(req.Source)
	return &angzarr.SagaPrepareResponse{Destinations: destinations}, nil
}

// Execute: Phase 2 - produce commands given source and destination state.
func (s *server) Execute(ctx context.Context, req *angzarr.SagaExecuteRequest) (*angzarr.SagaResponse, error) {
	commands := sagaLogic.Execute(req.Source, req.Destinations)
	if len(commands) > 0 {
		logger.Info("processed cancellation",
			zap.Int("compensation_commands", len(commands)))
	}
	return &angzarr.SagaResponse{Commands: commands}, nil
}

// Retry: Phase 2 (alternate) - retry after command rejection.
func (s *server) Retry(ctx context.Context, req *angzarr.SagaRetryRequest) (*angzarr.SagaResponse, error) {
	commands := sagaLogic.Execute(req.Source, req.Destinations)
	if len(commands) > 0 {
		logger.Info("retrying cancellation saga",
			zap.Uint32("attempt", req.Attempt),
			zap.Int("compensation_commands", len(commands)))
	}
	return &angzarr.SagaResponse{Commands: commands}, nil
}

func main() {
	var err error
	logger, err = zap.NewProduction()
	if err != nil {
		panic(err)
	}
	defer logger.Sync()

	sagaLogic = logic.NewCancellationSagaLogic()

	port := os.Getenv("PORT")
	if port == "" {
		port = "50209"
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
		zap.String("source_domain", logic.SourceDomain))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
