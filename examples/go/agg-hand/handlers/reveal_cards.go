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
		eventAny.TypeUrl = "type.poker/examples.CardsMucked"
	} else {
		// Player reveals their cards
		ranking := evaluateHand(player.HoleCards, state.CommunityCards, state.GameVariant)

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
		eventAny.TypeUrl = "type.poker/examples.CardsRevealed"
	}

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

// evaluateHand evaluates a poker hand and returns its ranking.
// This is a simplified implementation - production would need full hand evaluation.
func evaluateHand(holeCards, communityCards []*examples.Card, variant examples.GameVariant) *examples.HandRanking {
	// Combine all available cards
	allCards := append(holeCards, communityCards...)

	if len(allCards) < 5 {
		return &examples.HandRanking{
			RankType: examples.HandRankType_HIGH_CARD,
			Score:    0,
		}
	}

	// Count ranks and suits
	rankCounts := make(map[examples.Rank]int)
	suitCounts := make(map[examples.Suit]int)

	for _, card := range allCards {
		rankCounts[card.Rank]++
		suitCounts[card.Suit]++
	}

	// Check for flush
	hasFlush := false
	for _, count := range suitCounts {
		if count >= 5 {
			hasFlush = true
			break
		}
	}

	// Check for pairs, trips, quads
	pairs := 0
	threeOfKind := false
	fourOfKind := false
	highRank := examples.Rank_TWO

	for rank, count := range rankCounts {
		if count == 2 {
			pairs++
		} else if count == 3 {
			threeOfKind = true
		} else if count == 4 {
			fourOfKind = true
		}
		if rank > highRank {
			highRank = rank
		}
	}

	// Simplified ranking (production would check straights, full evaluation)
	var rankType examples.HandRankType
	score := int32(highRank)

	switch {
	case fourOfKind:
		rankType = examples.HandRankType_FOUR_OF_A_KIND
		score += 800
	case threeOfKind && pairs > 0:
		rankType = examples.HandRankType_FULL_HOUSE
		score += 700
	case hasFlush:
		rankType = examples.HandRankType_FLUSH
		score += 600
	case threeOfKind:
		rankType = examples.HandRankType_THREE_OF_A_KIND
		score += 400
	case pairs >= 2:
		rankType = examples.HandRankType_TWO_PAIR
		score += 300
	case pairs == 1:
		rankType = examples.HandRankType_PAIR
		score += 200
	default:
		rankType = examples.HandRankType_HIGH_CARD
		score += 100
	}

	return &examples.HandRanking{
		RankType: rankType,
		Score:    score,
	}
}
