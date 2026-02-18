// Table aggregate - rich domain model using OO pattern.
//
// This aggregate uses the OO-style pattern with embedded AggregateBase,
// method-based handlers, and fluent registration. This contrasts with
// the player aggregate which uses the functional CommandRouter pattern.
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"sort"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/types/known/timestamppb"
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

// Table aggregate with event sourcing using OO pattern.
type Table struct {
	angzarr.AggregateBase[TableState]
}

// NewTable creates a new Table aggregate with prior events for state reconstruction.
func NewTable(eventBook *pb.EventBook) *Table {
	t := &Table{}
	t.Init(eventBook, func() TableState {
		return TableState{Seats: make(map[int32]*SeatState)}
	})
	t.SetDomain("table")

	// Register event appliers
	t.Applies("TableCreated", t.applyTableCreated)
	t.Applies("PlayerJoined", t.applyPlayerJoined)
	t.Applies("PlayerLeft", t.applyPlayerLeft)
	t.Applies("PlayerSatOut", t.applyPlayerSatOut)
	t.Applies("PlayerSatIn", t.applyPlayerSatIn)
	t.Applies("HandStarted", t.applyHandStarted)
	t.Applies("HandEnded", t.applyHandEnded)
	t.Applies("ChipsAdded", t.applyChipsAdded)

	// Register command handlers
	t.Handles("CreateTable", t.create)
	t.Handles("JoinTable", t.join)
	t.Handles("LeaveTable", t.leave)
	t.Handles("StartHand", t.startHand)
	t.Handles("EndHand", t.endHand)

	return t
}

// --- Event Appliers ---

func (t *Table) applyTableCreated(state *TableState, event *examples.TableCreated) {
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

func (t *Table) applyPlayerJoined(state *TableState, event *examples.PlayerJoined) {
	state.Seats[event.SeatPosition] = &SeatState{
		Position:     event.SeatPosition,
		PlayerRoot:   event.PlayerRoot,
		Stack:        event.Stack,
		IsActive:     true,
		IsSittingOut: false,
	}
}

func (t *Table) applyPlayerLeft(state *TableState, event *examples.PlayerLeft) {
	delete(state.Seats, event.SeatPosition)
}

func (t *Table) applyPlayerSatOut(state *TableState, event *examples.PlayerSatOut) {
	pos := t.findSeatByPlayer(state, event.PlayerRoot)
	if pos >= 0 {
		state.Seats[pos].IsSittingOut = true
	}
}

func (t *Table) applyPlayerSatIn(state *TableState, event *examples.PlayerSatIn) {
	pos := t.findSeatByPlayer(state, event.PlayerRoot)
	if pos >= 0 {
		state.Seats[pos].IsSittingOut = false
	}
}

func (t *Table) applyHandStarted(state *TableState, event *examples.HandStarted) {
	state.CurrentHandRoot = event.HandRoot
	state.HandCount = event.HandNumber
	state.DealerPosition = event.DealerPosition
	state.Status = "in_hand"
}

func (t *Table) applyHandEnded(state *TableState, event *examples.HandEnded) {
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

func (t *Table) applyChipsAdded(state *TableState, event *examples.ChipsAdded) {
	pos := t.findSeatByPlayer(t.State(), event.PlayerRoot)
	if pos >= 0 {
		t.State().Seats[pos].Stack = event.NewStack
	}
}

// --- State Accessors ---

func (t *Table) exists() bool {
	return t.State().TableID != ""
}

func (t *Table) playerCount() int {
	return len(t.State().Seats)
}

func (t *Table) activePlayerCount() int {
	count := 0
	for _, seat := range t.State().Seats {
		if !seat.IsSittingOut {
			count++
		}
	}
	return count
}

func (t *Table) findSeatByPlayer(state *TableState, playerRoot []byte) int32 {
	playerHex := hex.EncodeToString(playerRoot)
	for pos, seat := range state.Seats {
		if hex.EncodeToString(seat.PlayerRoot) == playerHex {
			return pos
		}
	}
	return -1
}

func (t *Table) nextAvailableSeat() int32 {
	state := t.State()
	for i := int32(0); i < state.MaxPlayers; i++ {
		if _, exists := state.Seats[i]; !exists {
			return i
		}
	}
	return -1
}

func (t *Table) nextDealerPosition() int32 {
	state := t.State()
	if len(state.Seats) == 0 {
		return 0
	}

	positions := make([]int, 0, len(state.Seats))
	for pos := range state.Seats {
		positions = append(positions, int(pos))
	}
	sort.Ints(positions)

	currentIdx := 0
	for i, pos := range positions {
		if int32(pos) == state.DealerPosition {
			currentIdx = i
			break
		}
	}
	nextIdx := (currentIdx + 1) % len(positions)
	return int32(positions[nextIdx])
}

// --- Command Handlers ---

func (t *Table) create(cmd *examples.CreateTable) (*examples.TableCreated, error) {
	// Guard
	if t.exists() {
		return nil, angzarr.NewCommandRejectedError("Table already exists")
	}

	// Validate
	if cmd.TableName == "" {
		return nil, angzarr.NewCommandRejectedError("table_name is required")
	}
	if cmd.SmallBlind <= 0 {
		return nil, angzarr.NewCommandRejectedError("small_blind must be positive")
	}
	if cmd.BigBlind <= 0 || cmd.BigBlind < cmd.SmallBlind {
		return nil, angzarr.NewCommandRejectedError("big_blind must be >= small_blind")
	}
	if cmd.MinBuyIn <= 0 {
		return nil, angzarr.NewCommandRejectedError("min_buy_in must be positive")
	}
	if cmd.MaxBuyIn < cmd.MinBuyIn {
		return nil, angzarr.NewCommandRejectedError("max_buy_in must be >= min_buy_in")
	}
	if cmd.MaxPlayers < 2 || cmd.MaxPlayers > 10 {
		return nil, angzarr.NewCommandRejectedError("max_players must be 2-10")
	}

	// Compute
	return &examples.TableCreated{
		TableName:            cmd.TableName,
		GameVariant:          cmd.GameVariant,
		SmallBlind:           cmd.SmallBlind,
		BigBlind:             cmd.BigBlind,
		MinBuyIn:             cmd.MinBuyIn,
		MaxBuyIn:             cmd.MaxBuyIn,
		MaxPlayers:           cmd.MaxPlayers,
		ActionTimeoutSeconds: cmd.ActionTimeoutSeconds,
		CreatedAt:            timestamppb.New(time.Now()),
	}, nil
}

func (t *Table) join(cmd *examples.JoinTable) (*examples.PlayerJoined, error) {
	state := t.State()

	// Guard
	if !t.exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}

	// Validate
	if len(cmd.PlayerRoot) == 0 {
		return nil, angzarr.NewCommandRejectedError("player_root is required")
	}
	if t.findSeatByPlayer(state, cmd.PlayerRoot) >= 0 {
		return nil, angzarr.NewCommandRejectedError("Player already seated at table")
	}
	if t.playerCount() >= int(state.MaxPlayers) {
		return nil, angzarr.NewCommandRejectedError("Table is full")
	}
	if cmd.BuyInAmount < state.MinBuyIn {
		return nil, angzarr.NewCommandRejectedError(
			fmt.Sprintf("Buy-in must be at least %d", state.MinBuyIn))
	}
	if cmd.BuyInAmount > state.MaxBuyIn {
		return nil, angzarr.NewCommandRejectedError(
			fmt.Sprintf("Buy-in cannot exceed %d", state.MaxBuyIn))
	}

	// Determine seat position
	seatPos := t.nextAvailableSeat()
	if cmd.PreferredSeat > 0 && cmd.PreferredSeat < state.MaxPlayers {
		if _, occupied := state.Seats[cmd.PreferredSeat]; !occupied {
			seatPos = cmd.PreferredSeat
		}
	}

	// Compute
	return &examples.PlayerJoined{
		PlayerRoot:   cmd.PlayerRoot,
		SeatPosition: seatPos,
		BuyInAmount:  cmd.BuyInAmount,
		Stack:        cmd.BuyInAmount,
		JoinedAt:     timestamppb.New(time.Now()),
	}, nil
}

func (t *Table) leave(cmd *examples.LeaveTable) (*examples.PlayerLeft, error) {
	state := t.State()

	// Guard
	if !t.exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}

	// Validate
	if len(cmd.PlayerRoot) == 0 {
		return nil, angzarr.NewCommandRejectedError("player_root is required")
	}

	pos := t.findSeatByPlayer(state, cmd.PlayerRoot)
	if pos < 0 {
		return nil, angzarr.NewCommandRejectedError("Player is not seated at table")
	}
	if state.Status == "in_hand" {
		return nil, angzarr.NewCommandRejectedError("Cannot leave table during a hand")
	}

	seat := state.Seats[pos]

	// Compute
	return &examples.PlayerLeft{
		PlayerRoot:     cmd.PlayerRoot,
		SeatPosition:   pos,
		ChipsCashedOut: seat.Stack,
		LeftAt:         timestamppb.New(time.Now()),
	}, nil
}

func (t *Table) startHand(cmd *examples.StartHand) (*examples.HandStarted, error) {
	state := t.State()

	// Guard
	if !t.exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}
	if state.Status == "in_hand" {
		return nil, angzarr.NewCommandRejectedError("Hand already in progress")
	}
	if t.activePlayerCount() < 2 {
		return nil, angzarr.NewCommandRejectedError("Not enough players to start hand")
	}

	// Generate hand root (deterministic based on table + hand number)
	handNumber := state.HandCount + 1
	handRootInput := fmt.Sprintf("angzarr.poker.hand.%s.%d", state.TableID, handNumber)
	hash := sha256.Sum256([]byte(handRootInput))
	handRoot := hash[:16] // Use first 16 bytes as UUID-like identifier

	// Advance dealer button
	dealerPosition := t.nextDealerPosition()

	// Get active player positions
	var activePositions []int32
	for pos, seat := range state.Seats {
		if !seat.IsSittingOut {
			activePositions = append(activePositions, pos)
		}
	}
	sort.Slice(activePositions, func(i, j int) bool {
		return activePositions[i] < activePositions[j]
	})

	// Find dealer index in active positions
	dealerIdx := 0
	for i, pos := range activePositions {
		if pos == dealerPosition {
			dealerIdx = i
			break
		}
	}

	// Calculate blind positions
	var sbPosition, bbPosition int32
	numPlayers := len(activePositions)
	if numPlayers == 2 {
		sbPosition = activePositions[dealerIdx]
		bbPosition = activePositions[(dealerIdx+1)%2]
	} else {
		sbPosition = activePositions[(dealerIdx+1)%numPlayers]
		bbPosition = activePositions[(dealerIdx+2)%numPlayers]
	}

	// Build active players list
	var activePlayers []*examples.SeatSnapshot
	for _, pos := range activePositions {
		seat := state.Seats[pos]
		activePlayers = append(activePlayers, &examples.SeatSnapshot{
			Position:   pos,
			PlayerRoot: seat.PlayerRoot,
			Stack:      seat.Stack,
		})
	}

	// Compute
	return &examples.HandStarted{
		HandRoot:           handRoot,
		HandNumber:         handNumber,
		DealerPosition:     dealerPosition,
		SmallBlindPosition: sbPosition,
		BigBlindPosition:   bbPosition,
		GameVariant:        state.GameVariant,
		SmallBlind:         state.SmallBlind,
		BigBlind:           state.BigBlind,
		ActivePlayers:      activePlayers,
		StartedAt:          timestamppb.New(time.Now()),
	}, nil
}

func (t *Table) endHand(cmd *examples.EndHand) (*examples.HandEnded, error) {
	state := t.State()

	// Guard
	if !t.exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}
	if state.Status != "in_hand" {
		return nil, angzarr.NewCommandRejectedError("No hand in progress")
	}
	if hex.EncodeToString(cmd.HandRoot) != hex.EncodeToString(state.CurrentHandRoot) {
		return nil, angzarr.NewCommandRejectedError("Hand root mismatch")
	}

	// Calculate stack changes from results
	stackChanges := make(map[string]int64)
	for _, result := range cmd.Results {
		playerHex := hex.EncodeToString(result.WinnerRoot)
		stackChanges[playerHex] += result.Amount
	}

	// Compute
	return &examples.HandEnded{
		HandRoot:     cmd.HandRoot,
		StackChanges: stackChanges,
		Results:      cmd.Results,
		EndedAt:      timestamppb.New(time.Now()),
	}, nil
}
