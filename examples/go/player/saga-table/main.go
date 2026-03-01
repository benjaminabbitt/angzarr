// Saga: Player -> Table (OO Pattern)
//
// Propagates player sit-out/sit-in intent as facts to the table domain.
// Player events trigger corresponding facts in the table aggregate.
//
// Flow:
// - PlayerSittingOut -> PlayerSatOut fact to table
// - PlayerReturningToPlay -> PlayerSatIn fact to table
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

// PlayerTableSaga propagates player sit-out/sit-in as facts to tables.
type PlayerTableSaga struct {
	angzarr.SagaBase
}

// NewPlayerTableSaga creates a new PlayerTableSaga with registered handlers.
func NewPlayerTableSaga() *PlayerTableSaga {
	s := &PlayerTableSaga{}
	s.Init("saga-player-table", "player", "table")

	// Register prepare handlers (no destinations needed for facts)
	s.Prepares(s.prepareSittingOut)
	s.Prepares(s.prepareReturningToPlay)

	// Register event handlers
	s.Handles(s.handleSittingOut)
	s.Handles(s.handleReturningToPlay)

	return s
}

// prepareSittingOut - no destinations needed (emits facts, not commands).
func (s *PlayerTableSaga) prepareSittingOut(event *examples.PlayerSittingOut) []*pb.Cover {
	return nil
}

// prepareReturningToPlay - no destinations needed (emits facts, not commands).
func (s *PlayerTableSaga) prepareReturningToPlay(event *examples.PlayerReturningToPlay) []*pb.Cover {
	return nil
}

// handleSittingOut translates PlayerSittingOut -> PlayerSatOut fact for table.
func (s *PlayerTableSaga) handleSittingOut(
	event *examples.PlayerSittingOut,
	destinations []*pb.EventBook,
) (*pb.CommandBook, error) {
	// Create PlayerSatOut fact for the table
	satOut := &examples.PlayerSatOut{
		PlayerRoot: nil, // Will get player root from context in execute
		SatOutAt:   event.SatOutAt,
	}

	factAny, err := anypb.New(satOut)
	if err != nil {
		return nil, err
	}

	fact := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "table",
			Root:   &pb.UUID{Value: event.TableRoot},
		},
		Pages: []*pb.EventPage{
			{Event: factAny},
		},
	}

	// Emit as fact instead of command
	s.EmitFact(fact)

	return nil, nil
}

// handleReturningToPlay translates PlayerReturningToPlay -> PlayerSatIn fact for table.
func (s *PlayerTableSaga) handleReturningToPlay(
	event *examples.PlayerReturningToPlay,
	destinations []*pb.EventBook,
) (*pb.CommandBook, error) {
	// Create PlayerSatIn fact for the table
	satIn := &examples.PlayerSatIn{
		PlayerRoot: nil, // Will get player root from context in execute
		SatInAt:    event.SatInAt,
	}

	factAny, err := anypb.New(satIn)
	if err != nil {
		return nil, err
	}

	fact := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "table",
			Root:   &pb.UUID{Value: event.TableRoot},
		},
		Pages: []*pb.EventPage{
			{Event: factAny},
		},
	}

	// Emit as fact instead of command
	s.EmitFact(fact)

	return nil, nil
}

func main() {
	saga := NewPlayerTableSaga()
	angzarr.RunOOSagaServer("saga-player-table", "50214", saga)
}
