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

// HandleEndHand handles the EndHand command.
func HandleEndHand(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state TableState,
	seq uint32,
) (*pb.EventBook, error) {
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}

	var cmd examples.EndHand
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	// Verify we're in a hand
	if state.Status != "in_hand" {
		return nil, angzarr.NewCommandRejectedError("No hand in progress")
	}

	// Verify hand root matches
	if hex.EncodeToString(cmd.HandRoot) != hex.EncodeToString(state.CurrentHandRoot) {
		return nil, angzarr.NewCommandRejectedError("Hand root mismatch")
	}

	// Calculate stack changes from pot results
	stackChanges := make(map[string]int64)
	for _, result := range cmd.Results {
		winnerHex := hex.EncodeToString(result.WinnerRoot)
		stackChanges[winnerHex] += result.Amount
	}

	event := &examples.HandEnded{
		HandRoot:     cmd.HandRoot,
		Results:      cmd.Results,
		StackChanges: stackChanges,
		EndedAt:      timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
