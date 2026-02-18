// Process Manager: Hand Flow (OO Pattern)
//
// Orchestrates the flow of poker hands by:
// 1. Subscribing to table and hand domain events
// 2. Managing hand process state machines
// 3. Sending commands to drive hands forward
//
// This example demonstrates the OO pattern using:
// - ProcessManagerBase with generic state
// - Prepares() for destination declaration
// - Handles() for event processing
// - Applies() for state reconstruction (optional)
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
)

// PMState is the PM's aggregate state (rebuilt from its own events).
// For simplicity in this example, we use a minimal state.
type PMState struct {
	HandRoot       []byte
	HandInProgress bool
}

// HandFlowPM is the OO-style process manager for hand flow orchestration.
type HandFlowPM struct {
	angzarr.ProcessManagerBase[*PMState]
}

// NewHandFlowPM creates a new HandFlowPM with all handlers registered.
func NewHandFlowPM() *HandFlowPM {
	pm := &HandFlowPM{}
	pm.Init("hand-flow", "hand-flow", []string{"table", "hand"})
	pm.WithStateFactory(func() *PMState { return &PMState{} })

	// Register prepare handlers
	pm.Prepares("HandStarted", pm.prepareHandStarted)

	// Register event handlers
	pm.Handles("HandStarted", pm.handleHandStarted)
	pm.Handles("CardsDealt", pm.handleCardsDealt)
	pm.Handles("BlindPosted", pm.handleBlindPosted)
	pm.Handles("ActionTaken", pm.handleActionTaken)
	pm.Handles("CommunityCardsDealt", pm.handleCommunityDealt)
	pm.Handles("PotAwarded", pm.handlePotAwarded)

	return pm
}

// prepareHandStarted declares the hand destination needed when a hand starts.
func (pm *HandFlowPM) prepareHandStarted(
	trigger *pb.EventBook,
	state *PMState,
	event *examples.HandStarted,
) []*pb.Cover {
	return []*pb.Cover{{
		Domain: "hand",
		Root:   &pb.UUID{Value: event.HandRoot},
	}}
}

// handleHandStarted processes the HandStarted event.
func (pm *HandFlowPM) handleHandStarted(
	trigger *pb.EventBook,
	state *PMState,
	event *examples.HandStarted,
	dests []*pb.EventBook,
) ([]*pb.CommandBook, *pb.EventBook, error) {
	// Initialize hand process (not persisted in this simplified version).
	// The saga-table-hand will send DealCards, so we don't emit commands here.
	return nil, nil, nil
}

// handleCardsDealt processes the CardsDealt event.
func (pm *HandFlowPM) handleCardsDealt(
	trigger *pb.EventBook,
	state *PMState,
	event *examples.CardsDealt,
	dests []*pb.EventBook,
) ([]*pb.CommandBook, *pb.EventBook, error) {
	// Post small blind command.
	// In a real implementation, we'd track state to know which blind to post.
	// For now, we assume the hand aggregate tracks this.
	return nil, nil, nil
}

// handleBlindPosted processes the BlindPosted event.
func (pm *HandFlowPM) handleBlindPosted(
	trigger *pb.EventBook,
	state *PMState,
	event *examples.BlindPosted,
	dests []*pb.EventBook,
) ([]*pb.CommandBook, *pb.EventBook, error) {
	// In a full implementation, we'd check if both blinds are posted
	// and then start the betting round.
	return nil, nil, nil
}

// handleActionTaken processes the ActionTaken event.
func (pm *HandFlowPM) handleActionTaken(
	trigger *pb.EventBook,
	state *PMState,
	event *examples.ActionTaken,
	dests []*pb.EventBook,
) ([]*pb.CommandBook, *pb.EventBook, error) {
	// In a full implementation, we'd check if betting is complete
	// and advance to the next phase.
	return nil, nil, nil
}

// handleCommunityDealt processes the CommunityCardsDealt event.
func (pm *HandFlowPM) handleCommunityDealt(
	trigger *pb.EventBook,
	state *PMState,
	event *examples.CommunityCardsDealt,
	dests []*pb.EventBook,
) ([]*pb.CommandBook, *pb.EventBook, error) {
	// Start new betting round after community cards.
	return nil, nil, nil
}

// handlePotAwarded processes the PotAwarded event.
func (pm *HandFlowPM) handlePotAwarded(
	trigger *pb.EventBook,
	state *PMState,
	event *examples.PotAwarded,
	dests []*pb.EventBook,
) ([]*pb.CommandBook, *pb.EventBook, error) {
	// Hand is complete. Clean up.
	return nil, nil, nil
}

func main() {
	pm := NewHandFlowPM()
	angzarr.RunOOProcessManagerServer("hand-flow", "50292", pm)
}
