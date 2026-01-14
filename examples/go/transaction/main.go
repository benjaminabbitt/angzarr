// Package main provides the Transaction bounded context gRPC server.
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

	"transaction/logic"
	"transaction/proto/angzarr"
	"transaction/proto/examples"
)

const Domain = "transaction"

var logger *zap.Logger

// server implements the BusinessLogic gRPC service.
type server struct {
	angzarr.UnimplementedBusinessLogicServer
	logic logic.TransactionLogic
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

	var event interface{}
	var err error

	switch {
	case strings.HasSuffix(typeURL, "CreateTransaction"):
		var cmd examples.CreateTransaction
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("creating transaction",
			zap.String("customer_id", cmd.CustomerId),
			zap.Int("item_count", len(cmd.Items)),
		)
		event, err = s.logic.HandleCreateTransaction(state, cmd.CustomerId, cmd.Items)

	case strings.HasSuffix(typeURL, "ApplyDiscount"):
		var cmd examples.ApplyDiscount
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("applying discount",
			zap.String("discount_type", cmd.DiscountType),
			zap.Int32("value", cmd.Value),
		)
		event, err = s.logic.HandleApplyDiscount(state, cmd.DiscountType, cmd.Value, cmd.CouponCode)

	case strings.HasSuffix(typeURL, "CompleteTransaction"):
		var cmd examples.CompleteTransaction
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("completing transaction",
			zap.String("payment_method", cmd.PaymentMethod),
		)
		event, err = s.logic.HandleCompleteTransaction(state, cmd.PaymentMethod)

	case strings.HasSuffix(typeURL, "CancelTransaction"):
		var cmd examples.CancelTransaction
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("cancelling transaction",
			zap.String("reason", cmd.Reason),
		)
		event, err = s.logic.HandleCancelTransaction(state, cmd.Reason)

	default:
		return nil, status.Errorf(codes.InvalidArgument, "Unknown command type: %s", typeURL)
	}

	if err != nil {
		return nil, mapError(err)
	}

	eventBook, err := logic.PackEvent(cmdBook.Cover, event.(goproto.Message), seq)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "failed to pack event: %v", err)
	}

	return &angzarr.BusinessResponse{
		Result: &angzarr.BusinessResponse_Events{
			Events: eventBook,
		},
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
		port = "50053"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen",
			zap.String("port", port),
			zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterBusinessLogicServer(s, &server{
		logic: logic.NewTransactionLogic(),
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
