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

// HandleRequestDraw handles the RequestDraw command for Five Card Draw.
func HandleRequestDraw(
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

	// Only Five Card Draw supports drawing
	if state.GameVariant != examples.GameVariant_FIVE_CARD_DRAW {
		return nil, angzarr.NewCommandRejectedError("Draw is not supported in this game variant")
	}

	var cmd examples.RequestDraw
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

	// Validate card indices
	cardsToDiscard := len(cmd.CardIndices)
	for _, idx := range cmd.CardIndices {
		if idx < 0 || int(idx) >= len(player.HoleCards) {
			return nil, angzarr.NewCommandRejectedError("Invalid card index")
		}
	}

	// Check for duplicate indices
	seen := make(map[int32]bool)
	for _, idx := range cmd.CardIndices {
		if seen[idx] {
			return nil, angzarr.NewCommandRejectedError("Duplicate card index")
		}
		seen[idx] = true
	}

	// Check we have enough cards in deck
	if len(state.RemainingDeck) < cardsToDiscard {
		return nil, angzarr.NewCommandRejectedError("Not enough cards in deck")
	}

	// Draw new cards from the deck
	newCards := make([]*examples.Card, cardsToDiscard)
	for i := 0; i < cardsToDiscard; i++ {
		newCards[i] = state.RemainingDeck[i]
	}

	event := &examples.DrawCompleted{
		PlayerRoot:     cmd.PlayerRoot,
		CardsDiscarded: int32(cardsToDiscard),
		CardsDrawn:     int32(cardsToDiscard),
		NewCards:       newCards,
		DrawnAt:        timestamppb.New(time.Now()),
	}

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}
