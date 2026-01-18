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

	"order/logic"
	"order/proto/angzarr"
	"order/proto/examples"
)

const Domain = "order"

var logger *zap.Logger

type server struct {
	angzarr.UnimplementedAggregateServer
	logic logic.OrderLogic
}

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

	state := logic.RebuildState(priorEvents)
	seq := logic.NextSequence(priorEvents)
	typeURL := cmdPage.Command.TypeUrl

	var event goproto.Message
	var err error

	switch {
	case strings.HasSuffix(typeURL, "CreateOrder"):
		var cmd examples.CreateOrder
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("creating order", zap.String("customer_id", cmd.CustomerId), zap.Int("item_count", len(cmd.Items)))
		event, err = s.logic.HandleCreateOrder(state, cmd.CustomerId, cmd.Items)

	case strings.HasSuffix(typeURL, "ApplyLoyaltyDiscount"):
		var cmd examples.ApplyLoyaltyDiscount
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("applying loyalty discount", zap.Int32("points", cmd.Points), zap.Int32("discount_cents", cmd.DiscountCents))
		event, err = s.logic.HandleApplyLoyaltyDiscount(state, cmd.Points, cmd.DiscountCents)

	case strings.HasSuffix(typeURL, "SubmitPayment"):
		var cmd examples.SubmitPayment
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("submitting payment", zap.String("method", cmd.PaymentMethod), zap.Int32("amount_cents", cmd.AmountCents))
		event, err = s.logic.HandleSubmitPayment(state, cmd.PaymentMethod, cmd.AmountCents)

	case strings.HasSuffix(typeURL, "ConfirmPayment"):
		var cmd examples.ConfirmPayment
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("confirming payment", zap.String("reference", cmd.PaymentReference))
		event, err = s.logic.HandleConfirmPayment(state, cmd.PaymentReference)

	case strings.HasSuffix(typeURL, "CancelOrder"):
		var cmd examples.CancelOrder
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("cancelling order", zap.String("reason", cmd.Reason))
		event, err = s.logic.HandleCancelOrder(state, cmd.Reason)

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
		port = "50203"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen", zap.String("port", port), zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterAggregateServer(s, &server{logic: logic.NewOrderLogic()})

	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(s, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	logger.Info("business logic server started", zap.String("domain", Domain), zap.String("port", port))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
