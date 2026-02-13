// Package handlers implements player aggregate command handlers.
package handlers

import (
	"encoding/hex"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
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

// RebuildState rebuilds player state from event history.
func RebuildState(eventBook *pb.EventBook) PlayerState {
	state := NewPlayerState()

	if eventBook == nil {
		return state
	}

	// Start from snapshot if available
	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		if eventBook.Snapshot.State.MessageIs(&examples.PlayerState{}) {
			var snapshot examples.PlayerState
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

func applyEvent(state *PlayerState, eventAny *anypb.Any) {
	typeURL := eventAny.TypeUrl

	switch {
	case strings.HasSuffix(typeURL, "PlayerRegistered"):
		var event examples.PlayerRegistered
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			state.PlayerID = "player_" + event.Email
			state.DisplayName = event.DisplayName
			state.Email = event.Email
			state.PlayerType = event.PlayerType
			state.AIModelID = event.AiModelId
			state.Status = "active"
			state.Bankroll = 0
			state.ReservedFunds = 0
		}

	case strings.HasSuffix(typeURL, "FundsDeposited"):
		var event examples.FundsDeposited
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			if event.NewBalance != nil {
				state.Bankroll = event.NewBalance.Amount
			}
		}

	case strings.HasSuffix(typeURL, "FundsWithdrawn"):
		var event examples.FundsWithdrawn
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			if event.NewBalance != nil {
				state.Bankroll = event.NewBalance.Amount
			}
		}

	case strings.HasSuffix(typeURL, "FundsReserved"):
		var event examples.FundsReserved
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			if event.NewReservedBalance != nil {
				state.ReservedFunds = event.NewReservedBalance.Amount
			}
			if event.TableRoot != nil && event.Amount != nil {
				tableKey := hex.EncodeToString(event.TableRoot)
				state.TableReservations[tableKey] = event.Amount.Amount
			}
		}

	case strings.HasSuffix(typeURL, "FundsReleased"):
		var event examples.FundsReleased
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			if event.NewReservedBalance != nil {
				state.ReservedFunds = event.NewReservedBalance.Amount
			}
			if event.TableRoot != nil {
				tableKey := hex.EncodeToString(event.TableRoot)
				delete(state.TableReservations, tableKey)
			}
		}

	case strings.HasSuffix(typeURL, "FundsTransferred"):
		var event examples.FundsTransferred
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			if event.NewBalance != nil {
				state.Bankroll = event.NewBalance.Amount
			}
		}
	}
}
