// Package handlers implements table aggregate command handlers.
package handlers

import (
	"encoding/hex"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// TableState represents the current state of a table aggregate.
type TableState struct {
	TableID              string
	TableName            string
	GameVariant          examples.GameVariant
	SmallBlind           int64
	BigBlind             int64
	MinBuyIn             int64
	MaxBuyIn             int64
	MaxPlayers           int32
	ActionTimeoutSeconds int32
	Seats                map[int32]*SeatState // position -> seat
	DealerPosition       int32
	HandCount            int64
	CurrentHandRoot      []byte
	Status               string // "waiting", "in_hand", "paused"
}

// SeatState represents a player seat at the table.
type SeatState struct {
	Position     int32
	PlayerRoot   []byte
	Stack        int64
	IsActive     bool
	IsSittingOut bool
}

// NewTableState creates a new empty table state.
func NewTableState() TableState {
	return TableState{
		Seats: make(map[int32]*SeatState),
	}
}

// Exists returns true if the table has been created.
func (s TableState) Exists() bool {
	return s.TableID != ""
}

// PlayerCount returns the number of seated players.
func (s TableState) PlayerCount() int {
	return len(s.Seats)
}

// ActivePlayerCount returns the number of active (not sitting out) players.
func (s TableState) ActivePlayerCount() int {
	count := 0
	for _, seat := range s.Seats {
		if !seat.IsSittingOut {
			count++
		}
	}
	return count
}

// FindSeatByPlayer returns the seat position for a player root, or -1 if not found.
func (s TableState) FindSeatByPlayer(playerRoot []byte) int32 {
	playerHex := hex.EncodeToString(playerRoot)
	for pos, seat := range s.Seats {
		if hex.EncodeToString(seat.PlayerRoot) == playerHex {
			return pos
		}
	}
	return -1
}

// NextAvailableSeat returns the next available seat position, or -1 if full.
func (s TableState) NextAvailableSeat() int32 {
	for i := int32(0); i < s.MaxPlayers; i++ {
		if _, exists := s.Seats[i]; !exists {
			return i
		}
	}
	return -1
}

// RebuildState rebuilds table state from event history.
func RebuildState(eventBook *pb.EventBook) TableState {
	state := NewTableState()

	if eventBook == nil {
		return state
	}

	// Start from snapshot if available
	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		if eventBook.Snapshot.State.MessageIs(&examples.TableState{}) {
			var snapshot examples.TableState
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

func applySnapshot(snapshot *examples.TableState) TableState {
	seats := make(map[int32]*SeatState)
	for _, s := range snapshot.Seats {
		stack := int64(0)
		if s.Stack != nil {
			stack = s.Stack.Amount
		}
		seats[s.Position] = &SeatState{
			Position:     s.Position,
			PlayerRoot:   s.PlayerRoot,
			Stack:        stack,
			IsActive:     s.IsActive,
			IsSittingOut: s.IsSittingOut,
		}
	}

	return TableState{
		TableID:              snapshot.TableId,
		TableName:            snapshot.TableName,
		GameVariant:          snapshot.GameVariant,
		SmallBlind:           snapshot.SmallBlind,
		BigBlind:             snapshot.BigBlind,
		MinBuyIn:             snapshot.MinBuyIn,
		MaxBuyIn:             snapshot.MaxBuyIn,
		MaxPlayers:           snapshot.MaxPlayers,
		ActionTimeoutSeconds: snapshot.ActionTimeoutSeconds,
		Seats:                seats,
		DealerPosition:       snapshot.DealerPosition,
		HandCount:            snapshot.HandCount,
		CurrentHandRoot:      snapshot.CurrentHandRoot,
		Status:               snapshot.Status,
	}
}

func applyEvent(state *TableState, eventAny *anypb.Any) {
	typeURL := eventAny.TypeUrl

	switch {
	case strings.HasSuffix(typeURL, "TableCreated"):
		var event examples.TableCreated
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			state.TableID = "table_" + event.TableName
			state.TableName = event.TableName
			state.GameVariant = event.GameVariant
			state.SmallBlind = event.SmallBlind
			state.BigBlind = event.BigBlind
			state.MinBuyIn = event.MinBuyIn
			state.MaxBuyIn = event.MaxBuyIn
			state.MaxPlayers = event.MaxPlayers
			state.ActionTimeoutSeconds = event.ActionTimeoutSeconds
			state.DealerPosition = 0
			state.HandCount = 0
			state.Status = "waiting"
		}

	case strings.HasSuffix(typeURL, "PlayerJoined"):
		var event examples.PlayerJoined
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			state.Seats[event.SeatPosition] = &SeatState{
				Position:     event.SeatPosition,
				PlayerRoot:   event.PlayerRoot,
				Stack:        event.Stack,
				IsActive:     true,
				IsSittingOut: false,
			}
		}

	case strings.HasSuffix(typeURL, "PlayerLeft"):
		var event examples.PlayerLeft
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			delete(state.Seats, event.SeatPosition)
		}

	case strings.HasSuffix(typeURL, "PlayerSatOut"):
		var event examples.PlayerSatOut
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			pos := state.FindSeatByPlayer(event.PlayerRoot)
			if pos >= 0 {
				state.Seats[pos].IsSittingOut = true
			}
		}

	case strings.HasSuffix(typeURL, "PlayerSatIn"):
		var event examples.PlayerSatIn
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			pos := state.FindSeatByPlayer(event.PlayerRoot)
			if pos >= 0 {
				state.Seats[pos].IsSittingOut = false
			}
		}

	case strings.HasSuffix(typeURL, "HandStarted"):
		var event examples.HandStarted
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			state.CurrentHandRoot = event.HandRoot
			state.HandCount = event.HandNumber
			state.DealerPosition = event.DealerPosition
			state.Status = "in_hand"
		}

	case strings.HasSuffix(typeURL, "HandEnded"):
		var event examples.HandEnded
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			state.CurrentHandRoot = nil
			state.Status = "waiting"
			// Apply stack changes
			for playerHex, delta := range event.StackChanges {
				for _, seat := range state.Seats {
					if hex.EncodeToString(seat.PlayerRoot) == playerHex {
						seat.Stack += delta
						break
					}
				}
			}
		}

	case strings.HasSuffix(typeURL, "ChipsAdded"):
		var event examples.ChipsAdded
		if err := proto.Unmarshal(eventAny.Value, &event); err == nil {
			pos := state.FindSeatByPlayer(event.PlayerRoot)
			if pos >= 0 {
				state.Seats[pos].Stack = event.NewStack
			}
		}
	}
}
