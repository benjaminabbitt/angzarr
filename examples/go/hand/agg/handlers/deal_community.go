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

func guardDealCommunityCards(state HandState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.GameVariant == examples.GameVariant_FIVE_CARD_DRAW {
		return angzarr.NewCommandRejectedError("Five Card Draw does not use community cards")
	}
	return nil
}

type validatedCommunityDeal struct {
	newPhase    examples.BettingPhase
	cardsToDeal int
}

func validateDealCommunityCards(cmd *examples.DealCommunityCards, state HandState) (*validatedCommunityDeal, error) {
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

	if cmd.Count > 0 && int(cmd.Count) != cardsToDeal {
		return nil, angzarr.NewCommandRejectedError("Invalid card count for phase")
	}

	if len(state.RemainingDeck) < cardsToDeal {
		return nil, angzarr.NewCommandRejectedError("Not enough cards in deck")
	}

	return &validatedCommunityDeal{newPhase: newPhase, cardsToDeal: cardsToDeal}, nil
}

func computeCommunityCardsDealt(state HandState, vcd *validatedCommunityDeal) *examples.CommunityCardsDealt {
	newCards := state.RemainingDeck[:vcd.cardsToDeal]
	allCommunity := append(state.CommunityCards, newCards...)

	return &examples.CommunityCardsDealt{
		Cards:             newCards,
		Phase:             vcd.newPhase,
		AllCommunityCards: allCommunity,
		DealtAt:           timestamppb.New(time.Now()),
	}
}

// HandleDealCommunityCards handles dealing community cards (flop/turn/river).
func HandleDealCommunityCards(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state HandState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.DealCommunityCards
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardDealCommunityCards(state); err != nil {
		return nil, err
	}
	vcd, err := validateDealCommunityCards(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computeCommunityCardsDealt(state, vcd)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
