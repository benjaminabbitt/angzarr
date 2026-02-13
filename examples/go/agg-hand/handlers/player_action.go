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

// HandlePlayerAction handles the PlayerAction command.
func HandlePlayerAction(
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
	if state.Status != "betting" {
		return nil, angzarr.NewCommandRejectedError("Not in betting phase")
	}

	var cmd examples.PlayerAction
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

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

	// Validate action
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
			actualAmount = player.Stack // All-in call
		}

	case examples.ActionType_BET:
		if state.CurrentBet > 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot bet, use raise")
		}
		minBet := state.MinRaise
		if minBet == 0 {
			minBet = 10 // Default big blind
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

	newStack := player.Stack - actualAmount
	newPot := state.TotalPot() + actualAmount

	// Calculate new current bet
	newCurrentBet := state.CurrentBet
	playerTotalBet := player.BetThisRound + actualAmount
	if playerTotalBet > newCurrentBet {
		newCurrentBet = playerTotalBet
	}

	// Action to emit
	action := cmd.Action
	if actualAmount == player.Stack && actualAmount > 0 {
		action = examples.ActionType_ALL_IN
	}

	// Amount to emit in event depends on action type
	// For BET/RAISE, use total bet; for CALL, use chips put in
	amountToEmit := actualAmount
	if cmd.Action == examples.ActionType_BET || cmd.Action == examples.ActionType_RAISE {
		amountToEmit = cmd.Amount
	}

	event := &examples.ActionTaken{
		PlayerRoot:   cmd.PlayerRoot,
		Action:       action,
		Amount:       amountToEmit,
		PlayerStack:  newStack,
		PotTotal:     newPot,
		AmountToCall: newCurrentBet,
		ActionAt:     timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}
	eventAny.TypeUrl = "type.poker/examples.ActionTaken"

	return &pb.EventBook{
		Cover: commandBook.Cover,
		Pages: []*pb.EventPage{
			{
				Sequence:  &pb.EventPage_Num{Num: seq},
				Event:     eventAny,
				CreatedAt: timestamppb.New(time.Now()),
			},
		},
	}, nil
}
