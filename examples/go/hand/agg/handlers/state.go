// Package handlers implements hand aggregate state reconstruction.
package handlers

import (
	"encoding/hex"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
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

// NewHandState creates a new empty hand state.
func NewHandState() HandState {
	return HandState{
		Players: make(map[string]*PlayerHandState),
		Pots:    []*PotState{{PotType: "main"}},
	}
}

// Exists returns true if the hand has been dealt.
func (s HandState) Exists() bool {
	return s.HandID != ""
}

// IsComplete returns true if the hand is complete.
func (s HandState) IsComplete() bool {
	return s.Status == "complete"
}

// TotalPot returns the sum of all pots.
func (s HandState) TotalPot() int64 {
	total := int64(0)
	for _, pot := range s.Pots {
		total += pot.Amount
	}
	return total
}

// GetPlayerByRoot returns the player state for a given player root.
func (s HandState) GetPlayerByRoot(root []byte) *PlayerHandState {
	return s.Players[hex.EncodeToString(root)]
}

// ActivePlayerCount returns the number of players who haven't folded.
func (s HandState) ActivePlayerCount() int {
	count := 0
	for _, p := range s.Players {
		if !p.HasFolded {
			count++
		}
	}
	return count
}

// Event applier functions for StateRouter

func applyCardsDealt(state *HandState, event *examples.CardsDealt) {
	state.HandID = hex.EncodeToString(event.TableRoot) + "_" + string(rune(event.HandNumber))
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

func applyBlindPosted(state *HandState, event *examples.BlindPosted) {
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

func applyActionTaken(state *HandState, event *examples.ActionTaken) {
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

func applyBettingRoundComplete(state *HandState, event *examples.BettingRoundComplete) {
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

func applyCommunityCardsDealt(state *HandState, event *examples.CommunityCardsDealt) {
	state.CommunityCards = event.AllCommunityCards
	state.CurrentPhase = event.Phase
}

func applyDrawCompleted(state *HandState, event *examples.DrawCompleted) {
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

func applyShowdownStarted(state *HandState, _ *examples.ShowdownStarted) {
	state.Status = "showdown"
}

func applyCardsRevealed(state *HandState, _ *examples.CardsRevealed) {
	// Cards revealed during showdown - could store revealed hands
}

func applyCardsMucked(state *HandState, _ *examples.CardsMucked) {
	// Player mucked - could mark as mucked
}

func applyPotAwarded(state *HandState, event *examples.PotAwarded) {
	for _, winner := range event.Winners {
		key := hex.EncodeToString(winner.PlayerRoot)
		if player := state.Players[key]; player != nil {
			player.Stack += winner.Amount
		}
	}
}

func applyHandComplete(state *HandState, event *examples.HandComplete) {
	state.Status = "complete"
	// Update final stacks
	for _, snap := range event.FinalStacks {
		key := hex.EncodeToString(snap.PlayerRoot)
		if player := state.Players[key]; player != nil {
			player.Stack = snap.Stack
		}
	}
}

// stateRouter is the fluent state reconstruction router.
var stateRouter = angzarr.NewStateRouter(NewHandState).
	On(applyCardsDealt).
	On(applyBlindPosted).
	On(applyActionTaken).
	On(applyBettingRoundComplete).
	On(applyCommunityCardsDealt).
	On(applyDrawCompleted).
	On(applyShowdownStarted).
	On(applyCardsRevealed).
	On(applyCardsMucked).
	On(applyPotAwarded).
	On(applyHandComplete)

// RebuildState rebuilds hand state from event history.
func RebuildState(eventBook *pb.EventBook) HandState {
	if eventBook == nil {
		return NewHandState()
	}

	state := NewHandState()
	for _, page := range eventBook.Pages {
		event := page.GetEvent()
		if event != nil {
			stateRouter.ApplySingle(&state, event)
		}
	}
	return state
}
