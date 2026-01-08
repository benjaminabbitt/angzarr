// Package main provides the Customer bounded context business logic.
// Handles customer lifecycle and loyalty points management.
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
	"google.golang.org/grpc/status"
	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"

	"customer/proto/evented"
	"customer/proto/examples"
)

const Domain = "customer"

var logger *zap.Logger

// CustomerState represents the current state of a customer.
type CustomerState struct {
	Name           string
	Email          string
	LoyaltyPoints  int32
	LifetimePoints int32
}

// rebuildState reconstructs customer state from events.
func rebuildState(eventBook *evented.EventBook) *CustomerState {
	state := &CustomerState{}

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	// Start from snapshot if present
	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		var snapState examples.CustomerState
		if err := eventBook.Snapshot.State.UnmarshalTo(&snapState); err == nil {
			state.Name = snapState.Name
			state.Email = snapState.Email
			state.LoyaltyPoints = snapState.LoyaltyPoints
			state.LifetimePoints = snapState.LifetimePoints
		}
	}

	// Apply events
	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.CustomerCreated{}):
			var event examples.CustomerCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.Name = event.Name
				state.Email = event.Email
			}

		case page.Event.MessageIs(&examples.LoyaltyPointsAdded{}):
			var event examples.LoyaltyPointsAdded
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.LoyaltyPoints = event.NewBalance
				state.LifetimePoints += event.Points
			}

		case page.Event.MessageIs(&examples.LoyaltyPointsRedeemed{}):
			var event examples.LoyaltyPointsRedeemed
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.LoyaltyPoints = event.NewBalance
			}
		}
	}

	return state
}

// handleCreateCustomer handles the CreateCustomer command.
func handleCreateCustomer(cmdBook *evented.CommandBook, cmdData []byte, state *CustomerState) (*evented.EventBook, error) {
	if state.Name != "" {
		return nil, status.Error(codes.FailedPrecondition, "Customer already exists")
	}

	var cmd examples.CreateCustomer
	if err := goproto.Unmarshal(cmdData, &cmd); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
	}

	if cmd.Name == "" {
		return nil, status.Error(codes.InvalidArgument, "Customer name is required")
	}
	if cmd.Email == "" {
		return nil, status.Error(codes.InvalidArgument, "Customer email is required")
	}

	logger.Info("creating customer",
		zap.String("name", cmd.Name),
		zap.String("email", cmd.Email))

	event := &examples.CustomerCreated{
		Name:      cmd.Name,
		Email:     cmd.Email,
		CreatedAt: timestamppb.Now(),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "failed to create Any: %v", err)
	}

	return &evented.EventBook{
		Cover: cmdBook.Cover,
		Pages: []*evented.EventPage{
			{
				Sequence:  &evented.EventPage_Num{Num: 0},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

// handleAddLoyaltyPoints handles the AddLoyaltyPoints command.
func handleAddLoyaltyPoints(cmdBook *evented.CommandBook, cmdData []byte, state *CustomerState) (*evented.EventBook, error) {
	if state.Name == "" {
		return nil, status.Error(codes.FailedPrecondition, "Customer does not exist")
	}

	var cmd examples.AddLoyaltyPoints
	if err := goproto.Unmarshal(cmdData, &cmd); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
	}

	if cmd.Points <= 0 {
		return nil, status.Error(codes.InvalidArgument, "Points must be positive")
	}

	newBalance := state.LoyaltyPoints + cmd.Points

	logger.Info("adding loyalty points",
		zap.Int32("points", cmd.Points),
		zap.Int32("new_balance", newBalance),
		zap.String("reason", cmd.Reason))

	event := &examples.LoyaltyPointsAdded{
		Points:     cmd.Points,
		NewBalance: newBalance,
		Reason:     cmd.Reason,
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "failed to create Any: %v", err)
	}

	return &evented.EventBook{
		Cover: cmdBook.Cover,
		Pages: []*evented.EventPage{
			{
				Sequence:  &evented.EventPage_Num{Num: 0},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

// handleRedeemLoyaltyPoints handles the RedeemLoyaltyPoints command.
func handleRedeemLoyaltyPoints(cmdBook *evented.CommandBook, cmdData []byte, state *CustomerState) (*evented.EventBook, error) {
	if state.Name == "" {
		return nil, status.Error(codes.FailedPrecondition, "Customer does not exist")
	}

	var cmd examples.RedeemLoyaltyPoints
	if err := goproto.Unmarshal(cmdData, &cmd); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
	}

	if cmd.Points <= 0 {
		return nil, status.Error(codes.InvalidArgument, "Points must be positive")
	}
	if cmd.Points > state.LoyaltyPoints {
		return nil, status.Errorf(codes.FailedPrecondition,
			"Insufficient points: have %d, need %d", state.LoyaltyPoints, cmd.Points)
	}

	newBalance := state.LoyaltyPoints - cmd.Points

	logger.Info("redeeming loyalty points",
		zap.Int32("points", cmd.Points),
		zap.Int32("new_balance", newBalance),
		zap.String("redemption_type", cmd.RedemptionType))

	event := &examples.LoyaltyPointsRedeemed{
		Points:         cmd.Points,
		NewBalance:     newBalance,
		RedemptionType: cmd.RedemptionType,
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "failed to create Any: %v", err)
	}

	return &evented.EventBook{
		Cover: cmdBook.Cover,
		Pages: []*evented.EventPage{
			{
				Sequence:  &evented.EventPage_Num{Num: 0},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

// server implements the BusinessLogic gRPC service.
type server struct {
	evented.UnimplementedBusinessLogicServer
}

// Handle processes a contextual command and returns resulting events.
func (s *server) Handle(ctx context.Context, req *evented.ContextualCommand) (*evented.EventBook, error) {
	cmdBook := req.Command
	priorEvents := req.Events

	if cmdBook == nil || len(cmdBook.Pages) == 0 {
		return nil, status.Error(codes.InvalidArgument, "CommandBook has no pages")
	}

	cmdPage := cmdBook.Pages[0]
	if cmdPage.Command == nil {
		return nil, status.Error(codes.InvalidArgument, "Command page has no command")
	}

	state := rebuildState(priorEvents)
	typeURL := cmdPage.Command.TypeUrl

	switch {
	case strings.HasSuffix(typeURL, "CreateCustomer"):
		return handleCreateCustomer(cmdBook, cmdPage.Command.Value, state)

	case strings.HasSuffix(typeURL, "AddLoyaltyPoints"):
		return handleAddLoyaltyPoints(cmdBook, cmdPage.Command.Value, state)

	case strings.HasSuffix(typeURL, "RedeemLoyaltyPoints"):
		return handleRedeemLoyaltyPoints(cmdBook, cmdPage.Command.Value, state)

	default:
		return nil, status.Errorf(codes.InvalidArgument, "Unknown command type: %s", typeURL)
	}
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
	evented.RegisterBusinessLogicServer(s, &server{})

	logger.Info("business logic server started",
		zap.String("domain", Domain),
		zap.String("port", port))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
