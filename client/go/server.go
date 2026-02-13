package angzarr

import (
	"context"
	"fmt"
	"net"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"

	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/grpc/reflection"
)

// TransportConfig holds the transport configuration for a gRPC server.
type TransportConfig struct {
	Type    string // "tcp" or "uds"
	Address string // "[::]:port" for TCP or "/path/to/socket" for UDS
}

// GetTransportConfig reads transport configuration from environment.
//
// Environment variables:
//   - TRANSPORT_TYPE: "tcp" (default) or "uds"
//   - UDS_BASE_PATH: Base directory for sockets (default: /tmp/angzarr)
//   - SERVICE_NAME: Service type ("business", "saga", "projector")
//   - DOMAIN: Domain name for aggregates
//   - SAGA_NAME: Saga name (used if DOMAIN not set)
//   - PROJECTOR_NAME: Projector name (used if DOMAIN and SAGA_NAME not set)
//   - PORT: TCP port (default: 50052)
func GetTransportConfig() TransportConfig {
	transport := os.Getenv("TRANSPORT_TYPE")
	if transport == "" {
		transport = "tcp"
	}

	if transport == "uds" {
		basePath := os.Getenv("UDS_BASE_PATH")
		if basePath == "" {
			basePath = "/tmp/angzarr"
		}
		serviceName := os.Getenv("SERVICE_NAME")
		if serviceName == "" {
			serviceName = "business"
		}

		// Get qualifier from DOMAIN, SAGA_NAME, or PROJECTOR_NAME
		qualifier := os.Getenv("DOMAIN")
		if qualifier == "" {
			qualifier = os.Getenv("SAGA_NAME")
		}
		if qualifier == "" {
			qualifier = os.Getenv("PROJECTOR_NAME")
		}

		var socketPath string
		if qualifier != "" {
			socketPath = filepath.Join(basePath, fmt.Sprintf("%s-%s.sock", serviceName, qualifier))
		} else {
			socketPath = filepath.Join(basePath, serviceName+".sock")
		}

		// Ensure parent directory exists
		_ = os.MkdirAll(filepath.Dir(socketPath), 0755)

		// Remove stale socket file if exists
		_ = os.Remove(socketPath)

		return TransportConfig{
			Type:    "uds",
			Address: socketPath,
		}
	}

	port := os.Getenv("PORT")
	if port == "" {
		port = "50052"
	}
	return TransportConfig{
		Type:    "tcp",
		Address: "[::]:" + port,
	}
}

// ServerOptions configures the gRPC server.
type ServerOptions struct {
	ServiceName    string
	Domain         string
	DefaultPort    string
	EnableReflection bool
}

// ServiceRegistrar registers a service with the gRPC server.
type ServiceRegistrar func(server *grpc.Server)

// CreateServer creates a gRPC server with health checking and optional reflection.
//
// Returns: (server, listener, cleanup function)
func CreateServer(registrar ServiceRegistrar, opts ServerOptions) (*grpc.Server, net.Listener, func()) {
	if opts.DefaultPort != "" && os.Getenv("PORT") == "" {
		os.Setenv("PORT", opts.DefaultPort)
	}

	config := GetTransportConfig()

	var listener net.Listener
	var err error

	if config.Type == "uds" {
		listener, err = net.Listen("unix", config.Address)
	} else {
		listener, err = net.Listen("tcp", config.Address)
	}
	if err != nil {
		panic(fmt.Sprintf("failed to listen: %v", err))
	}

	server := grpc.NewServer()

	// Add the main service
	registrar(server)

	// Add health service
	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(server, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)
	if opts.ServiceName != "" {
		healthServer.SetServingStatus(opts.ServiceName, grpc_health_v1.HealthCheckResponse_SERVING)
	}

	// Add reflection if enabled
	if opts.EnableReflection {
		reflection.Register(server)
	}

	cleanup := func() {
		if config.Type == "uds" {
			_ = os.Remove(config.Address)
		}
	}

	return server, listener, cleanup
}

// RunServer runs a gRPC server until SIGINT or SIGTERM.
func RunServer(registrar ServiceRegistrar, opts ServerOptions) {
	server, listener, cleanup := CreateServer(registrar, opts)
	defer cleanup()

	config := GetTransportConfig()
	fmt.Printf("Server started: %s (%s) on %s (%s)\n",
		opts.ServiceName, opts.Domain, config.Address, config.Type)

	// Handle graceful shutdown
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	go func() {
		<-ctx.Done()
		fmt.Println("Shutting down server...")
		server.GracefulStop()
	}()

	if err := server.Serve(listener); err != nil {
		fmt.Printf("Server error: %v\n", err)
	}
}

// CleanupSocket removes a UDS socket file.
func CleanupSocket(socketPath string) {
	if socketPath != "" {
		_ = os.Remove(socketPath)
	}
}
