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

func guardReleaseFunds(state PlayerState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Player does not exist")
	}
	return nil
}

func validateReleaseFunds(cmd *examples.ReleaseFunds, state PlayerState) (int64, error) {
	if cmd.TableRoot == nil || len(cmd.TableRoot) == 0 {
		return 0, angzarr.NewCommandRejectedError("table_root is required")
	}

	tableKey := hex.EncodeToString(cmd.TableRoot)
	reserved, ok := state.TableReservations[tableKey]
	if !ok {
		return 0, angzarr.NewCommandRejectedError("No funds reserved for this table")
	}

	return reserved, nil
}

func computeFundsReleased(cmd *examples.ReleaseFunds, state PlayerState, reserved int64) *examples.FundsReleased {
	newReserved := state.ReservedFunds - reserved
	newAvailable := state.Bankroll - newReserved
	return &examples.FundsReleased{
		Amount:              &examples.Currency{Amount: reserved, CurrencyCode: "CHIPS"},
		TableRoot:           cmd.TableRoot,
		NewAvailableBalance: &examples.Currency{Amount: newAvailable, CurrencyCode: "CHIPS"},
		NewReservedBalance:  &examples.Currency{Amount: newReserved, CurrencyCode: "CHIPS"},
		ReleasedAt:          timestamppb.New(time.Now()),
	}
}

// HandleReleaseFunds handles the ReleaseFunds command (release reserved funds back to bankroll).
func HandleReleaseFunds(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.ReleaseFunds
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardReleaseFunds(state); err != nil {
		return nil, err
	}
	reserved, err := validateReleaseFunds(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computeFundsReleased(&cmd, state, reserved)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
