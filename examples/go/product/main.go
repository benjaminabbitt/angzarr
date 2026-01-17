// Package main provides the Product bounded context gRPC server.
package main

import (
	"context"
	"fmt"
	"net"
	"os"
	"strings"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/grpc/status"
	goproto "google.golang.org/protobuf/proto"

	"product/logic"
	"product/proto/angzarr"
	"product/proto/examples"
)

const Domain = "product"

var logger *zap.Logger

// server implements the BusinessLogic gRPC service.
type server struct {
	angzarr.UnimplementedBusinessLogicServer
	logic logic.ProductLogic
}

// Handle processes a contextual command and returns resulting events.
func (s *server) Handle(ctx context.Context, req *angzarr.ContextualCommand) (*angzarr.BusinessResponse, error) {
	cmdBook := req.Command
	priorEvents := req.Events

	if cmdBook == nil || len(cmdBook.Pages) == 0 {
		return nil, status.Error(codes.InvalidArgument, "CommandBook has no pages")
	}

	cmdPage := cmdBook.Pages[0]
	if cmdPage.Command == nil {
		return nil, status.Error(codes.InvalidArgument, "Command page has no command")
	}

	state := s.logic.RebuildState(priorEvents)
	seq := logic.NextSequence(priorEvents)
	typeURL := cmdPage.Command.TypeUrl

	var event goproto.Message
	var err error

	switch {
	case strings.HasSuffix(typeURL, "CreateProduct"):
		var cmd examples.CreateProduct
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("creating product",
			zap.String("sku", cmd.Sku),
			zap.String("name", cmd.Name))
		event, err = s.logic.HandleCreateProduct(state, cmd.Sku, cmd.Name, cmd.Description, cmd.PriceCents)

	case strings.HasSuffix(typeURL, "UpdateProduct"):
		var cmd examples.UpdateProduct
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("updating product",
			zap.String("name", cmd.Name))
		event, err = s.logic.HandleUpdateProduct(state, cmd.Name, cmd.Description)

	case strings.HasSuffix(typeURL, "SetPrice"):
		var cmd examples.SetPrice
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("setting price",
			zap.Int32("price_cents", cmd.PriceCents))
		event, err = s.logic.HandleSetPrice(state, cmd.PriceCents)

	case strings.HasSuffix(typeURL, "Discontinue"):
		var cmd examples.Discontinue
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("discontinuing product",
			zap.String("reason", cmd.Reason))
		event, err = s.logic.HandleDiscontinue(state, cmd.Reason)

	default:
		return nil, status.Errorf(codes.InvalidArgument, "Unknown command type: %s", typeURL)
	}

	if err != nil {
		return nil, mapError(err)
	}

	eventBook, err := logic.PackEvent(cmdBook.Cover, event, seq)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "failed to pack event: %v", err)
	}

	return &angzarr.BusinessResponse{
		Result: &angzarr.BusinessResponse_Events{Events: eventBook},
	}, nil
}

// mapError converts logic errors to gRPC status errors.
func mapError(err error) error {
	if cmdErr, ok := err.(*logic.CommandError); ok {
		switch cmdErr.Code {
		case logic.StatusInvalidArgument:
			return status.Error(codes.InvalidArgument, cmdErr.Message)
		case logic.StatusFailedPrecondition:
			return status.Error(codes.FailedPrecondition, cmdErr.Message)
		}
	}
	return status.Errorf(codes.Internal, "internal error: %v", err)
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
		port = "50201"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen",
			zap.String("port", port),
			zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterBusinessLogicServer(s, &server{
		logic: logic.NewProductLogic(),
	})

	// Register gRPC health service
	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(s, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	logger.Info("business logic server started",
		zap.String("domain", Domain),
		zap.String("port", port))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
