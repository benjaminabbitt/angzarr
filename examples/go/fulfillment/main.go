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

	"fulfillment/logic"
	"fulfillment/proto/angzarr"
	"fulfillment/proto/examples"
)

const Domain = "fulfillment"

var logger *zap.Logger

type server struct {
	angzarr.UnimplementedAggregateServer
	logic logic.FulfillmentLogic
}

func (s *server) Handle(ctx context.Context, req *angzarr.ContextualCommand) (*angzarr.BusinessResponse, error) {
	cmdBook := req.Command
	priorEvents := req.Events

	if cmdBook == nil || len(cmdBook.Pages) == 0 {
		return nil, status.Error(codes.InvalidArgument, logic.ErrMsgNoCommandPages)
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
	case strings.HasSuffix(typeURL, "CreateShipment"):
		var cmd examples.CreateShipment
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("creating shipment", zap.String("order_id", cmd.OrderId))
		event, err = s.logic.HandleCreateShipment(state, cmd.OrderId)

	case strings.HasSuffix(typeURL, "MarkPicked"):
		var cmd examples.MarkPicked
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("marking picked", zap.String("picker_id", cmd.PickerId))
		event, err = s.logic.HandleMarkPicked(state, cmd.PickerId)

	case strings.HasSuffix(typeURL, "MarkPacked"):
		var cmd examples.MarkPacked
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("marking packed", zap.String("packer_id", cmd.PackerId))
		event, err = s.logic.HandleMarkPacked(state, cmd.PackerId)

	case strings.HasSuffix(typeURL, "Ship"):
		var cmd examples.Ship
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("shipping", zap.String("carrier", cmd.Carrier), zap.String("tracking_number", cmd.TrackingNumber))
		event, err = s.logic.HandleShip(state, cmd.Carrier, cmd.TrackingNumber)

	case strings.HasSuffix(typeURL, "RecordDelivery"):
		var cmd examples.RecordDelivery
		if err := goproto.Unmarshal(cmdPage.Command.Value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("recording delivery", zap.String("signature", cmd.Signature))
		event, err = s.logic.HandleRecordDelivery(state, cmd.Signature)

	default:
		return nil, status.Errorf(codes.InvalidArgument, "%s: %s", logic.ErrMsgUnknownCommand, typeURL)
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
		port = "50205"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen", zap.String("port", port), zap.Error(err))
	}

	s := grpc.NewServer()
	angzarr.RegisterAggregateServer(s, &server{logic: logic.NewFulfillmentLogic()})

	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(s, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	logger.Info("business logic server started", zap.String("domain", Domain), zap.String("port", port))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
