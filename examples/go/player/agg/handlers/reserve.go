// DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//      Update documentation when making changes to handler patterns.
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

func guardReserveFunds(state PlayerState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Player does not exist")
	}
	return nil
}

func validateReserveFunds(cmd *examples.ReserveFunds, state PlayerState) (int64, error) {
	amount := int64(0)
	if cmd.Amount != nil {
		amount = cmd.Amount.Amount
	}
	if amount <= 0 {
		return 0, angzarr.NewCommandRejectedError("amount must be positive")
	}
	if amount > state.AvailableBalance() {
		return 0, angzarr.NewCommandRejectedError("Insufficient funds")
	}

	tableKey := hex.EncodeToString(cmd.TableRoot)
	if _, exists := state.TableReservations[tableKey]; exists {
		return 0, angzarr.NewCommandRejectedError("Funds already reserved for this table")
	}

	return amount, nil
}

func computeFundsReserved(cmd *examples.ReserveFunds, state PlayerState, amount int64) *examples.FundsReserved {
	newReserved := state.ReservedFunds + amount
	newAvailable := state.Bankroll - newReserved
	return &examples.FundsReserved{
		Amount:              cmd.Amount,
		TableRoot:           cmd.TableRoot,
		NewAvailableBalance: &examples.Currency{Amount: newAvailable, CurrencyCode: "CHIPS"},
		NewReservedBalance:  &examples.Currency{Amount: newReserved, CurrencyCode: "CHIPS"},
		ReservedAt:          timestamppb.New(time.Now()),
	}
}

// HandleReserveFunds handles the ReserveFunds command (reserve funds for table buy-in).
func HandleReserveFunds(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.ReserveFunds
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardReserveFunds(state); err != nil {
		return nil, err
	}
	amount, err := validateReserveFunds(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computeFundsReserved(&cmd, state, amount)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
