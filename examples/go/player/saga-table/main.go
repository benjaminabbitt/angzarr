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

	// Register event handlers
	s.Handles(s.handleSittingOut)
	s.Handles(s.handleReturningToPlay)

	return s
}

// handleSittingOut translates PlayerSittingOut -> PlayerSatOut fact for table.
// Sagas are stateless translators - framework handles sequence stamping.
func (s *PlayerTableSaga) handleSittingOut(
	event *examples.PlayerSittingOut,
) (*pb.CommandBook, error) {
	// Create PlayerSatOut fact for the table
	satOut := &examples.PlayerSatOut{
		PlayerRoot: nil, // Will get player root from context
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
			{Payload: &pb.EventPage_Event{Event: factAny}},
		},
	}

	// Emit as fact instead of command
	s.EmitFact(fact)

	return nil, nil
}

// handleReturningToPlay translates PlayerReturningToPlay -> PlayerSatIn fact for table.
// Sagas are stateless translators - framework handles sequence stamping.
func (s *PlayerTableSaga) handleReturningToPlay(
	event *examples.PlayerReturningToPlay,
) (*pb.CommandBook, error) {
	// Create PlayerSatIn fact for the table
	satIn := &examples.PlayerSatIn{
		PlayerRoot: nil, // Will get player root from context
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
			{Payload: &pb.EventPage_Event{Event: factAny}},
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
