package handlers

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	mathrand "math/rand"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

func guardDealCards(state HandState) error {
	if state.Exists() {
		return angzarr.NewCommandRejectedError("Hand already dealt")
	}
	return nil
}

func validateDealCards(cmd *examples.DealCards) error {
	if len(cmd.Players) < 2 {
		return angzarr.NewCommandRejectedError("Need at least 2 players")
	}
	return nil
}

func computeCardsDealt(cmd *examples.DealCards) *examples.CardsDealt {
	deck := createDeck()
	seed := cmd.DeckSeed
	if len(seed) == 0 {
		seed = make([]byte, 32)
		rand.Read(seed)
	}
	shuffleDeck(deck, seed)

	cardsPerPlayer := getCardsPerPlayer(cmd.GameVariant)
	playerCards := make([]*examples.PlayerHoleCards, len(cmd.Players))

	for i, player := range cmd.Players {
		cards := make([]*examples.Card, cardsPerPlayer)
		for j := 0; j < cardsPerPlayer; j++ {
			cards[j] = deck[0]
			deck = deck[1:]
		}
		playerCards[i] = &examples.PlayerHoleCards{
			PlayerRoot: player.PlayerRoot,
			Cards:      cards,
		}
	}

	return &examples.CardsDealt{
		TableRoot:      cmd.TableRoot,
		HandNumber:     cmd.HandNumber,
		GameVariant:    cmd.GameVariant,
		PlayerCards:    playerCards,
		DealerPosition: cmd.DealerPosition,
		Players:        cmd.Players,
		DealtAt:        timestamppb.New(time.Now()),
	}
}

// HandleDealCards handles the DealCards command to start a new hand.
func HandleDealCards(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state HandState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.DealCards
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardDealCards(state); err != nil {
		return nil, err
	}
	if err := validateDealCards(&cmd); err != nil {
		return nil, err
	}

	event := computeCardsDealt(&cmd)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}

// createDeck creates a standard 52-card deck.
func createDeck() []*examples.Card {
	suits := []examples.Suit{
		examples.Suit_CLUBS,
		examples.Suit_DIAMONDS,
		examples.Suit_HEARTS,
		examples.Suit_SPADES,
	}
	ranks := []examples.Rank{
		examples.Rank_TWO, examples.Rank_THREE, examples.Rank_FOUR,
		examples.Rank_FIVE, examples.Rank_SIX, examples.Rank_SEVEN,
		examples.Rank_EIGHT, examples.Rank_NINE, examples.Rank_TEN,
		examples.Rank_JACK, examples.Rank_QUEEN, examples.Rank_KING,
		examples.Rank_ACE,
	}

	deck := make([]*examples.Card, 0, 52)
	for _, suit := range suits {
		for _, rank := range ranks {
			deck = append(deck, &examples.Card{Suit: suit, Rank: rank})
		}
	}
	return deck
}

// shuffleDeck shuffles the deck using a seed for determinism.
func shuffleDeck(deck []*examples.Card, seed []byte) {
	h := sha256.Sum256(seed)
	seedInt := int64(binary.BigEndian.Uint64(h[:8]))
	rng := mathrand.New(mathrand.NewSource(seedInt))

	for i := len(deck) - 1; i > 0; i-- {
		j := rng.Intn(i + 1)
		deck[i], deck[j] = deck[j], deck[i]
	}
}

// getCardsPerPlayer returns hole cards count based on game variant.
func getCardsPerPlayer(variant examples.GameVariant) int {
	switch variant {
	case examples.GameVariant_TEXAS_HOLDEM:
		return 2
	case examples.GameVariant_OMAHA:
		return 4
	case examples.GameVariant_FIVE_CARD_DRAW:
		return 5
	case examples.GameVariant_SEVEN_CARD_STUD:
		return 2 // Initial deal, more come later
	default:
		return 2
	}
}
