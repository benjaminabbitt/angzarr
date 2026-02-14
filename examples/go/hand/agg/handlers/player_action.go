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

func guardPlayerAction(state HandState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.Status != "betting" {
		return angzarr.NewCommandRejectedError("Not in betting phase")
	}
	return nil
}

type validatedAction struct {
	player       *PlayerHandState
	actualAmount int64
}

func validatePlayerAction(cmd *examples.PlayerAction, state HandState) (*validatedAction, error) {
	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, angzarr.NewCommandRejectedError("Player has folded")
	}
	if player.IsAllIn {
		return nil, angzarr.NewCommandRejectedError("Player is all-in")
	}

	amountToCall := state.CurrentBet - player.BetThisRound
	actualAmount := int64(0)

	switch cmd.Action {
	case examples.ActionType_FOLD:
		// Always valid

	case examples.ActionType_CHECK:
		if amountToCall > 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot check, must call or fold")
		}

	case examples.ActionType_CALL:
		if amountToCall <= 0 {
			return nil, angzarr.NewCommandRejectedError("Nothing to call")
		}
		actualAmount = amountToCall
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_BET:
		if state.CurrentBet > 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot bet, use raise")
		}
		minBet := state.MinRaise
		if minBet == 0 {
			minBet = 10
		}
		if cmd.Amount < minBet {
			return nil, angzarr.NewCommandRejectedError(fmt.Sprintf("Bet must be at least %d", minBet))
		}
		actualAmount = cmd.Amount
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_RAISE:
		if state.CurrentBet <= 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot raise, use bet")
		}
		totalBet := cmd.Amount
		raiseAmount := totalBet - state.CurrentBet
		if raiseAmount < state.MinRaise {
			return nil, angzarr.NewCommandRejectedError("Raise below minimum")
		}
		actualAmount = totalBet - player.BetThisRound
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_ALL_IN:
		actualAmount = player.Stack

	default:
		return nil, angzarr.NewCommandRejectedError("Unknown action")
	}

	return &validatedAction{player: player, actualAmount: actualAmount}, nil
}

func computeActionTaken(cmd *examples.PlayerAction, state HandState, va *validatedAction) *examples.ActionTaken {
	newStack := va.player.Stack - va.actualAmount
	newPot := state.TotalPot() + va.actualAmount

	newCurrentBet := state.CurrentBet
	playerTotalBet := va.player.BetThisRound + va.actualAmount
	if playerTotalBet > newCurrentBet {
		newCurrentBet = playerTotalBet
	}

	action := cmd.Action
	if va.actualAmount == va.player.Stack && va.actualAmount > 0 {
		action = examples.ActionType_ALL_IN
	}

	amountToEmit := va.actualAmount
	if cmd.Action == examples.ActionType_BET || cmd.Action == examples.ActionType_RAISE {
		amountToEmit = cmd.Amount
	}

	return &examples.ActionTaken{
		PlayerRoot:   cmd.PlayerRoot,
		Action:       action,
		Amount:       amountToEmit,
		PlayerStack:  newStack,
		PotTotal:     newPot,
		AmountToCall: newCurrentBet,
		ActionAt:     timestamppb.New(time.Now()),
	}
}

// HandlePlayerAction handles the PlayerAction command.
func HandlePlayerAction(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state HandState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.PlayerAction
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardPlayerAction(state); err != nil {
		return nil, err
	}
	va, err := validatePlayerAction(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computeActionTaken(&cmd, state, va)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
