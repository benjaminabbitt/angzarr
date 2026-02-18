package handlers

import (
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// docs:start:deposit_guard
func guardDepositFunds(state PlayerState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Player does not exist")
	}
	return nil
}
// docs:end:deposit_guard

// docs:start:deposit_validate
func validateDepositFunds(cmd *examples.DepositFunds) (int64, error) {
	amount := int64(0)
	if cmd.Amount != nil {
		amount = cmd.Amount.Amount
	}
	if amount <= 0 {
		return 0, angzarr.NewCommandRejectedError("amount must be positive")
	}
	return amount, nil
}
// docs:end:deposit_validate

// docs:start:deposit_compute
func computeFundsDeposited(cmd *examples.DepositFunds, state PlayerState, amount int64) *examples.FundsDeposited {
	newBalance := state.Bankroll + amount
	return &examples.FundsDeposited{
		Amount:      cmd.Amount,
		NewBalance:  &examples.Currency{Amount: newBalance, CurrencyCode: "CHIPS"},
		DepositedAt: timestamppb.New(time.Now()),
	}
}
// docs:end:deposit_compute

// HandleDepositFunds handles the DepositFunds command.
func HandleDepositFunds(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.DepositFunds
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardDepositFunds(state); err != nil {
		return nil, err
	}
	amount, err := validateDepositFunds(&cmd)
	if err != nil {
		return nil, err
	}

	event := computeFundsDeposited(&cmd, state, amount)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
