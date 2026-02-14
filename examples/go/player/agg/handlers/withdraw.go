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

func guardWithdrawFunds(state PlayerState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Player does not exist")
	}
	return nil
}

func validateWithdrawFunds(cmd *examples.WithdrawFunds, state PlayerState) (int64, error) {
	amount := int64(0)
	if cmd.Amount != nil {
		amount = cmd.Amount.Amount
	}
	if amount <= 0 {
		return 0, angzarr.NewCommandRejectedError("amount must be positive")
	}
	if amount > state.AvailableBalance() {
		return 0, angzarr.NewCommandRejectedError("insufficient available balance")
	}
	return amount, nil
}

func computeFundsWithdrawn(cmd *examples.WithdrawFunds, state PlayerState, amount int64) *examples.FundsWithdrawn {
	newBalance := state.Bankroll - amount
	return &examples.FundsWithdrawn{
		Amount:      cmd.Amount,
		NewBalance:  &examples.Currency{Amount: newBalance, CurrencyCode: "CHIPS"},
		WithdrawnAt: timestamppb.New(time.Now()),
	}
}

// HandleWithdrawFunds handles the WithdrawFunds command.
func HandleWithdrawFunds(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.WithdrawFunds
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardWithdrawFunds(state); err != nil {
		return nil, err
	}
	amount, err := validateWithdrawFunds(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computeFundsWithdrawn(&cmd, state, amount)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
