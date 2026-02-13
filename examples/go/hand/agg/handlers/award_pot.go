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

// HandleAwardPot handles awarding the pot to winners.
func HandleAwardPot(
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

	var cmd examples.AwardPot
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if len(cmd.Awards) == 0 {
		return nil, angzarr.NewCommandRejectedError("No awards specified")
	}

	// Validate awards
	totalAwarded := int64(0)
	for _, award := range cmd.Awards {
		player := state.GetPlayerByRoot(award.PlayerRoot)
		if player == nil {
			return nil, angzarr.NewCommandRejectedError("Award to player not in hand")
		}
		if player.HasFolded {
			return nil, angzarr.NewCommandRejectedError("Cannot award to folded player")
		}
		totalAwarded += award.Amount
	}

	// Verify total matches pot
	if totalAwarded > state.TotalPot() {
		return nil, angzarr.NewCommandRejectedError("Awards exceed pot total")
	}

	// Build pot winners
	winners := make([]*examples.PotWinner, len(cmd.Awards))
	for i, award := range cmd.Awards {
		winners[i] = &examples.PotWinner{
			PlayerRoot: award.PlayerRoot,
			Amount:     award.Amount,
			PotType:    award.PotType,
		}
	}

	// Build events
	now := time.Now()

	potAwardedEvent := &examples.PotAwarded{
		Winners:   winners,
		AwardedAt: timestamppb.New(now),
	}

	potAwardedAny, err := anypb.New(potAwardedEvent)
	if err != nil {
		return nil, err
	}

	// Build final stacks
	finalStacks := make([]*examples.PlayerStackSnapshot, 0, len(state.Players))
	for _, player := range state.Players {
		finalStack := player.Stack
		// Add any winnings
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

	handCompleteEvent := &examples.HandComplete{
		TableRoot:   state.TableRoot,
		HandNumber:  state.HandNumber,
		Winners:     winners,
		FinalStacks: finalStacks,
		CompletedAt: timestamppb.New(now),
	}

	handCompleteAny, err := anypb.New(handCompleteEvent)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBookMulti(commandBook.Cover, seq, potAwardedAny, handCompleteAny), nil
}
