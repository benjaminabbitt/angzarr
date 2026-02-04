package angzarr

import (
	"fmt"
	"net"
	"os"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
)

// RegisterFunc registers gRPC services on a server.
type RegisterFunc func(*grpc.Server)

// ServerConfig configures a gRPC server.
type ServerConfig struct {
	Domain      string
	DefaultPort string
}

// RunServer starts a gRPC server with health checks.
//
// Reads PORT from the environment, falling back to cfg.DefaultPort.
// Blocks until the server exits.
func RunServer(cfg ServerConfig, register RegisterFunc) error {
	logger, err := zap.NewProduction()
	if err != nil {
		return fmt.Errorf("failed to create logger: %w", err)
	}
	defer logger.Sync()

	port := os.Getenv("PORT")
	if port == "" {
		port = cfg.DefaultPort
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		return fmt.Errorf("failed to listen on port %s: %w", port, err)
	}

	s := grpc.NewServer()
	register(s)

	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(s, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	logger.Info("client logic server started",
		zap.String("domain", cfg.Domain),
		zap.String("port", port),
	)

	if err := s.Serve(lis); err != nil {
		return fmt.Errorf("failed to serve: %w", err)
	}
	return nil
}
