// Package handlers implements table aggregate command handlers for testing.
//
// These functional handlers mirror the OO handlers in the main package,
// enabling unit testing without importing the main package.
package handlers

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"sort"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleCreateTable handles the CreateTable command.
func HandleCreateTable(_ *pb.EventBook, cmdAny *anypb.Any, state TableState) (*anypb.Any, error) {
	var cmd examples.CreateTable
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if state.Exists() {
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
	event := &examples.TableCreated{
		TableName:            cmd.TableName,
		GameVariant:          cmd.GameVariant,
		SmallBlind:           cmd.SmallBlind,
		BigBlind:             cmd.BigBlind,
		MinBuyIn:             cmd.MinBuyIn,
		MaxBuyIn:             cmd.MaxBuyIn,
		MaxPlayers:           cmd.MaxPlayers,
		ActionTimeoutSeconds: cmd.ActionTimeoutSeconds,
		CreatedAt:            timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandleJoinTable handles the JoinTable command.
func HandleJoinTable(_ *pb.EventBook, cmdAny *anypb.Any, state TableState) (*anypb.Any, error) {
	var cmd examples.JoinTable
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}

	// Validate
	if len(cmd.PlayerRoot) == 0 {
		return nil, angzarr.NewCommandRejectedError("player_root is required")
	}
	if findSeatByPlayer(state, cmd.PlayerRoot) >= 0 {
		return nil, angzarr.NewCommandRejectedError("Player already seated at table")
	}
	if state.PlayerCount() >= int(state.MaxPlayers) {
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
	seatPos := int32(-1)
	if cmd.PreferredSeat >= 0 && cmd.PreferredSeat < state.MaxPlayers {
		// Specific seat requested
		if _, occupied := state.Seats[cmd.PreferredSeat]; occupied {
			return nil, angzarr.NewCommandRejectedError("Seat is occupied")
		}
		seatPos = cmd.PreferredSeat
	} else {
		// No preference, find any available seat
		seatPos = nextAvailableSeat(state)
	}

	// Compute
	event := &examples.PlayerJoined{
		PlayerRoot:   cmd.PlayerRoot,
		SeatPosition: seatPos,
		BuyInAmount:  cmd.BuyInAmount,
		Stack:        cmd.BuyInAmount,
		JoinedAt:     timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandleLeaveTable handles the LeaveTable command.
func HandleLeaveTable(_ *pb.EventBook, cmdAny *anypb.Any, state TableState) (*anypb.Any, error) {
	var cmd examples.LeaveTable
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}

	// Validate
	if len(cmd.PlayerRoot) == 0 {
		return nil, angzarr.NewCommandRejectedError("player_root is required")
	}

	pos := findSeatByPlayer(state, cmd.PlayerRoot)
	if pos < 0 {
		return nil, angzarr.NewCommandRejectedError("Player is not seated at table")
	}
	if state.Status == "in_hand" {
		return nil, angzarr.NewCommandRejectedError("Cannot leave table during a hand")
	}

	seat := state.Seats[pos]

	// Compute
	event := &examples.PlayerLeft{
		PlayerRoot:     cmd.PlayerRoot,
		SeatPosition:   pos,
		ChipsCashedOut: seat.Stack,
		LeftAt:         timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandleStartHand handles the StartHand command.
func HandleStartHand(_ *pb.EventBook, _ *anypb.Any, state TableState) (*anypb.Any, error) {
	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Table does not exist")
	}
	if state.Status == "in_hand" {
		return nil, angzarr.NewCommandRejectedError("Hand already in progress")
	}
	if state.ActivePlayerCount() < 2 {
		return nil, angzarr.NewCommandRejectedError("Not enough players to start hand")
	}

	// Generate hand root (deterministic based on table + hand number)
	handNumber := state.HandCount + 1
	handRootInput := fmt.Sprintf("angzarr.poker.hand.%s.%d", state.TableID, handNumber)
	hash := sha256.Sum256([]byte(handRootInput))
	handRoot := hash[:16] // Use first 16 bytes as UUID-like identifier

	// Advance dealer button
	dealerPosition := nextDealerPosition(state)

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
	event := &examples.HandStarted{
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
	}

	return anypb.New(event)
}

// HandleEndHand handles the EndHand command.
func HandleEndHand(_ *pb.EventBook, cmdAny *anypb.Any, state TableState) (*anypb.Any, error) {
	var cmd examples.EndHand
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
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
	event := &examples.HandEnded{
		HandRoot:     cmd.HandRoot,
		StackChanges: stackChanges,
		Results:      cmd.Results,
		EndedAt:      timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// Helper functions

func findSeatByPlayer(state TableState, playerRoot []byte) int32 {
	playerHex := hex.EncodeToString(playerRoot)
	for pos, seat := range state.Seats {
		if hex.EncodeToString(seat.PlayerRoot) == playerHex {
			return pos
		}
	}
	return -1
}

func nextAvailableSeat(state TableState) int32 {
	for i := int32(0); i < state.MaxPlayers; i++ {
		if _, exists := state.Seats[i]; !exists {
			return i
		}
	}
	return -1
}

func nextDealerPosition(state TableState) int32 {
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
