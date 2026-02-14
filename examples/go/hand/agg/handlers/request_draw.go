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

func guardRequestDraw(state HandState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.GameVariant != examples.GameVariant_FIVE_CARD_DRAW {
		return angzarr.NewCommandRejectedError("Draw is not supported in this game variant")
	}
	return nil
}

func validateRequestDraw(cmd *examples.RequestDraw, state HandState) (*PlayerHandState, int, error) {
	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, 0, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, 0, angzarr.NewCommandRejectedError("Player has folded")
	}

	cardsToDiscard := len(cmd.CardIndices)
	for _, idx := range cmd.CardIndices {
		if idx < 0 || int(idx) >= len(player.HoleCards) {
			return nil, 0, angzarr.NewCommandRejectedError("Invalid card index")
		}
	}

	seen := make(map[int32]bool)
	for _, idx := range cmd.CardIndices {
		if seen[idx] {
			return nil, 0, angzarr.NewCommandRejectedError("Duplicate card index")
		}
		seen[idx] = true
	}

	if len(state.RemainingDeck) < cardsToDiscard {
		return nil, 0, angzarr.NewCommandRejectedError("Not enough cards in deck")
	}

	return player, cardsToDiscard, nil
}

func computeDrawCompleted(cmd *examples.RequestDraw, state HandState, cardsToDiscard int) *examples.DrawCompleted {
	newCards := make([]*examples.Card, cardsToDiscard)
	for i := 0; i < cardsToDiscard; i++ {
		newCards[i] = state.RemainingDeck[i]
	}

	return &examples.DrawCompleted{
		PlayerRoot:     cmd.PlayerRoot,
		CardsDiscarded: int32(cardsToDiscard),
		CardsDrawn:     int32(cardsToDiscard),
		NewCards:       newCards,
		DrawnAt:        timestamppb.New(time.Now()),
	}
}

// HandleRequestDraw handles the RequestDraw command for Five Card Draw.
func HandleRequestDraw(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state HandState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.RequestDraw
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardRequestDraw(state); err != nil {
		return nil, err
	}
	_, cardsToDiscard, err := validateRequestDraw(&cmd, state)
	if err != nil {
		return nil, err
	}

	event := computeDrawCompleted(&cmd, state, cardsToDiscard)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
