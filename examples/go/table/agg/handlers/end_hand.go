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

func guardEndHand(state TableState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Table does not exist")
	}
	if state.Status != "in_hand" {
		return angzarr.NewCommandRejectedError("No hand in progress")
	}
	return nil
}

func validateEndHand(cmd *examples.EndHand, state TableState) error {
	if hex.EncodeToString(cmd.HandRoot) != hex.EncodeToString(state.CurrentHandRoot) {
		return angzarr.NewCommandRejectedError("Hand root mismatch")
	}
	return nil
}

func computeHandEnded(cmd *examples.EndHand) *examples.HandEnded {
	stackChanges := make(map[string]int64)
	for _, result := range cmd.Results {
		winnerHex := hex.EncodeToString(result.WinnerRoot)
		stackChanges[winnerHex] += result.Amount
	}

	return &examples.HandEnded{
		HandRoot:     cmd.HandRoot,
		Results:      cmd.Results,
		StackChanges: stackChanges,
		EndedAt:      timestamppb.New(time.Now()),
	}
}

// HandleEndHand handles the EndHand command.
func HandleEndHand(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state TableState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.EndHand
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardEndHand(state); err != nil {
		return nil, err
	}
	if err := validateEndHand(&cmd, state); err != nil {
		return nil, err
	}

	event := computeHandEnded(&cmd)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
