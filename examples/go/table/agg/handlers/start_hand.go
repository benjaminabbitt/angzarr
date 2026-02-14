package handlers

import (
	"crypto/sha256"
	"encoding/binary"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

func guardStartHand(state TableState) error {
	if !state.Exists() {
		return angzarr.NewCommandRejectedError("Table does not exist")
	}
	if state.Status == "in_hand" {
		return angzarr.NewCommandRejectedError("Hand already in progress")
	}
	if state.ActivePlayerCount() < 2 {
		return angzarr.NewCommandRejectedError("Not enough players to start hand")
	}
	return nil
}

func computeHandStarted(state TableState, tableRoot []byte) *examples.HandStarted {
	handNumber := state.HandCount + 1
	handRoot := generateHandRoot(tableRoot, handNumber)

	dealerPosition := advanceToNextActive(state.DealerPosition, state)
	smallBlindPosition := advanceToNextActive(dealerPosition, state)
	bigBlindPosition := advanceToNextActive(smallBlindPosition, state)

	var activePlayers []*examples.SeatSnapshot
	for _, seat := range state.Seats {
		if !seat.IsSittingOut {
			activePlayers = append(activePlayers, &examples.SeatSnapshot{
				Position:   seat.Position,
				PlayerRoot: seat.PlayerRoot,
				Stack:      seat.Stack,
			})
		}
	}

	return &examples.HandStarted{
		HandRoot:           handRoot,
		HandNumber:         handNumber,
		DealerPosition:     dealerPosition,
		SmallBlindPosition: smallBlindPosition,
		BigBlindPosition:   bigBlindPosition,
		ActivePlayers:      activePlayers,
		GameVariant:        state.GameVariant,
		SmallBlind:         state.SmallBlind,
		BigBlind:           state.BigBlind,
		StartedAt:          timestamppb.New(time.Now()),
	}
}

// HandleStartHand handles the StartHand command.
func HandleStartHand(
	commandBook *pb.CommandBook,
	commandAny *anypb.Any,
	state TableState,
	seq uint32,
) (*pb.EventBook, error) {
	var cmd examples.StartHand
	if err := proto.Unmarshal(commandAny.Value, &cmd); err != nil {
		return nil, err
	}

	if err := guardStartHand(state); err != nil {
		return nil, err
	}

	event := computeHandStarted(state, commandBook.Cover.Root.Value)

	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return angzarr.NewEventBook(commandBook.Cover, seq, eventAny), nil
}

// generateHandRoot creates a deterministic hand root from table root and hand number.
func generateHandRoot(tableRoot []byte, handNumber int64) []byte {
	h := sha256.New()
	h.Write(tableRoot)
	buf := make([]byte, 8)
	binary.BigEndian.PutUint64(buf, uint64(handNumber))
	h.Write(buf)
	return h.Sum(nil)
}

// advanceToNextActive finds the next active (non-sitting-out) player position.
func advanceToNextActive(currentPos int32, state TableState) int32 {
	maxPlayers := state.MaxPlayers
	for i := int32(1); i <= maxPlayers; i++ {
		nextPos := (currentPos + i) % maxPlayers
		if seat, exists := state.Seats[nextPos]; exists && !seat.IsSittingOut {
			return nextPos
		}
	}
	return currentPos // Shouldn't happen if we have active players
}
