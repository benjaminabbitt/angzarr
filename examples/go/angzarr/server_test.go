package angzarr

import (
	"context"
	"fmt"
	"net"
	"testing"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/health/grpc_health_v1"
)

func TestRunServer_registersServicesAndHealthCheck(t *testing.T) {
	// Find a free port.
	lis, err := net.Listen("tcp", ":0")
	if err != nil {
		t.Fatalf("failed to find free port: %v", err)
	}
	port := lis.Addr().(*net.TCPAddr).Port
	lis.Close()

	registered := false
	cfg := ServerConfig{
		Domain:      "test",
		DefaultPort: fmt.Sprintf("%d", port),
	}

	errCh := make(chan error, 1)
	go func() {
		errCh <- RunServer(cfg, func(s *grpc.Server) {
			registered = true
		})
	}()

	// Wait for server to start.
	var conn *grpc.ClientConn
	for i := 0; i < 20; i++ {
		conn, err = grpc.NewClient(
			fmt.Sprintf("localhost:%d", port),
			grpc.WithTransportCredentials(insecure.NewCredentials()),
		)
		if err == nil {
			ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
			resp, herr := grpc_health_v1.NewHealthClient(conn).Check(ctx, &grpc_health_v1.HealthCheckRequest{})
			cancel()
			if herr == nil && resp.Status == grpc_health_v1.HealthCheckResponse_SERVING {
				break
			}
			conn.Close()
		}
		time.Sleep(50 * time.Millisecond)
	}

	if conn == nil {
		t.Fatal("could not connect to server")
	}
	defer conn.Close()

	if !registered {
		t.Error("register function was not called")
	}

	// Verify health check responds.
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	resp, err := grpc_health_v1.NewHealthClient(conn).Check(ctx, &grpc_health_v1.HealthCheckRequest{})
	if err != nil {
		t.Fatalf("health check failed: %v", err)
	}
	if resp.Status != grpc_health_v1.HealthCheckResponse_SERVING {
		t.Errorf("expected SERVING, got %v", resp.Status)
	}
}
