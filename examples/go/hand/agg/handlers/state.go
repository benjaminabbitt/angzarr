// Package handlers implements hand aggregate command handlers.
package handlers

import (
	"encoding/hex"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
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

// ActivePlayerCount returns number of players still in the hand.
func (s HandState) ActivePlayerCount() int {
	count := 0
	for _, p := range s.Players {
		if !p.HasFolded {
			count++
		}
	}
	return count
}

// GetPlayerByPosition finds a player by seat position.
func (s HandState) GetPlayerByPosition(pos int32) *PlayerHandState {
	for _, p := range s.Players {
		if p.Position == pos {
			return p
		}
	}
	return nil
}

// GetPlayerByRoot finds a player by root.
func (s HandState) GetPlayerByRoot(root []byte) *PlayerHandState {
	return s.Players[hex.EncodeToString(root)]
}

// TotalPot returns the total amount in all pots.
func (s HandState) TotalPot() int64 {
	total := int64(0)
	for _, pot := range s.Pots {
		total += pot.Amount
	}
	return total
}

// RebuildState rebuilds hand state from event history.
func RebuildState(eventBook *pb.EventBook) HandState {
	state := NewHandState()

	if eventBook == nil {
		return state
	}

	// Start from snapshot if available
	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		if eventBook.Snapshot.State.MessageIs(&examples.HandState{}) {
			var snapshot examples.HandState
			if err := eventBook.Snapshot.State.UnmarshalTo(&snapshot); err == nil {
				state = applySnapshot(&snapshot)
			}
		}
	}

	// Apply events since snapshot
	for _, page := range eventBook.Pages {
		if page.Event != nil {
			applyEvent(&state, page.Event)
		}
	}

	return state
}

func applySnapshot(snapshot *examples.HandState) HandState {
	players := make(map[string]*PlayerHandState)
	for _, p := range snapshot.Players {
		key := hex.EncodeToString(p.PlayerRoot)
		players[key] = &PlayerHandState{
			PlayerRoot:    p.PlayerRoot,
			Position:      p.Position,
			HoleCards:     p.HoleCards,
			Stack:         p.Stack,
			BetThisRound:  p.BetThisRound,
			TotalInvested: p.TotalInvested,
			HasActed:      p.HasActed,
			HasFolded:     p.HasFolded,
			IsAllIn:       p.IsAllIn,
		}
	}

	var pots []*PotState
	for _, pot := range snapshot.Pots {
		pots = append(pots, &PotState{
			Amount:          pot.Amount,
			EligiblePlayers: pot.EligiblePlayers,
			PotType:         pot.PotType,
		})
	}
	if len(pots) == 0 {
		pots = []*PotState{{PotType: "main"}}
	}

	return HandState{
		HandID:             snapshot.HandId,
		TableRoot:          snapshot.TableRoot,
		HandNumber:         snapshot.HandNumber,
		GameVariant:        snapshot.GameVariant,
		RemainingDeck:      snapshot.RemainingDeck,
		Players:            players,
		CommunityCards:     snapshot.CommunityCards,
		CurrentPhase:       snapshot.CurrentPhase,
		ActionOnPosition:   snapshot.ActionOnPosition,
		CurrentBet:         snapshot.CurrentBet,
		MinRaise:           snapshot.MinRaise,
		Pots:               pots,
		DealerPosition:     snapshot.DealerPosition,
		SmallBlindPosition: snapshot.SmallBlindPosition,
		BigBlindPosition:   snapshot.BigBlindPosition,
		Status:             snapshot.Status,
	}
}

func applyEvent(state *HandState, eventAny *anypb.Any) {
	typeURL := eventAny.TypeUrl

	switch {
	case strings.HasSuffix(typeURL, ".CardsDealt"):
		var event examples.CardsDealt
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			state.HandID = hex.EncodeToString(state.TableRoot) + "_" + string(rune(event.HandNumber))
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

	case strings.HasSuffix(typeURL, "BlindPosted"):
		var event examples.BlindPosted
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
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

	case strings.HasSuffix(typeURL, "ActionTaken"):
		var event examples.ActionTaken
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
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

	case strings.HasSuffix(typeURL, "BettingRoundComplete"):
		var event examples.BettingRoundComplete
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
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

	case strings.HasSuffix(typeURL, "CommunityCardsDealt"):
		var event examples.CommunityCardsDealt
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			state.CommunityCards = event.AllCommunityCards
			state.CurrentPhase = event.Phase
		}

	case strings.HasSuffix(typeURL, "DrawCompleted"):
		var event examples.DrawCompleted
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			key := hex.EncodeToString(event.PlayerRoot)
			if player := state.Players[key]; player != nil {
				// Replace discarded cards with new cards
				// The event contains the new cards; we need to update hole cards
				// For simplicity, just append new cards (in real impl would replace specific indices)
				if len(event.NewCards) > 0 {
					// Create new hole cards array keeping non-discarded and adding new
					player.HoleCards = append(player.HoleCards[:len(player.HoleCards)-int(event.CardsDiscarded)], event.NewCards...)
				}
			}
			// Update remaining deck
			if int(event.CardsDrawn) <= len(state.RemainingDeck) {
				state.RemainingDeck = state.RemainingDeck[event.CardsDrawn:]
			}
		}

	case strings.HasSuffix(typeURL, "ShowdownStarted"):
		state.Status = "showdown"

	case strings.HasSuffix(typeURL, "CardsRevealed"):
		// Cards revealed during showdown - could store revealed hands

	case strings.HasSuffix(typeURL, "CardsMucked"):
		// Player mucked - could mark as mucked

	case strings.HasSuffix(typeURL, "PotAwarded"):
		var event examples.PotAwarded
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			for _, winner := range event.Winners {
				key := hex.EncodeToString(winner.PlayerRoot)
				if player := state.Players[key]; player != nil {
					player.Stack += winner.Amount
				}
			}
		}

	case strings.HasSuffix(typeURL, "HandComplete"):
		var event examples.HandComplete
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			state.Status = "complete"
			// Update final stacks
			for _, snap := range event.FinalStacks {
				key := hex.EncodeToString(snap.PlayerRoot)
				if player := state.Players[key]; player != nil {
					player.Stack = snap.Stack
				}
			}
		}
	}
}
