package handlers

import (
	"fmt"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

func guardJoinTable(state TableState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Table does not exist")
	}
	return nil
}

func validateJoinTable(cmd *examples.JoinTable, state TableState) (int32, error) {
	if len(cmd.PlayerRoot) == 0 {
		return 0, angzarr.NewCommandRejectedError("player_root is required")
	}

	if state.FindSeatByPlayer(cmd.PlayerRoot) >= 0 {
		return 0, angzarr.NewCommandRejectedError("Player already seated")
	}

	if cmd.BuyInAmount < state.MinBuyIn {
		return 0, angzarr.NewCommandRejectedError(fmt.Sprintf("Buy-in must be at least %d", state.MinBuyIn))
	}
	if cmd.BuyInAmount > state.MaxBuyIn {
		return 0, angzarr.NewCommandRejectedError("Buy-in above maximum")
	}

	var seatPosition int32
	if cmd.PreferredSeat >= 0 && cmd.PreferredSeat < state.MaxPlayers {
		if _, taken := state.Seats[cmd.PreferredSeat]; taken {
			return 0, angzarr.NewCommandRejectedError("Seat is occupied")
		}
		seatPosition = cmd.PreferredSeat
	} else {
		seatPosition = state.NextAvailableSeat()
		if seatPosition < 0 {
			return 0, angzarr.NewCommandRejectedError("Table is full")
		}
	}

	return seatPosition, nil
}

func computePlayerJoined(cmd *examples.JoinTable, seatPosition int32) *examples.PlayerJoined {
	return &examples.PlayerJoined{
		PlayerRoot:   cmd.PlayerRoot,
		SeatPosition: seatPosition,
		BuyInAmount:  cmd.BuyInAmount,
		Stack:        cmd.BuyInAmount,
		JoinedAt:     timestamppb.New(time.Now()),
	}
}

// HandleJoinTable handles the JoinTable command.
func HandleJoinTable(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state TableState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.JoinTable
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardJoinTable(state); err != nil {
		return nil, err
	}
	seatPosition, err := validateJoinTable(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computePlayerJoined(&cmd, seatPosition)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
