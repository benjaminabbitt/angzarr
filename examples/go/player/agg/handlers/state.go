// Package handlers implements player aggregate command handlers.
//
// DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//      Update documentation when making changes to StateRouter patterns.
package handlers

import (
	"encoding/hex"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
)

// PlayerState represents the current state of a player aggregate.
type PlayerState struct {
	PlayerID          string
	DisplayName       string
	Email             string
	PlayerType        examples.PlayerType
	AIModelID         string
	Bankroll          int64 // In smallest unit (chips)
	ReservedFunds     int64
	TableReservations map[string]int64 // table_root_hex -> amount
	Status            string
}

// NewPlayerState creates a new empty player state.
func NewPlayerState() PlayerState {
	return PlayerState{
		TableReservations: make(map[string]int64),
	}
}

// Exists returns true if the player has been registered.
func (s PlayerState) Exists() bool {
	return s.PlayerID != ""
}

// AvailableBalance returns the available balance (bankroll - reserved).
func (s PlayerState) AvailableBalance() int64 {
	return s.Bankroll - s.ReservedFunds
}

// IsAI returns true if this is an AI player.
func (s PlayerState) IsAI() bool {
	return s.PlayerType == examples.PlayerType_AI
}

// Event applier functions for StateRouter

// docs:start:state_router
func applyRegistered(state *PlayerState, event *examples.PlayerRegistered) {
	state.PlayerID = "player_" + event.Email
	state.DisplayName = event.DisplayName
	state.Email = event.Email
	state.PlayerType = event.PlayerType
	state.AIModelID = event.AiModelId
	state.Status = "active"
	state.Bankroll = 0
	state.ReservedFunds = 0
}

func applyDeposited(state *PlayerState, event *examples.FundsDeposited) {
	if event.NewBalance != nil {
		state.Bankroll = event.NewBalance.Amount
	}
}

func applyWithdrawn(state *PlayerState, event *examples.FundsWithdrawn) {
	if event.NewBalance != nil {
		state.Bankroll = event.NewBalance.Amount
	}
}

func applyReserved(state *PlayerState, event *examples.FundsReserved) {
	if event.NewReservedBalance != nil {
		state.ReservedFunds = event.NewReservedBalance.Amount
	}
	if event.TableRoot != nil && event.Amount != nil {
		tableKey := hex.EncodeToString(event.TableRoot)
		state.TableReservations[tableKey] = event.Amount.Amount
	}
}

func applyReleased(state *PlayerState, event *examples.FundsReleased) {
	if event.NewReservedBalance != nil {
		state.ReservedFunds = event.NewReservedBalance.Amount
	}
	if event.TableRoot != nil {
		tableKey := hex.EncodeToString(event.TableRoot)
		delete(state.TableReservations, tableKey)
	}
}

func applyTransferred(state *PlayerState, event *examples.FundsTransferred) {
	if event.NewBalance != nil {
		state.Bankroll = event.NewBalance.Amount
	}
}

// stateRouter is the fluent state reconstruction router.
var stateRouter = angzarr.NewStateRouter(NewPlayerState).
	On(applyRegistered).
	On(applyDeposited).
	On(applyWithdrawn).
	On(applyReserved).
	On(applyReleased).
	On(applyTransferred)

// docs:end:state_router

// RebuildState rebuilds player state from event history.
func RebuildState(eventBook *pb.EventBook) PlayerState {
	if eventBook == nil {
		return NewPlayerState()
	}

	// Start from snapshot if available
	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		if eventBook.Snapshot.State.MessageIs(&examples.PlayerState{}) {
			var snapshot examples.PlayerState
			if err := eventBook.Snapshot.State.UnmarshalTo(&snapshot); err == nil {
				state := applySnapshot(&snapshot)
				// Apply events since snapshot
				for _, page := range eventBook.Pages {
					if page.Event != nil {
						stateRouter.ApplySingle(&state, page.Event)
					}
				}
				return state
			}
		}
	}

	return stateRouter.WithEventBook(eventBook)
}

func applySnapshot(snapshot *examples.PlayerState) PlayerState {
	bankroll := int64(0)
	if snapshot.Bankroll != nil {
		bankroll = snapshot.Bankroll.Amount
	}
	reservedFunds := int64(0)
	if snapshot.ReservedFunds != nil {
		reservedFunds = snapshot.ReservedFunds.Amount
	}

	reservations := make(map[string]int64)
	for k, v := range snapshot.TableReservations {
		reservations[k] = v
	}

	return PlayerState{
		PlayerID:          snapshot.PlayerId,
		DisplayName:       snapshot.DisplayName,
		Email:             snapshot.Email,
		PlayerType:        snapshot.PlayerType,
		AIModelID:         snapshot.AiModelId,
		Bankroll:          bankroll,
		ReservedFunds:     reservedFunds,
		TableReservations: reservations,
		Status:            snapshot.Status,
	}
}
