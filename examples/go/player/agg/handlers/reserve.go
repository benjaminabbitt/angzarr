package handlers

import (
	"encoding/hex"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleReserveFunds handles the ReserveFunds command (reserve funds for table buy-in).
func HandleReserveFunds(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Player does not exist")
	}

	var cmd examples.ReserveFunds
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	amount := int64(0)
	if cmd.Amount != nil {
		amount = cmd.Amount.Amount
	}
	if amount <= 0 {
		return nil, angzarr.NewCommandRejectedError("amount must be positive")
	}

	if amount > state.AvailableBalance() {
		return nil, angzarr.NewCommandRejectedError("Insufficient funds")
	}

	// Check if table already has a reservation
	tableKey := hex.EncodeToString(cmd.TableRoot)
	if _, exists := state.TableReservations[tableKey]; exists {
		return nil, angzarr.NewCommandRejectedError("Funds already reserved for this table")
	}

	newReserved := state.ReservedFunds + amount
	newAvailable := state.Bankroll - newReserved

	event := &examples.FundsReserved{
		Amount:              cmd.Amount,
		TableRoot:           cmd.TableRoot,
		NewAvailableBalance: &examples.Currency{Amount: newAvailable, CurrencyCode: "CHIPS"},
		NewReservedBalance:  &examples.Currency{Amount: newReserved, CurrencyCode: "CHIPS"},
		ReservedAt:          timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
