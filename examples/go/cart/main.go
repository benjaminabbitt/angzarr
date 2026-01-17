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

	"cart/logic"
	"cart/proto/angzarr"
	"cart/proto/examples"
)

const Domain = "cart"

var logger *zap.Logger

type server struct {
	angzarr.UnimplementedBusinessLogicServer
	logic logic.CartLogic
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

	state := s.logic.RebuildState(priorEvents)
	seq := logic.NextSequence(priorEvents)
	typeURL := cmdPage.Command.TypeUrl

	var event goproto.Message
	var err error

	switch {
	case strings.HasSuffix(typeURL, "CreateCart"):
		var cmd examples.CreateCart
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("creating cart", zap.String("customer_id", cmd.CustomerId))
		event, err = s.logic.HandleCreateCart(state, cmd.CustomerId)

	case strings.HasSuffix(typeURL, "AddItem"):
		var cmd examples.AddItem
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("adding item", zap.String("product_id", cmd.ProductId), zap.Int32("quantity", cmd.Quantity))
		event, err = s.logic.HandleAddItem(state, cmd.ProductId, cmd.Name, cmd.Quantity, cmd.UnitPriceCents)

	case strings.HasSuffix(typeURL, "UpdateQuantity"):
		var cmd examples.UpdateQuantity
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("updating quantity", zap.String("product_id", cmd.ProductId), zap.Int32("new_quantity", cmd.NewQuantity))
		event, err = s.logic.HandleUpdateQuantity(state, cmd.ProductId, cmd.NewQuantity)

	case strings.HasSuffix(typeURL, "RemoveItem"):
		var cmd examples.RemoveItem
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("removing item", zap.String("product_id", cmd.ProductId))
		event, err = s.logic.HandleRemoveItem(state, cmd.ProductId)

	case strings.HasSuffix(typeURL, "ApplyCoupon"):
		var cmd examples.ApplyCoupon
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("applying coupon", zap.String("code", cmd.Code))
		event, err = s.logic.HandleApplyCoupon(state, cmd.Code, cmd.CouponType, cmd.Value)

	case strings.HasSuffix(typeURL, "ClearCart"):
		if err := goproto.Unmarshal(cmdPage.Command.Value, &examples.ClearCart{}); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("clearing cart")
		event, err = s.logic.HandleClearCart(state)

	case strings.HasSuffix(typeURL, "Checkout"):
		if err := goproto.Unmarshal(cmdPage.Command.Value, &examples.Checkout{}); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("checking out")
		event, err = s.logic.HandleCheckout(state)

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
		port = "50202"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen", zap.String("port", port), zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterBusinessLogicServer(s, &server{logic: logic.NewCartLogic()})

	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(s, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	logger.Info("business logic server started", zap.String("domain", Domain), zap.String("port", port))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
