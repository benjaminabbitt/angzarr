// Package main provides the Customer bounded context gRPC server.
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

	"customer/logic"
	"customer/proto/angzarr"
	"customer/proto/examples"
)

const Domain = "customer"

var logger *zap.Logger

// server implements the BusinessLogic gRPC service.
type server struct {
	angzarr.UnimplementedBusinessLogicServer
	logic logic.CustomerLogic
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
	case strings.HasSuffix(typeURL, "CreateCustomer"):
		var cmd examples.CreateCustomer
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("creating customer",
			zap.String("name", cmd.Name),
			zap.String("email", cmd.Email))
		event, err = s.logic.HandleCreateCustomer(state, cmd.Name, cmd.Email)

	case strings.HasSuffix(typeURL, "AddLoyaltyPoints"):
		var cmd examples.AddLoyaltyPoints
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("adding loyalty points",
			zap.Int32("points", cmd.Points),
			zap.String("reason", cmd.Reason))
		event, err = s.logic.HandleAddLoyaltyPoints(state, cmd.Points, cmd.Reason)

	case strings.HasSuffix(typeURL, "RedeemLoyaltyPoints"):
		var cmd examples.RedeemLoyaltyPoints
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("redeeming loyalty points",
			zap.Int32("points", cmd.Points),
			zap.String("redemption_type", cmd.RedemptionType))
		event, err = s.logic.HandleRedeemLoyaltyPoints(state, cmd.Points, cmd.RedemptionType)

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
		port = "50052"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen",
			zap.String("port", port),
			zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterBusinessLogicServer(s, &server{
		logic: logic.NewCustomerLogic(),
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
