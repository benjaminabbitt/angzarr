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

func guardPostBlind(state HandState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return angzarr.NewCommandRejectedError("Hand already complete")
	}
	return nil
}

func validatePostBlind(cmd *examples.PostBlind, state HandState) (*PlayerHandState, error) {
	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if cmd.Amount <= 0 {
		return nil, angzarr.NewCommandRejectedError("Amount must be positive")
	}
	return player, nil
}

func computeBlindPosted(cmd *examples.PostBlind, state HandState, player *PlayerHandState) *examples.BlindPosted {
	actualAmount := cmd.Amount
	if actualAmount > player.Stack {
		actualAmount = player.Stack
	}

	newStack := player.Stack - actualAmount
	newPot := state.TotalPot() + actualAmount

	return &examples.BlindPosted{
		PlayerRoot:  cmd.PlayerRoot,
		BlindType:   cmd.BlindType,
		Amount:      actualAmount,
		PlayerStack: newStack,
		PotTotal:    newPot,
		PostedAt:    timestamppb.New(time.Now()),
	}
}

// HandlePostBlind handles the PostBlind command.
func HandlePostBlind(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state HandState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.PostBlind
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardPostBlind(state); err != nil {
		return nil, err
	}
	player, err := validatePostBlind(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computeBlindPosted(&cmd, state, player)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
