// Package main provides the Receipt Projector - Go Implementation.
// Generates human-readable receipts when transactions complete.
package main

import (
	"context"
	"encoding/hex"
	"fmt"
	"net"
	"os"
	"strings"

	"go.uber.org/zap"
	"google.golang.org/grpc"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/emptypb"

	"projector-receipt/proto/evented"
	"projector-receipt/proto/examples"
)

const ProjectorName = "receipt"

var logger *zap.Logger

// TransactionState holds the rebuilt state from events.
type TransactionState struct {
	CustomerID          string
	Items               []*examples.LineItem
	SubtotalCents       int32
	DiscountCents       int32
	DiscountType        string
	FinalTotalCents     int32
	PaymentMethod       string
	LoyaltyPointsEarned int32
	Completed           bool
}

// server implements the Projector gRPC service.
type server struct {
	evented.UnimplementedProjectorServer
}

// Handle processes events asynchronously (fire-and-forget).
func (s *server) Handle(ctx context.Context, req *evented.EventBook) (*emptypb.Empty, error) {
	_, _ = s.HandleSync(ctx, req)
	return &emptypb.Empty{}, nil
}

// HandleSync processes events and returns projection synchronously.
func (s *server) HandleSync(ctx context.Context, req *evented.EventBook) (*evented.Projection, error) {
	return project(req), nil
}

// project rebuilds transaction state and generates a receipt if completed.
func project(eventBook *evented.EventBook) *evented.Projection {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

	// Rebuild transaction state from all events
	state := &TransactionState{}

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
			}

		case page.Event.MessageIs(&examples.DiscountApplied{}):
			var event examples.DiscountApplied
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.DiscountType = event.DiscountType
				state.DiscountCents = event.DiscountCents
			}

		case page.Event.MessageIs(&examples.TransactionCompleted{}):
			var event examples.TransactionCompleted
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.FinalTotalCents = event.FinalTotalCents
				state.PaymentMethod = event.PaymentMethod
				state.LoyaltyPointsEarned = event.LoyaltyPointsEarned
				state.Completed = true
			}
		}
	}

	// Only generate receipt if transaction completed
	if !state.Completed {
		return nil
	}

	transactionID := ""
	if eventBook.Cover != nil && eventBook.Cover.Root != nil {
		transactionID = hex.EncodeToString(eventBook.Cover.Root.Value)
	}

	shortID := transactionID
	if len(shortID) > 16 {
		shortID = shortID[:16]
	}

	// Generate formatted receipt text
	receiptText := formatReceipt(transactionID, state)

	logger.Info("generated receipt",
		zap.String("transaction_id", shortID),
		zap.Int32("total_cents", state.FinalTotalCents),
		zap.String("payment_method", state.PaymentMethod))

	// Create Receipt message
	receipt := &examples.Receipt{
		TransactionId:       transactionID,
		CustomerId:          state.CustomerID,
		Items:               state.Items,
		SubtotalCents:       state.SubtotalCents,
		DiscountCents:       state.DiscountCents,
		FinalTotalCents:     state.FinalTotalCents,
		PaymentMethod:       state.PaymentMethod,
		LoyaltyPointsEarned: state.LoyaltyPointsEarned,
		FormattedText:       receiptText,
	}

	receiptAny, err := anypb.New(receipt)
	if err != nil {
		logger.Error("failed to create Any for Receipt",
			zap.Error(err))
		return nil
	}

	// Get sequence from last page
	var sequence uint32
	if len(eventBook.Pages) > 0 {
		lastPage := eventBook.Pages[len(eventBook.Pages)-1]
		if num, ok := lastPage.Sequence.(*evented.EventPage_Num); ok {
			sequence = num.Num
		}
	}

	return &evented.Projection{
		Cover:      eventBook.Cover,
		Projector:  ProjectorName,
		Sequence:   sequence,
		Projection: receiptAny,
	}
}

func formatReceipt(transactionID string, state *TransactionState) string {
	var lines []string

	shortTxID := transactionID
	if len(shortTxID) > 16 {
		shortTxID = shortTxID[:16]
	}

	shortCustID := state.CustomerID
	if len(shortCustID) > 16 {
		shortCustID = shortCustID[:16]
	}

	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, "           RECEIPT")
	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, fmt.Sprintf("Transaction: %s...", shortTxID))
	if state.CustomerID != "" {
		lines = append(lines, fmt.Sprintf("Customer: %s...", shortCustID))
	} else {
		lines = append(lines, "Customer: N/A")
	}
	lines = append(lines, strings.Repeat("─", 40))

	// Items
	for _, item := range state.Items {
		lineTotal := item.Quantity * item.UnitPriceCents
		lines = append(lines, fmt.Sprintf("%d x %s @ $%.2f = $%.2f",
			item.Quantity,
			item.Name,
			float64(item.UnitPriceCents)/100,
			float64(lineTotal)/100))
	}

	lines = append(lines, strings.Repeat("─", 40))
	lines = append(lines, fmt.Sprintf("Subtotal:              $%.2f", float64(state.SubtotalCents)/100))

	if state.DiscountCents > 0 {
		lines = append(lines, fmt.Sprintf("Discount (%s):       -$%.2f",
			state.DiscountType,
			float64(state.DiscountCents)/100))
	}

	lines = append(lines, strings.Repeat("─", 40))
	lines = append(lines, fmt.Sprintf("TOTAL:                 $%.2f", float64(state.FinalTotalCents)/100))
	lines = append(lines, fmt.Sprintf("Payment: %s", state.PaymentMethod))
	lines = append(lines, strings.Repeat("─", 40))
	lines = append(lines, fmt.Sprintf("Loyalty Points Earned: %d", state.LoyaltyPointsEarned))
	lines = append(lines, strings.Repeat("═", 40))
	lines = append(lines, "     Thank you for your purchase!")
	lines = append(lines, strings.Repeat("═", 40))

	return strings.Join(lines, "\n")
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
		port = "50055"
	}

	lis, err := net.Listen("tcp", fmt.Sprintf(":%s", port))
	if err != nil {
		logger.Fatal("failed to listen",
			zap.String("port", port),
			zap.Error(err))
	}

	s := grpc.NewServer()
	evented.RegisterProjectorServer(s, &server{})

	logger.Info("projector server started",
		zap.String("name", ProjectorName),
		zap.String("port", port),
		zap.String("listens_to", "transaction domain"))

	if err := s.Serve(lis); err != nil {
		logger.Fatal("failed to serve", zap.Error(err))
	}
}
