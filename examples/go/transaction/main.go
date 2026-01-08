// Package main provides the Transaction bounded context business logic.
// Handles purchases, discounts, and transaction lifecycle.
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

	"transaction/proto/evented"
	"transaction/proto/examples"
)

const Domain = "transaction"

var logger *zap.Logger

// TransactionState represents the current state of a transaction.
type TransactionState struct {
	CustomerID    string
	Items         []*examples.LineItem
	SubtotalCents int32
	DiscountCents int32
	DiscountType  string
	Status        string // "new", "pending", "completed", "cancelled"
}

// rebuildState reconstructs transaction state from events.
func rebuildState(eventBook *evented.EventBook) *TransactionState {
	state := &TransactionState{
		Status: "new",
	}

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.TransactionCreated{}):
			var event examples.TransactionCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.CustomerID = event.CustomerId
				state.Items = event.Items
				state.SubtotalCents = event.SubtotalCents
				state.Status = "pending"
			}

		case page.Event.MessageIs(&examples.DiscountApplied{}):
			var event examples.DiscountApplied
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.DiscountCents = event.DiscountCents
				state.DiscountType = event.DiscountType
			}

		case page.Event.MessageIs(&examples.TransactionCompleted{}):
			state.Status = "completed"

		case page.Event.MessageIs(&examples.TransactionCancelled{}):
			state.Status = "cancelled"
		}
	}

	return state
}

// handleCreateTransaction handles the CreateTransaction command.
func handleCreateTransaction(cmdBook *evented.CommandBook, cmdData []byte, state *TransactionState) (*evented.EventBook, error) {
	if state.Status != "new" {
		return nil, status.Error(codes.FailedPrecondition, "Transaction already exists")
	}

	var cmd examples.CreateTransaction
	if err := goproto.Unmarshal(cmdData, &cmd); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
	}

	if cmd.CustomerId == "" {
		return nil, status.Error(codes.InvalidArgument, "customer_id is required")
	}
	if len(cmd.Items) == 0 {
		return nil, status.Error(codes.InvalidArgument, "at least one item is required")
	}

	var subtotal int32
	for _, item := range cmd.Items {
		subtotal += item.Quantity * item.UnitPriceCents
	}

	logger.Info("creating transaction",
		zap.String("customer_id", cmd.CustomerId),
		zap.Int("item_count", len(cmd.Items)),
		zap.Int32("subtotal_cents", subtotal))

	event := &examples.TransactionCreated{
		CustomerId:    cmd.CustomerId,
		Items:         cmd.Items,
		SubtotalCents: subtotal,
		CreatedAt:     timestamppb.Now(),
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

// handleApplyDiscount handles the ApplyDiscount command.
func handleApplyDiscount(cmdBook *evented.CommandBook, cmdData []byte, state *TransactionState) (*evented.EventBook, error) {
	if state.Status != "pending" {
		return nil, status.Error(codes.FailedPrecondition, "Can only apply discount to pending transaction")
	}

	var cmd examples.ApplyDiscount
	if err := goproto.Unmarshal(cmdData, &cmd); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
	}

	var discountCents int32
	switch cmd.DiscountType {
	case "percentage":
		if cmd.Value < 0 || cmd.Value > 100 {
			return nil, status.Error(codes.InvalidArgument, "Percentage must be 0-100")
		}
		discountCents = (state.SubtotalCents * cmd.Value) / 100
	case "fixed":
		discountCents = cmd.Value
		if discountCents > state.SubtotalCents {
			discountCents = state.SubtotalCents
		}
	case "coupon":
		discountCents = 500 // $5 off
	default:
		return nil, status.Errorf(codes.InvalidArgument, "Unknown discount type: %s", cmd.DiscountType)
	}

	logger.Info("applying discount",
		zap.String("discount_type", cmd.DiscountType),
		zap.Int32("value", cmd.Value),
		zap.Int32("discount_cents", discountCents))

	event := &examples.DiscountApplied{
		DiscountType:  cmd.DiscountType,
		Value:         cmd.Value,
		DiscountCents: discountCents,
		CouponCode:    cmd.CouponCode,
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

// handleCompleteTransaction handles the CompleteTransaction command.
func handleCompleteTransaction(cmdBook *evented.CommandBook, cmdData []byte, state *TransactionState) (*evented.EventBook, error) {
	if state.Status != "pending" {
		return nil, status.Error(codes.FailedPrecondition, "Can only complete pending transaction")
	}

	var cmd examples.CompleteTransaction
	if err := goproto.Unmarshal(cmdData, &cmd); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
	}

	finalTotal := state.SubtotalCents - state.DiscountCents
	if finalTotal < 0 {
		finalTotal = 0
	}
	loyaltyPoints := finalTotal / 100 // 1 point per dollar

	logger.Info("completing transaction",
		zap.Int32("final_total_cents", finalTotal),
		zap.String("payment_method", cmd.PaymentMethod),
		zap.Int32("loyalty_points_earned", loyaltyPoints))

	event := &examples.TransactionCompleted{
		FinalTotalCents:     finalTotal,
		PaymentMethod:       cmd.PaymentMethod,
		LoyaltyPointsEarned: loyaltyPoints,
		CompletedAt:         timestamppb.Now(),
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

// handleCancelTransaction handles the CancelTransaction command.
func handleCancelTransaction(cmdBook *evented.CommandBook, cmdData []byte, state *TransactionState) (*evented.EventBook, error) {
	if state.Status != "pending" {
		return nil, status.Error(codes.FailedPrecondition, "Can only cancel pending transaction")
	}

	var cmd examples.CancelTransaction
	if err := goproto.Unmarshal(cmdData, &cmd); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
	}

	logger.Info("cancelling transaction",
		zap.String("reason", cmd.Reason))

	event := &examples.TransactionCancelled{
		Reason:      cmd.Reason,
		CancelledAt: timestamppb.Now(),
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
	case strings.HasSuffix(typeURL, "CreateTransaction"):
		return handleCreateTransaction(cmdBook, cmdPage.Command.Value, state)

	case strings.HasSuffix(typeURL, "ApplyDiscount"):
		return handleApplyDiscount(cmdBook, cmdPage.Command.Value, state)

	case strings.HasSuffix(typeURL, "CompleteTransaction"):
		return handleCompleteTransaction(cmdBook, cmdPage.Command.Value, state)

	case strings.HasSuffix(typeURL, "CancelTransaction"):
		return handleCancelTransaction(cmdBook, cmdPage.Command.Value, state)

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
		port = "50053"
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
