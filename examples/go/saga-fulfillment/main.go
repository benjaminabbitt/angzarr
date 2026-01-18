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

	"saga-fulfillment/logic"
	"saga-fulfillment/proto/angzarr"
)

var (
	logger    *zap.Logger
	sagaLogic logic.FulfillmentSagaLogic
)

type server struct {
	angzarr.UnimplementedSagaServer
}

func (s *server) Handle(ctx context.Context, req *angzarr.EventBook) (*angzarr.SagaResponse, error) {
	commands := sagaLogic.ProcessEvents(req)
	if len(commands) > 0 {
		logger.Info("processed events",
			zap.Int("commands_generated", len(commands)))
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

	sagaLogic = logic.NewFulfillmentSagaLogic()

	port := os.Getenv("PORT")
	if port == "" {
		port = "50207"
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
