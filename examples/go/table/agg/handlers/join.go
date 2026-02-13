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

// HandleJoinTable handles the JoinTable command.
func HandleJoinTable(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state TableState,
	seq uint32,
) (*pb.EventBook, error) {
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}

	var cmd examples.JoinTable
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if len(cmd.PlayerRoot) == 0 {
		return nil, angzarr.NewCommandRejectedError("player_root is required")
	}

	// Check if player already seated
	if state.FindSeatByPlayer(cmd.PlayerRoot) >= 0 {
		return nil, angzarr.NewCommandRejectedError("Player already seated")
	}

	// Validate buy-in
	if cmd.BuyInAmount < state.MinBuyIn {
		return nil, angzarr.NewCommandRejectedError(fmt.Sprintf("Buy-in must be at least %d", state.MinBuyIn))
	}
	if cmd.BuyInAmount > state.MaxBuyIn {
		return nil, angzarr.NewCommandRejectedError("Buy-in above maximum")
	}

	// Find seat
	var seatPosition int32
	if cmd.PreferredSeat >= 0 && cmd.PreferredSeat < state.MaxPlayers {
		if _, taken := state.Seats[cmd.PreferredSeat]; taken {
			return nil, angzarr.NewCommandRejectedError("Seat is occupied")
		}
		seatPosition = cmd.PreferredSeat
	} else {
		seatPosition = state.NextAvailableSeat()
		if seatPosition < 0 {
			return nil, angzarr.NewCommandRejectedError("Table is full")
		}
	}

	event := &examples.PlayerJoined{
		PlayerRoot:   cmd.PlayerRoot,
		SeatPosition: seatPosition,
		BuyInAmount:  cmd.BuyInAmount,
		Stack:        cmd.BuyInAmount,
		JoinedAt:     timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
