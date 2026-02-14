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

func guardRevealCards(state HandState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return angzarr.NewCommandRejectedError("Hand already complete")
	}
	return nil
}

func validateRevealCards(cmd *examples.RevealCards, state HandState) (*PlayerHandState, error) {
	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, angzarr.NewCommandRejectedError("Player has folded")
	}
	return player, nil
}

func computeCardsMucked(cmd *examples.RevealCards) *examples.CardsMucked {
	return &examples.CardsMucked{
		PlayerRoot: cmd.PlayerRoot,
		MuckedAt:   timestamppb.New(time.Now()),
	}
}

func computeCardsRevealed(cmd *examples.RevealCards, state HandState, player *PlayerHandState) *examples.CardsRevealed {
	rules := GetRules(state.GameVariant)
	handRank := rules.EvaluateHand(player.HoleCards, state.CommunityCards)

	ranking := &examples.HandRanking{
		RankType: handRank.RankType,
		Score:    handRank.Score,
	}

	return &examples.CardsRevealed{
		PlayerRoot: cmd.PlayerRoot,
		Cards:      player.HoleCards,
		Ranking:    ranking,
		RevealedAt: timestamppb.New(time.Now()),
	}
}

// HandleRevealCards handles revealing or mucking cards at showdown.
func HandleRevealCards(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state HandState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.RevealCards
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardRevealCards(state); err != nil {
		return nil, err
	}
	player, err := validateRevealCards(&cmd, state)
	if err != nil {
		return nil, err
	}

	var eventAny *anypb.Any

	if cmd.Muck {
		event := computeCardsMucked(&cmd)
		eventAny, err = anypb.New(event)
	} else {
		event := computeCardsRevealed(&cmd, state, player)
		eventAny, err = anypb.New(event)
	}
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}

