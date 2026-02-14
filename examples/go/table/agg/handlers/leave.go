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

func guardLeaveTable(state TableState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Table does not exist")
	}
	if state.Status == "in_hand" {
		return angzarr.NewCommandRejectedError("Cannot leave during a hand")
	}
	return nil
}

func validateLeaveTable(cmd *examples.LeaveTable, state TableState) (*SeatState, int32, error) {
	if len(cmd.PlayerRoot) == 0 {
		return nil, 0, angzarr.NewCommandRejectedError("player_root is required")
	}

	seatPosition := state.FindSeatByPlayer(cmd.PlayerRoot)
	if seatPosition < 0 {
		return nil, 0, angzarr.NewCommandRejectedError("Player not seated at table")
	}

	seat := state.Seats[seatPosition]
	return seat, seatPosition, nil
}

func computePlayerLeft(cmd *examples.LeaveTable, seat *SeatState, seatPosition int32) *examples.PlayerLeft {
	return &examples.PlayerLeft{
		PlayerRoot:     cmd.PlayerRoot,
		SeatPosition:   seatPosition,
		ChipsCashedOut: seat.Stack,
		LeftAt:         timestamppb.New(time.Now()),
	}
}

// HandleLeaveTable handles the LeaveTable command.
func HandleLeaveTable(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state TableState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.LeaveTable
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardLeaveTable(state); err != nil {
		return nil, err
	}
	seat, seatPosition, err := validateLeaveTable(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computePlayerLeft(&cmd, seat, seatPosition)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
