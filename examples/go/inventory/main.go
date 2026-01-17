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

	"inventory/logic"
	"inventory/proto/angzarr"
	"inventory/proto/examples"
)

const Domain = "inventory"

var logger *zap.Logger

type server struct {
	angzarr.UnimplementedBusinessLogicServer
	logic logic.InventoryLogic
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

	switch {
	case strings.HasSuffix(typeURL, "InitializeStock"):
		var cmd examples.InitializeStock
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("initializing stock", zap.String("product_id", cmd.ProductId), zap.Int32("quantity", cmd.Quantity))
		event, err := s.logic.HandleInitializeStock(state, cmd.ProductId, cmd.Quantity, cmd.LowStockThreshold)
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

	case strings.HasSuffix(typeURL, "ReceiveStock"):
		var cmd examples.ReceiveStock
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("receiving stock", zap.Int32("quantity", cmd.Quantity), zap.String("reference", cmd.Reference))
		event, err := s.logic.HandleReceiveStock(state, cmd.Quantity, cmd.Reference)
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

	case strings.HasSuffix(typeURL, "ReserveStock"):
		var cmd examples.ReserveStock
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("reserving stock", zap.Int32("quantity", cmd.Quantity), zap.String("order_id", cmd.OrderId))
		events, err := s.logic.HandleReserveStock(state, cmd.Quantity, cmd.OrderId)
		if err != nil {
			return nil, mapError(err)
		}
		eventBook, err := logic.PackEvents(cmdBook.Cover, events, seq)
		if err != nil {
			return nil, status.Errorf(codes.Internal, "failed to pack events: %v", err)
		}
		return &angzarr.BusinessResponse{
			Result: &angzarr.BusinessResponse_Events{Events: eventBook},
		}, nil

	case strings.HasSuffix(typeURL, "ReleaseReservation"):
		var cmd examples.ReleaseReservation
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("releasing reservation", zap.String("order_id", cmd.OrderId))
		event, err := s.logic.HandleReleaseReservation(state, cmd.OrderId)
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

	case strings.HasSuffix(typeURL, "CommitReservation"):
		var cmd examples.CommitReservation
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("committing reservation", zap.String("order_id", cmd.OrderId))
		event, err := s.logic.HandleCommitReservation(state, cmd.OrderId)
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

	default:
		return nil, status.Errorf(codes.InvalidArgument, "Unknown command type: %s", typeURL)
	}
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
		port = "50204"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen", zap.String("port", port), zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterBusinessLogicServer(s, &server{logic: logic.NewInventoryLogic()})

	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(s, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	logger.Info("business logic server started", zap.String("domain", Domain), zap.String("port", port))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
