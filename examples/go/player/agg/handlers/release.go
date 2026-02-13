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

// HandleReleaseFunds handles the ReleaseFunds command (release reserved funds back to bankroll).
func HandleReleaseFunds(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state PlayerState,
	seq uint32,
) (*pb.EventBook, error) {
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Player does not exist")
	}

	var cmd examples.ReleaseFunds
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if cmd.TableRoot == nil || len(cmd.TableRoot) == 0 {
		return nil, angzarr.NewCommandRejectedError("table_root is required")
	}

	tableKey := hex.EncodeToString(cmd.TableRoot)
	reserved, ok := state.TableReservations[tableKey]
	if !ok {
		return nil, angzarr.NewCommandRejectedError("No funds reserved for this table")
	}

	newReserved := state.ReservedFunds - reserved
	newAvailable := state.Bankroll - newReserved

	event := &examples.FundsReleased{
		Amount:              &examples.Currency{Amount: reserved, CurrencyCode: "CHIPS"},
		TableRoot:           cmd.TableRoot,
		NewAvailableBalance: &examples.Currency{Amount: newAvailable, CurrencyCode: "CHIPS"},
		NewReservedBalance:  &examples.Currency{Amount: newReserved, CurrencyCode: "CHIPS"},
		ReleasedAt:          timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
