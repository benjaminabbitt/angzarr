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

func guardAwardPot(state HandState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return angzarr.NewCommandRejectedError("Hand already complete")
	}
	return nil
}

func validateAwardPot(cmd *examples.AwardPot, state HandState) error {
	if len(cmd.Awards) == 0 {
		return angzarr.NewCommandRejectedError("No awards specified")
	}

	totalAwarded := int64(0)
	for _, award := range cmd.Awards {
		player := state.GetPlayerByRoot(award.PlayerRoot)
		if player == nil {
			return angzarr.NewCommandRejectedError("Award to player not in hand")
		}
		if player.HasFolded {
			return angzarr.NewCommandRejectedError("Cannot award to folded player")
		}
		totalAwarded += award.Amount
	}

	if totalAwarded > state.TotalPot() {
		return angzarr.NewCommandRejectedError("Awards exceed pot total")
	}

	return nil
}

func computePotAwarded(cmd *examples.AwardPot, now time.Time) (*examples.PotAwarded, []*examples.PotWinner) {
	winners := make([]*examples.PotWinner, len(cmd.Awards))
	for i, award := range cmd.Awards {
		winners[i] = &examples.PotWinner{
			PlayerRoot: award.PlayerRoot,
			Amount:     award.Amount,
			PotType:    award.PotType,
		}
	}

	return &examples.PotAwarded{
		Winners:   winners,
		AwardedAt: timestamppb.New(now),
	}, winners
}

func computeHandComplete(cmd *examples.AwardPot, state HandState, winners []*examples.PotWinner, now time.Time) *examples.HandComplete {
	finalStacks := make([]*examples.PlayerStackSnapshot, 0, len(state.Players))
	for _, player := range state.Players {
		finalStack := player.Stack
		for _, award := range cmd.Awards {
			if state.GetPlayerByRoot(award.PlayerRoot) == player {
				finalStack += award.Amount
			}
		}
		finalStacks = append(finalStacks, &examples.PlayerStackSnapshot{
			PlayerRoot: player.PlayerRoot,
			Stack:      finalStack,
			IsAllIn:    player.IsAllIn,
			HasFolded:  player.HasFolded,
		})
	}

	return &examples.HandComplete{
		TableRoot:   state.TableRoot,
		HandNumber:  state.HandNumber,
		Winners:     winners,
		FinalStacks: finalStacks,
		CompletedAt: timestamppb.New(now),
	}
}

// HandleAwardPot handles awarding the pot to winners.
func HandleAwardPot(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state HandState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.AwardPot
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardAwardPot(state); err != nil {
		return nil, err
	}
	if err := validateAwardPot(&cmd, state); err != nil {
		return nil, err
	}

	now := time.Now()
	potAwardedEvent, winners := computePotAwarded(&cmd, now)
	handCompleteEvent := computeHandComplete(&cmd, state, winners, now)

	potAwardedAny, err := anypb.New(potAwardedEvent)
	if err != nil {
		return nil, err
	}

	handCompleteAny, err := anypb.New(handCompleteEvent)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBookMulti(commandBook.Cover, seq, potAwardedAny, handCompleteAny), nil
}
