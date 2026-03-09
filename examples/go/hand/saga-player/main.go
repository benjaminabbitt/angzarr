// Saga: Hand → Player (OO Pattern)
//
// Reacts to PotAwarded events from Hand domain.
// Sends DepositFunds commands to Player domain.
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

// HandPlayerSaga translates PotAwarded events to DepositFunds commands.
type HandPlayerSaga struct {
	angzarr.SagaBase
}

// NewHandPlayerSaga creates a new HandPlayerSaga with registered handlers.
func NewHandPlayerSaga() *HandPlayerSaga {
	s := &HandPlayerSaga{}
	s.Init("saga-hand-player", "hand", "player")

	// Register event handler (multi-command)
	s.HandlesMulti(s.handlePotAwarded)

	return s
}

// handlePotAwarded translates PotAwarded → DepositFunds for each winner.
// Sagas are stateless translators - framework handles sequence stamping.
func (s *HandPlayerSaga) handlePotAwarded(
	event *examples.PotAwarded,
) ([]*pb.CommandBook, error) {
	var commands []*pb.CommandBook

	// Create DepositFunds commands for each winner
	for _, winner := range event.Winners {
		depositFunds := &examples.DepositFunds{
			Amount: &examples.Currency{
				Amount: winner.Amount,
			},
		}

		cmdAny, err := anypb.New(depositFunds)
		if err != nil {
			return nil, err
		}

		// Use angzarr_deferred - framework stamps sequence on delivery
		commands = append(commands, &pb.CommandBook{
			Cover: &pb.Cover{
				Domain: "player",
				Root:   &pb.UUID{Value: winner.PlayerRoot},
			},
			Pages: []*pb.CommandPage{
				{
					Header:  &pb.PageHeader{SequenceType: &pb.PageHeader_AngzarrDeferred{AngzarrDeferred: &pb.AngzarrDeferredSequence{}}},
					Payload: &pb.CommandPage_Command{Command: cmdAny},
				},
			},
		})
	}

	return commands, nil
}

func main() {
	saga := NewHandPlayerSaga()
	angzarr.RunOOSagaServer("saga-hand-player", "50215", saga)
}
