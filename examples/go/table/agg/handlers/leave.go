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

// HandleLeaveTable handles the LeaveTable command.
func HandleLeaveTable(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state TableState,
	seq uint32,
) (*pb.EventBook, error) {
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}

	var cmd examples.LeaveTable
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if len(cmd.PlayerRoot) == 0 {
		return nil, angzarr.NewCommandRejectedError("player_root is required")
	}

	seatPosition := state.FindSeatByPlayer(cmd.PlayerRoot)
	if seatPosition < 0 {
		return nil, angzarr.NewCommandRejectedError("Player not seated at table")
	}

	// Can't leave during a hand
	if state.Status == "in_hand" {
		return nil, angzarr.NewCommandRejectedError("Cannot leave during a hand")
	}

	seat := state.Seats[seatPosition]

	event := &examples.PlayerLeft{
		PlayerRoot:     cmd.PlayerRoot,
		SeatPosition:   seatPosition,
		ChipsCashedOut: seat.Stack,
		LeftAt:         timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
