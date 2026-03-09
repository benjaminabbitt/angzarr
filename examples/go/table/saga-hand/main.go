// Saga: Table → Hand (OO Pattern)
//
// DOC: This file is referenced in docs/docs/examples/sagas.mdx
//
//	Update documentation when making changes to saga patterns.
//
// Reacts to HandStarted events from Table domain.
// Sends DealCards commands to Hand domain.
//
// Uses the OO-style implementation with SagaBase and method-based
// handlers with fluent registration.
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

// docs:start:saga_oo
// docs:start:saga_handler
// NewTableHandSaga creates a new TableHandSaga with registered handlers.
func NewTableHandSaga() *TableHandSaga {
	s := &TableHandSaga{}
	s.Init("saga-table-hand", "table", "hand")

	// Register event handler
	s.Handles(s.handleHandStarted)

	return s
}

// handleHandStarted translates HandStarted → DealCards.
// Sagas are stateless translators - framework handles sequence stamping.
func (s *TableHandSaga) handleHandStarted(
	event *examples.HandStarted,
) (*pb.CommandBook, error) {
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

	// Use angzarr_deferred - framework stamps sequence on delivery
	return &pb.CommandBook{
		Cover: &pb.Cover{
			Domain: "hand",
			Root:   &pb.UUID{Value: event.HandRoot},
		},
		Pages: []*pb.CommandPage{
			{
				Header:  &pb.PageHeader{SequenceType: &pb.PageHeader_AngzarrDeferred{AngzarrDeferred: &pb.AngzarrDeferredSequence{}}},
				Payload: &pb.CommandPage_Command{Command: cmdAny},
			},
		},
	}, nil
}

// docs:end:saga_handler
// docs:end:saga_oo

// docs:start:event_router
func main() {
	saga := NewTableHandSaga()
	angzarr.RunOOSagaServer("saga-table-hand", "50211", saga)
}

// docs:end:event_router
