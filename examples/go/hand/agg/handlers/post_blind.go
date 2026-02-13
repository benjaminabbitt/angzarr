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

// HandlePostBlind handles the PostBlind command.
func HandlePostBlind(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state HandState,
	seq uint32,
) (*pb.EventBook, error) {
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}

	var cmd examples.PostBlind
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}

	if cmd.Amount <= 0 {
		return nil, angzarr.NewCommandRejectedError("Amount must be positive")
	}

	// Calculate actual amount (might be all-in)
	actualAmount := cmd.Amount
	if actualAmount > player.Stack {
		actualAmount = player.Stack
	}

	newStack := player.Stack - actualAmount
	newPot := state.TotalPot() + actualAmount

	event := &examples.BlindPosted{
		PlayerRoot:  cmd.PlayerRoot,
		BlindType:   cmd.BlindType,
		Amount:      actualAmount,
		PlayerStack: newStack,
		PotTotal:    newPot,
		PostedAt:    timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
