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

// HandleRevealCards handles revealing or mucking cards at showdown.
func HandleRevealCards(
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

	var cmd examples.RevealCards
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

	var eventAny *anypb.Any
	var err error

	if cmd.Muck {
		// Player mucks their cards
		event := &examples.CardsMucked{
			PlayerRoot: cmd.PlayerRoot,
			MuckedAt:   timestamppb.New(time.Now()),
		}
		eventAny, err = anypb.New(event)
		if err != nil {
			return nil, err
		}
	} else {
		// Player reveals their cards - use proper game rules for hand evaluation
		rules := GetRules(state.GameVariant)
		handRank := rules.EvaluateHand(player.HoleCards, state.CommunityCards)

		ranking := &examples.HandRanking{
			RankType: handRank.RankType,
			Score:    handRank.Score,
		}

		event := &examples.CardsRevealed{
			PlayerRoot: cmd.PlayerRoot,
			Cards:      player.HoleCards,
			Ranking:    ranking,
			RevealedAt: timestamppb.New(time.Now()),
		}
		eventAny, err = anypb.New(event)
		if err != nil {
			return nil, err
		}
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}

