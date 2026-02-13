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

// HandleDealCommunityCards handles dealing community cards (flop/turn/river).
func HandleDealCommunityCards(
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

	// Check game variant supports community cards
	if state.GameVariant == examples.GameVariant_FIVE_CARD_DRAW {
		return nil, angzarr.NewCommandRejectedError("Five Card Draw does not use community cards")
	}

	var cmd examples.DealCommunityCards
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	// Determine next phase and cards to deal
	var newPhase examples.BettingPhase
	var cardsToDeal int

	switch state.CurrentPhase {
	case examples.BettingPhase_PREFLOP:
		newPhase = examples.BettingPhase_FLOP
		cardsToDeal = 3
	case examples.BettingPhase_FLOP:
		newPhase = examples.BettingPhase_TURN
		cardsToDeal = 1
	case examples.BettingPhase_TURN:
		newPhase = examples.BettingPhase_RIVER
		cardsToDeal = 1
	default:
		return nil, angzarr.NewCommandRejectedError("Cannot deal more community cards")
	}

	// Validate count if specified
	if cmd.Count > 0 && int(cmd.Count) != cardsToDeal {
		return nil, angzarr.NewCommandRejectedError("Invalid card count for phase")
	}

	// Check we have enough cards in deck
	if len(state.RemainingDeck) < cardsToDeal {
		return nil, angzarr.NewCommandRejectedError("Not enough cards in deck")
	}

	// Deal cards from remaining deck
	newCards := state.RemainingDeck[:cardsToDeal]
	allCommunity := append(state.CommunityCards, newCards...)

	event := &examples.CommunityCardsDealt{
		Cards:             newCards,
		Phase:             newPhase,
		AllCommunityCards: allCommunity,
		DealtAt:           timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}
	eventAny.TypeUrl = "type.poker/examples.CommunityCardsDealt"

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
