package main

import (
	"context"
	"embed"
	"flag"
	"fmt"
	"io/fs"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/grpc-ecosystem/grpc-gateway/v2/runtime"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	gw "github.com/angzarr-io/angzarr/gateway/gen/angzarr"
)

//go:embed api/*
var apiFS embed.FS

var (
	grpcTarget = flag.String("grpc-target", "", "gRPC server endpoint (default: GRPC_TARGET env or localhost:1310)")
	httpPort   = flag.Int("http-port", 8080, "HTTP server port")
)

func main() {
	flag.Parse()

	target := *grpcTarget
	if target == "" {
		target = os.Getenv("GRPC_TARGET")
	}
	if target == "" {
		target = "localhost:1310"
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Set up gRPC connection
	conn, err := grpc.NewClient(target, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("Failed to connect to gRPC server at %s: %v", target, err)
	}
	defer conn.Close()

	// Create gRPC-Gateway mux
	gwMux := runtime.NewServeMux()

	// Register all service handlers
	if err := gw.RegisterAggregateCoordinatorServiceHandler(ctx, gwMux, conn); err != nil {
		log.Fatalf("Failed to register AggregateCoordinatorService handler: %v", err)
	}
	if err := gw.RegisterEventQueryServiceHandler(ctx, gwMux, conn); err != nil {
		log.Fatalf("Failed to register EventQueryService handler: %v", err)
	}
	if err := gw.RegisterEventStreamServiceHandler(ctx, gwMux, conn); err != nil {
		log.Fatalf("Failed to register EventStreamService handler: %v", err)
	}
	if err := gw.RegisterSagaCoordinatorServiceHandler(ctx, gwMux, conn); err != nil {
		log.Fatalf("Failed to register SagaCoordinatorService handler: %v", err)
	}
	if err := gw.RegisterProjectorCoordinatorServiceHandler(ctx, gwMux, conn); err != nil {
		log.Fatalf("Failed to register ProjectorCoordinatorService handler: %v", err)
	}
	if err := gw.RegisterProcessManagerCoordinatorServiceHandler(ctx, gwMux, conn); err != nil {
		log.Fatalf("Failed to register ProcessManagerCoordinatorService handler: %v", err)
	}

	// Create main HTTP mux
	mux := http.NewServeMux()

	// Health check endpoint
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("ok"))
	})

	// OpenAPI spec endpoint
	mux.HandleFunc("/openapi.json", func(w http.ResponseWriter, r *http.Request) {
		apiContent, err := fs.Sub(apiFS, "api")
		if err != nil {
			http.Error(w, "OpenAPI spec not found", http.StatusNotFound)
			return
		}
		data, err := fs.ReadFile(apiContent, "angzarr.swagger.json")
		if err != nil {
			http.Error(w, "OpenAPI spec not found", http.StatusNotFound)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.Write(data)
	})

	// Mount gRPC-Gateway at root
	mux.Handle("/", gwMux)

	addr := fmt.Sprintf(":%d", *httpPort)
	server := &http.Server{
		Addr:    addr,
		Handler: mux,
	}

	// Graceful shutdown
	go func() {
		sigChan := make(chan os.Signal, 1)
		signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
		<-sigChan

		log.Println("Shutting down HTTP server...")
		shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer shutdownCancel()

		if err := server.Shutdown(shutdownCtx); err != nil {
			log.Printf("HTTP server shutdown error: %v", err)
		}
		cancel()
	}()

	log.Printf("Starting gRPC-Gateway on %s, proxying to %s", addr, target)
	if err := server.ListenAndServe(); err != http.ErrServerClosed {
		log.Fatalf("HTTP server error: %v", err)
	}
	log.Println("Server stopped")
}
