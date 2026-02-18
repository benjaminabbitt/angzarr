// Hand aggregate - rich domain model using OO pattern.
//
// This aggregate uses the OO-style pattern with embedded AggregateBase,
// method-based handlers, and fluent registration. This contrasts with
// the player aggregate which uses the functional CommandRouter pattern.
package main

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	mathrand "math/rand"
	"sort"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandState represents the current state of a hand aggregate.
type HandState struct {
	HandID      string
	TableRoot   []byte
	HandNumber  int64
	GameVariant examples.GameVariant

	// Deck state
	RemainingDeck []*examples.Card

	// Player state
	Players map[string]*PlayerHandState // player_root_hex -> state

	// Community cards
	CommunityCards []*examples.Card

	// Betting state
	CurrentPhase     examples.BettingPhase
	ActionOnPosition int32
	CurrentBet       int64
	MinRaise         int64
	Pots             []*PotState

	// Positions
	DealerPosition     int32
	SmallBlindPosition int32
	BigBlindPosition   int32

	Status string // "dealing", "betting", "showdown", "complete"
}

// PlayerHandState represents a player's state in the hand.
type PlayerHandState struct {
	PlayerRoot    []byte
	Position      int32
	HoleCards     []*examples.Card
	Stack         int64
	BetThisRound  int64
	TotalInvested int64
	HasActed      bool
	HasFolded     bool
	IsAllIn       bool
}

// PotState represents a pot (main or side).
type PotState struct {
	Amount          int64
	EligiblePlayers [][]byte
	PotType         string
}

// Hand aggregate with event sourcing using OO pattern.
type Hand struct {
	angzarr.AggregateBase[HandState]
}

// docs:start:oo_handlers
// NewHand creates a new Hand aggregate with prior events for state reconstruction.
func NewHand(eventBook *pb.EventBook) *Hand {
	h := &Hand{}
	h.Init(eventBook, func() HandState {
		return HandState{
			Players: make(map[string]*PlayerHandState),
			Pots:    []*PotState{{PotType: "main"}},
		}
	})
	h.SetDomain("hand")

	// Register event appliers
	h.Applies("CardsDealt", h.applyCardsDealt)
	h.Applies("BlindPosted", h.applyBlindPosted)
	h.Applies("ActionTaken", h.applyActionTaken)
	h.Applies("BettingRoundComplete", h.applyBettingRoundComplete)
	h.Applies("CommunityCardsDealt", h.applyCommunityCardsDealt)
	h.Applies("DrawCompleted", h.applyDrawCompleted)
	h.Applies("ShowdownStarted", h.applyShowdownStarted)
	h.Applies("CardsRevealed", h.applyCardsRevealed)
	h.Applies("CardsMucked", h.applyCardsMucked)
	h.Applies("PotAwarded", h.applyPotAwarded)
	h.Applies("HandComplete", h.applyHandComplete)

	// Register command handlers
	h.Handles("DealCards", h.dealCards)
	h.Handles("PostBlind", h.postBlind)
	h.Handles("PlayerAction", h.playerAction)
	h.Handles("DealCommunityCards", h.dealCommunityCards)
	h.Handles("RequestDraw", h.requestDraw)
	h.Handles("RevealCards", h.revealCards)
	h.HandlesMulti("AwardPot", h.awardPot)

	return h
}
// docs:end:oo_handlers

// --- Event Appliers ---

func (h *Hand) applyCardsDealt(state *HandState, event *examples.CardsDealt) {
	state.HandID = hex.EncodeToString(event.TableRoot) + "_" + fmt.Sprint(event.HandNumber)
	state.TableRoot = event.TableRoot
	state.HandNumber = event.HandNumber
	state.GameVariant = event.GameVariant
	state.DealerPosition = event.DealerPosition
	state.RemainingDeck = event.RemainingDeck
	state.CurrentPhase = examples.BettingPhase_PREFLOP
	state.Status = "betting"

	// Initialize players
	for _, p := range event.Players {
		key := hex.EncodeToString(p.PlayerRoot)
		state.Players[key] = &PlayerHandState{
			PlayerRoot: p.PlayerRoot,
			Position:   p.Position,
			Stack:      p.Stack,
		}
	}

	// Apply hole cards
	for _, pc := range event.PlayerCards {
		key := hex.EncodeToString(pc.PlayerRoot)
		if player := state.Players[key]; player != nil {
			player.HoleCards = pc.Cards
		}
	}
}

func (h *Hand) applyBlindPosted(state *HandState, event *examples.BlindPosted) {
	key := hex.EncodeToString(event.PlayerRoot)
	if player := state.Players[key]; player != nil {
		player.Stack = event.PlayerStack
		player.BetThisRound += event.Amount
		player.TotalInvested += event.Amount
	}
	state.Pots[0].Amount = event.PotTotal
	if event.Amount > state.CurrentBet {
		state.CurrentBet = event.Amount
	}
}

func (h *Hand) applyActionTaken(state *HandState, event *examples.ActionTaken) {
	key := hex.EncodeToString(event.PlayerRoot)
	if player := state.Players[key]; player != nil {
		player.Stack = event.PlayerStack
		player.HasActed = true

		switch event.Action {
		case examples.ActionType_FOLD:
			player.HasFolded = true
		case examples.ActionType_ALL_IN:
			player.IsAllIn = true
			player.BetThisRound += event.Amount
			player.TotalInvested += event.Amount
		case examples.ActionType_BET, examples.ActionType_RAISE, examples.ActionType_CALL:
			player.BetThisRound += event.Amount
			player.TotalInvested += event.Amount
		}
	}
	state.Pots[0].Amount = event.PotTotal
	state.CurrentBet = event.AmountToCall
}

func (h *Hand) applyBettingRoundComplete(state *HandState, event *examples.BettingRoundComplete) {
	// Reset for next round
	for _, p := range state.Players {
		p.BetThisRound = 0
		p.HasActed = false
	}
	state.CurrentBet = 0

	// Update stacks from snapshot
	for _, snap := range event.Stacks {
		key := hex.EncodeToString(snap.PlayerRoot)
		if player := state.Players[key]; player != nil {
			player.Stack = snap.Stack
			player.IsAllIn = snap.IsAllIn
			player.HasFolded = snap.HasFolded
		}
	}
}

func (h *Hand) applyCommunityCardsDealt(state *HandState, event *examples.CommunityCardsDealt) {
	state.CommunityCards = event.AllCommunityCards
	state.CurrentPhase = event.Phase
}

func (h *Hand) applyDrawCompleted(state *HandState, event *examples.DrawCompleted) {
	key := hex.EncodeToString(event.PlayerRoot)
	if player := state.Players[key]; player != nil {
		// Replace discarded cards with new cards
		if len(event.NewCards) > 0 {
			player.HoleCards = append(player.HoleCards[:len(player.HoleCards)-int(event.CardsDiscarded)], event.NewCards...)
		}
	}
	// Update remaining deck
	if int(event.CardsDrawn) <= len(state.RemainingDeck) {
		state.RemainingDeck = state.RemainingDeck[event.CardsDrawn:]
	}
}

func (h *Hand) applyShowdownStarted(state *HandState, _ *examples.ShowdownStarted) {
	state.Status = "showdown"
}

func (h *Hand) applyCardsRevealed(state *HandState, _ *examples.CardsRevealed) {
	// Cards revealed during showdown - could store revealed hands
}

func (h *Hand) applyCardsMucked(state *HandState, _ *examples.CardsMucked) {
	// Player mucked - could mark as mucked
}

func (h *Hand) applyPotAwarded(state *HandState, event *examples.PotAwarded) {
	for _, winner := range event.Winners {
		key := hex.EncodeToString(winner.PlayerRoot)
		if player := state.Players[key]; player != nil {
			player.Stack += winner.Amount
		}
	}
}

func (h *Hand) applyHandComplete(state *HandState, event *examples.HandComplete) {
	state.Status = "complete"
	// Update final stacks
	for _, snap := range event.FinalStacks {
		key := hex.EncodeToString(snap.PlayerRoot)
		if player := state.Players[key]; player != nil {
			player.Stack = snap.Stack
		}
	}
}

// --- State Accessors ---

func (h *Hand) exists() bool {
	return h.State().HandID != ""
}

func (h *Hand) isComplete() bool {
	return h.State().Status == "complete"
}

func (h *Hand) totalPot() int64 {
	total := int64(0)
	for _, pot := range h.State().Pots {
		total += pot.Amount
	}
	return total
}

func (h *Hand) getPlayerByRoot(root []byte) *PlayerHandState {
	return h.State().Players[hex.EncodeToString(root)]
}

func (h *Hand) getPlayerByPosition(pos int32) *PlayerHandState {
	for _, p := range h.State().Players {
		if p.Position == pos {
			return p
		}
	}
	return nil
}

// --- Command Handlers ---

func (h *Hand) dealCards(cmd *examples.DealCards) (*examples.CardsDealt, error) {
	// Guard
	if h.exists() {
		return nil, angzarr.NewCommandRejectedError("Hand already dealt")
	}

	// Validate
	if len(cmd.Players) < 2 {
		return nil, angzarr.NewCommandRejectedError("Need at least 2 players")
	}

	// Compute
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
		RemainingDeck:  deck,
		DealtAt:        timestamppb.New(time.Now()),
	}, nil
}

func (h *Hand) postBlind(cmd *examples.PostBlind) (*examples.BlindPosted, error) {
	// Guard
	if !h.exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if h.isComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}

	// Validate
	player := h.getPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if cmd.Amount <= 0 {
		return nil, angzarr.NewCommandRejectedError("Amount must be positive")
	}

	// Compute
	actualAmount := cmd.Amount
	if actualAmount > player.Stack {
		actualAmount = player.Stack
	}

	newStack := player.Stack - actualAmount
	newPot := h.totalPot() + actualAmount

	return &examples.BlindPosted{
		PlayerRoot:  cmd.PlayerRoot,
		BlindType:   cmd.BlindType,
		Amount:      actualAmount,
		PlayerStack: newStack,
		PotTotal:    newPot,
		PostedAt:    timestamppb.New(time.Now()),
	}, nil
}

func (h *Hand) playerAction(cmd *examples.PlayerAction) (*examples.ActionTaken, error) {
	state := h.State()

	// Guard
	if !h.exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if h.isComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.Status != "betting" {
		return nil, angzarr.NewCommandRejectedError("Not in betting phase")
	}

	// Validate
	player := h.getPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, angzarr.NewCommandRejectedError("Player has folded")
	}
	if player.IsAllIn {
		return nil, angzarr.NewCommandRejectedError("Player is all-in")
	}

	amountToCall := state.CurrentBet - player.BetThisRound
	actualAmount := int64(0)

	switch cmd.Action {
	case examples.ActionType_FOLD:
		// Always valid

	case examples.ActionType_CHECK:
		if amountToCall > 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot check, must call or fold")
		}

	case examples.ActionType_CALL:
		if amountToCall <= 0 {
			return nil, angzarr.NewCommandRejectedError("Nothing to call")
		}
		actualAmount = amountToCall
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_BET:
		if state.CurrentBet > 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot bet, use raise")
		}
		minBet := state.MinRaise
		if minBet == 0 {
			minBet = 10
		}
		if cmd.Amount < minBet {
			return nil, angzarr.NewCommandRejectedError(fmt.Sprintf("Bet must be at least %d", minBet))
		}
		actualAmount = cmd.Amount
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_RAISE:
		if state.CurrentBet <= 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot raise, use bet")
		}
		totalBet := cmd.Amount
		raiseAmount := totalBet - state.CurrentBet
		if raiseAmount < state.MinRaise {
			return nil, angzarr.NewCommandRejectedError("Raise below minimum")
		}
		actualAmount = totalBet - player.BetThisRound
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_ALL_IN:
		actualAmount = player.Stack

	default:
		return nil, angzarr.NewCommandRejectedError("Unknown action")
	}

	// Compute
	newStack := player.Stack - actualAmount
	newPot := h.totalPot() + actualAmount

	newCurrentBet := state.CurrentBet
	playerTotalBet := player.BetThisRound + actualAmount
	if playerTotalBet > newCurrentBet {
		newCurrentBet = playerTotalBet
	}

	action := cmd.Action
	if actualAmount == player.Stack && actualAmount > 0 {
		action = examples.ActionType_ALL_IN
	}

	amountToEmit := actualAmount
	if cmd.Action == examples.ActionType_BET || cmd.Action == examples.ActionType_RAISE {
		amountToEmit = cmd.Amount
	}

	return &examples.ActionTaken{
		PlayerRoot:   cmd.PlayerRoot,
		Action:       action,
		Amount:       amountToEmit,
		PlayerStack:  newStack,
		PotTotal:     newPot,
		AmountToCall: newCurrentBet,
		ActionAt:     timestamppb.New(time.Now()),
	}, nil
}

func (h *Hand) dealCommunityCards(cmd *examples.DealCommunityCards) (*examples.CommunityCardsDealt, error) {
	state := h.State()

	// Guard
	if !h.exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if h.isComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.GameVariant == examples.GameVariant_FIVE_CARD_DRAW {
		return nil, angzarr.NewCommandRejectedError("Five Card Draw does not use community cards")
	}

	// Validate
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

	// Compute
	newCards := state.RemainingDeck[:cardsToDeal]
	allCommunity := append(state.CommunityCards, newCards...)

	return &examples.CommunityCardsDealt{
		Cards:             newCards,
		Phase:             newPhase,
		AllCommunityCards: allCommunity,
		DealtAt:           timestamppb.New(time.Now()),
	}, nil
}

func (h *Hand) requestDraw(cmd *examples.RequestDraw) (*examples.DrawCompleted, error) {
	state := h.State()

	// Guard
	if !h.exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if h.isComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.GameVariant != examples.GameVariant_FIVE_CARD_DRAW {
		return nil, angzarr.NewCommandRejectedError("Draw is not supported in this game variant")
	}

	// Validate
	player := h.getPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, angzarr.NewCommandRejectedError("Player has folded")
	}

	cardsToDiscard := len(cmd.CardIndices)
	for _, idx := range cmd.CardIndices {
		if idx < 0 || int(idx) >= len(player.HoleCards) {
			return nil, angzarr.NewCommandRejectedError("Invalid card index")
		}
	}

	seen := make(map[int32]bool)
	for _, idx := range cmd.CardIndices {
		if seen[idx] {
			return nil, angzarr.NewCommandRejectedError("Duplicate card index")
		}
		seen[idx] = true
	}

	if len(state.RemainingDeck) < cardsToDiscard {
		return nil, angzarr.NewCommandRejectedError("Not enough cards in deck")
	}

	// Compute
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
	}, nil
}

func (h *Hand) revealCards(cmd *examples.RevealCards) (*examples.CardsRevealed, error) {
	state := h.State()

	// Guard
	if !h.exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if h.isComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}

	// Validate
	player := h.getPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, angzarr.NewCommandRejectedError("Player has folded")
	}

	// Compute - muck is handled separately
	if cmd.Muck {
		// For muck, we return nil and the caller handles it
		// This is a limitation - we'd need a different handler signature
		// For now, we emit CardsRevealed with empty cards to indicate muck
		return &examples.CardsRevealed{
			PlayerRoot: cmd.PlayerRoot,
			Cards:      nil,
			Ranking:    nil,
			RevealedAt: timestamppb.New(time.Now()),
		}, nil
	}

	rules := getGameRules(state.GameVariant)
	handRank := rules.evaluateHand(player.HoleCards, state.CommunityCards)

	ranking := &examples.HandRanking{
		RankType: handRank.rankType,
		Score:    handRank.score,
	}

	return &examples.CardsRevealed{
		PlayerRoot: cmd.PlayerRoot,
		Cards:      player.HoleCards,
		Ranking:    ranking,
		RevealedAt: timestamppb.New(time.Now()),
	}, nil
}

func (h *Hand) awardPot(cmd *examples.AwardPot) ([]proto.Message, error) {
	state := h.State()

	// Guard
	if !h.exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if h.isComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}

	// Validate
	if len(cmd.Awards) == 0 {
		return nil, angzarr.NewCommandRejectedError("No awards specified")
	}

	totalAwarded := int64(0)
	for _, award := range cmd.Awards {
		player := h.getPlayerByRoot(award.PlayerRoot)
		if player == nil {
			return nil, angzarr.NewCommandRejectedError("Award to player not in hand")
		}
		if player.HasFolded {
			return nil, angzarr.NewCommandRejectedError("Cannot award to folded player")
		}
		totalAwarded += award.Amount
	}

	if totalAwarded > h.totalPot() {
		return nil, angzarr.NewCommandRejectedError("Awards exceed pot total")
	}

	// Compute - return both events
	now := time.Now()
	winners := make([]*examples.PotWinner, len(cmd.Awards))
	for i, award := range cmd.Awards {
		winners[i] = &examples.PotWinner{
			PlayerRoot: award.PlayerRoot,
			Amount:     award.Amount,
			PotType:    award.PotType,
		}
	}

	finalStacks := make([]*examples.PlayerStackSnapshot, 0, len(state.Players))
	for _, player := range state.Players {
		finalStack := player.Stack
		for _, award := range cmd.Awards {
			if h.getPlayerByRoot(award.PlayerRoot) == player {
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

	// Return both events
	return []proto.Message{
		&examples.PotAwarded{
			Winners:   winners,
			AwardedAt: timestamppb.New(now),
		},
		&examples.HandComplete{
			TableRoot:   state.TableRoot,
			HandNumber:  state.HandNumber,
			Winners:     winners,
			FinalStacks: finalStacks,
			CompletedAt: timestamppb.New(now),
		},
	}, nil
}

// --- Helper Functions ---

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

func shuffleDeck(deck []*examples.Card, seed []byte) {
	hash := sha256.Sum256(seed)
	seedInt := int64(binary.BigEndian.Uint64(hash[:8]))
	rng := mathrand.New(mathrand.NewSource(seedInt))

	for i := len(deck) - 1; i > 0; i-- {
		j := rng.Intn(i + 1)
		deck[i], deck[j] = deck[j], deck[i]
	}
}

func getCardsPerPlayer(variant examples.GameVariant) int {
	switch variant {
	case examples.GameVariant_TEXAS_HOLDEM:
		return 2
	case examples.GameVariant_OMAHA:
		return 4
	case examples.GameVariant_FIVE_CARD_DRAW:
		return 5
	case examples.GameVariant_SEVEN_CARD_STUD:
		return 2
	default:
		return 2
	}
}

// --- Game Rules ---

type handRank struct {
	rankType examples.HandRankType
	score    int32
	kickers  []examples.Rank
}

type gameRules interface {
	holeCardCount() int
	evaluateHand(holeCards, communityCards []*examples.Card) *handRank
	usesCommunityCards() bool
}

func getGameRules(variant examples.GameVariant) gameRules {
	switch variant {
	case examples.GameVariant_TEXAS_HOLDEM:
		return &texasHoldemRules{}
	case examples.GameVariant_OMAHA:
		return &omahaRules{}
	case examples.GameVariant_FIVE_CARD_DRAW:
		return &fiveCardDrawRules{}
	default:
		return &texasHoldemRules{}
	}
}

type texasHoldemRules struct{}

func (r *texasHoldemRules) holeCardCount() int { return 2 }

func (r *texasHoldemRules) evaluateHand(holeCards, communityCards []*examples.Card) *handRank {
	allCards := append(holeCards, communityCards...)
	if len(allCards) < 5 {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}
	return findBestFive(allCards)
}

func (r *texasHoldemRules) usesCommunityCards() bool { return true }

type omahaRules struct{}

func (r *omahaRules) holeCardCount() int { return 4 }

func (r *omahaRules) evaluateHand(holeCards, communityCards []*examples.Card) *handRank {
	if len(holeCards) < 2 || len(communityCards) < 3 {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}

	var bestRank *handRank

	holeCombos := combinations(holeCards, 2)
	commCombos := combinations(communityCards, 3)

	for _, holeCombo := range holeCombos {
		for _, commCombo := range commCombos {
			five := append(holeCombo, commCombo...)
			rank := evaluateFive(five)
			if bestRank == nil || rank.score > bestRank.score {
				bestRank = rank
			}
		}
	}

	if bestRank == nil {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}
	return bestRank
}

func (r *omahaRules) usesCommunityCards() bool { return true }

type fiveCardDrawRules struct{}

func (r *fiveCardDrawRules) holeCardCount() int { return 5 }

func (r *fiveCardDrawRules) evaluateHand(holeCards, _ []*examples.Card) *handRank {
	if len(holeCards) < 5 {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}
	return evaluateFive(holeCards[:5])
}

func (r *fiveCardDrawRules) usesCommunityCards() bool { return false }

func findBestFive(cards []*examples.Card) *handRank {
	if len(cards) < 5 {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}

	var bestRank *handRank

	combos := combinations(cards, 5)
	for _, combo := range combos {
		rank := evaluateFive(combo)
		if bestRank == nil || rank.score > bestRank.score {
			bestRank = rank
		}
	}

	if bestRank == nil {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}
	return bestRank
}

func evaluateFive(cards []*examples.Card) *handRank {
	if len(cards) != 5 {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}

	rankCounts := make(map[int32]int)
	suitCounts := make(map[int32]int)
	ranks := make([]int32, 5)

	for i, card := range cards {
		rankCounts[int32(card.Rank)]++
		suitCounts[int32(card.Suit)]++
		ranks[i] = int32(card.Rank)
	}

	sort.Slice(ranks, func(i, j int) bool { return ranks[i] > ranks[j] })

	isFlush := false
	for _, count := range suitCounts {
		if count == 5 {
			isFlush = true
			break
		}
	}

	isStraight := checkStraight(ranks)
	isWheel := checkWheel(ranks)

	type rankCount struct {
		rank  int32
		count int
	}
	var counts []rankCount
	for rank, count := range rankCounts {
		counts = append(counts, rankCount{rank, count})
	}
	sort.Slice(counts, func(i, j int) bool {
		if counts[i].count != counts[j].count {
			return counts[i].count > counts[j].count
		}
		return counts[i].rank > counts[j].rank
	})

	countPattern := make([]int, len(counts))
	for i, c := range counts {
		countPattern[i] = c.count
	}

	if isFlush && (isStraight || isWheel) {
		high := ranks[0]
		if isWheel {
			high = 5
		}
		if high == int32(examples.Rank_ACE) && !isWheel {
			return &handRank{
				rankType: examples.HandRankType_ROYAL_FLUSH,
				score:    10_000_000,
			}
		}
		return &handRank{
			rankType: examples.HandRankType_STRAIGHT_FLUSH,
			score:    9_000_000 + high,
			kickers:  []examples.Rank{examples.Rank(high)},
		}
	}

	if len(countPattern) >= 2 && countPattern[0] == 4 && countPattern[1] == 1 {
		quadRank := counts[0].rank
		kicker := counts[1].rank
		return &handRank{
			rankType: examples.HandRankType_FOUR_OF_A_KIND,
			score:    8_000_000 + quadRank*100 + kicker,
			kickers:  []examples.Rank{examples.Rank(quadRank), examples.Rank(kicker)},
		}
	}

	if len(countPattern) >= 2 && countPattern[0] == 3 && countPattern[1] == 2 {
		tripsRank := counts[0].rank
		pairRank := counts[1].rank
		return &handRank{
			rankType: examples.HandRankType_FULL_HOUSE,
			score:    7_000_000 + tripsRank*100 + pairRank,
			kickers:  []examples.Rank{examples.Rank(tripsRank), examples.Rank(pairRank)},
		}
	}

	if isFlush {
		score := rankScore(ranks)
		kickers := make([]examples.Rank, len(ranks))
		for i, r := range ranks {
			kickers[i] = examples.Rank(r)
		}
		return &handRank{
			rankType: examples.HandRankType_FLUSH,
			score:    6_000_000 + score,
			kickers:  kickers,
		}
	}

	if isStraight || isWheel {
		high := ranks[0]
		if isWheel {
			high = 5
		}
		return &handRank{
			rankType: examples.HandRankType_STRAIGHT,
			score:    5_000_000 + high,
			kickers:  []examples.Rank{examples.Rank(high)},
		}
	}

	if len(countPattern) >= 3 && countPattern[0] == 3 && countPattern[1] == 1 && countPattern[2] == 1 {
		tripsRank := counts[0].rank
		kicker1 := counts[1].rank
		kicker2 := counts[2].rank
		kickerScore := kicker1*50 + kicker2
		return &handRank{
			rankType: examples.HandRankType_THREE_OF_A_KIND,
			score:    4_000_000 + tripsRank*1000 + kickerScore,
			kickers:  []examples.Rank{examples.Rank(tripsRank), examples.Rank(kicker1), examples.Rank(kicker2)},
		}
	}

	if len(countPattern) >= 3 && countPattern[0] == 2 && countPattern[1] == 2 && countPattern[2] == 1 {
		highPair := counts[0].rank
		lowPair := counts[1].rank
		kicker := counts[2].rank
		return &handRank{
			rankType: examples.HandRankType_TWO_PAIR,
			score:    3_000_000 + highPair*1000 + lowPair*50 + kicker,
			kickers:  []examples.Rank{examples.Rank(highPair), examples.Rank(lowPair), examples.Rank(kicker)},
		}
	}

	if len(countPattern) >= 4 && countPattern[0] == 2 && countPattern[1] == 1 && countPattern[2] == 1 && countPattern[3] == 1 {
		pairRank := counts[0].rank
		kicker1 := counts[1].rank
		kicker2 := counts[2].rank
		kicker3 := counts[3].rank
		kickerScore := kicker1*1000 + kicker2*50 + kicker3
		return &handRank{
			rankType: examples.HandRankType_PAIR,
			score:    2_000_000 + pairRank*10000 + kickerScore,
			kickers:  []examples.Rank{examples.Rank(pairRank), examples.Rank(kicker1), examples.Rank(kicker2), examples.Rank(kicker3)},
		}
	}

	score := rankScore(ranks)
	kickers := make([]examples.Rank, len(ranks))
	for i, r := range ranks {
		kickers[i] = examples.Rank(r)
	}
	return &handRank{
		rankType: examples.HandRankType_HIGH_CARD,
		score:    1_000_000 + score,
		kickers:  kickers,
	}
}

func checkStraight(ranks []int32) bool {
	if len(ranks) != 5 {
		return false
	}
	for i := 0; i < 4; i++ {
		if ranks[i]-ranks[i+1] != 1 {
			return false
		}
	}
	return true
}

func checkWheel(ranks []int32) bool {
	sorted := make([]int32, len(ranks))
	copy(sorted, ranks)
	sort.Slice(sorted, func(i, j int) bool { return sorted[i] < sorted[j] })

	return len(sorted) == 5 &&
		sorted[0] == 2 &&
		sorted[1] == 3 &&
		sorted[2] == 4 &&
		sorted[3] == 5 &&
		sorted[4] == int32(examples.Rank_ACE)
}

func rankScore(ranks []int32) int32 {
	var score int32
	multiplier := int32(10000)
	for i := 0; i < 5 && i < len(ranks); i++ {
		score += ranks[i] * multiplier
		multiplier /= 15
	}
	return score
}

func combinations(cards []*examples.Card, k int) [][]*examples.Card {
	if k > len(cards) {
		return nil
	}
	if k == 0 {
		return [][]*examples.Card{{}}
	}
	if k == len(cards) {
		result := make([]*examples.Card, len(cards))
		copy(result, cards)
		return [][]*examples.Card{result}
	}

	var result [][]*examples.Card
	var helper func(start int, current []*examples.Card)
	helper = func(start int, current []*examples.Card) {
		if len(current) == k {
			combo := make([]*examples.Card, k)
			copy(combo, current)
			result = append(result, combo)
			return
		}
		for i := start; i < len(cards); i++ {
			helper(i+1, append(current, cards[i]))
		}
	}
	helper(0, nil)
	return result
}
