// Saga: Table → Hand (OO Pattern)
//
// Reacts to HandStarted events from Table domain.
// Sends DealCards commands to Hand domain.
//
// This is the OO-style implementation using SagaBase with method-based
// handlers and fluent registration. Contrasts with saga-hand/ which
// uses the functional EventRouter pattern.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/types/known/anypb"
)

// TableHandSaga translates HandStarted events to DealCards commands.
type TableHandSaga struct {
	angzarr.SagaBase
}

// NewTableHandSaga creates a new TableHandSaga with registered handlers.
func NewTableHandSaga() *TableHandSaga {
	s := &TableHandSaga{}
	s.Init("saga-table-hand", "table", "hand")

	// Register prepare handler
	s.Prepares("HandStarted", s.prepareHandStarted)

	// Register event handler
	s.ReactsTo("HandStarted", s.handleHandStarted)

	return s
}

// prepareHandStarted declares the hand aggregate as destination.
func (s *TableHandSaga) prepareHandStarted(event *examples.HandStarted) []*pb.Cover {
	return []*pb.Cover{
		{
			Domain: "hand",
			Root:   &pb.UUID{Value: event.HandRoot},
		},
	}
}

// handleHandStarted translates HandStarted → DealCards.
func (s *TableHandSaga) handleHandStarted(
	event *examples.HandStarted,
	destinations []*pb.EventBook,
) (*pb.CommandBook, error) {
	// Get next sequence from destination state
	var destSeq uint32
	if len(destinations) > 0 {
		destSeq = angzarr.NextSequence(destinations[0])
	}

	// Convert SeatSnapshot to PlayerInHand
	players := make([]*examples.PlayerInHand, len(event.ActivePlayers))
	for i, seat := range event.ActivePlayers {
		players[i] = &examples.PlayerInHand{
			PlayerRoot: seat.PlayerRoot,
			Position:   seat.Position,
			Stack:      seat.Stack,
		}
	}

	// Build DealCards command
	dealCards := &examples.DealCards{
		TableRoot:      event.HandRoot,
		HandNumber:     event.HandNumber,
		GameVariant:    event.GameVariant,
		Players:        players,
		DealerPosition: event.DealerPosition,
		SmallBlind:     event.SmallBlind,
		BigBlind:       event.BigBlind,
	}

	cmdAny, err := anypb.New(dealCards)
	if err != nil {
		return nil, err
	}

	return &pb.CommandBook{
		Cover: &pb.Cover{
			Domain: "hand",
			Root:   &pb.UUID{Value: event.HandRoot},
		},
		Pages: []*pb.CommandPage{
			{
				Sequence: destSeq,
				Command:  cmdAny,
			},
		},
	}, nil
}

func main() {
	saga := NewTableHandSaga()
	angzarr.RunOOSagaServer("saga-table-hand", "50212", saga)
}
