// Saga: Hand → Player (OO Pattern)
//
// Reacts to PotAwarded events from Hand domain.
// Sends DepositFunds commands to Player domain.
//
// Uses the OO-style implementation with SagaBase and method-based
// handlers with fluent registration.
package main

import (
	"encoding/hex"

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

	// Register prepare handler
	s.Prepares(s.preparePotAwarded)

	// Register event handler (multi-command)
	s.HandlesMulti(s.handlePotAwarded)

	return s
}

// preparePotAwarded declares all winners as destinations.
func (s *HandPlayerSaga) preparePotAwarded(event *examples.PotAwarded) []*pb.Cover {
	var covers []*pb.Cover
	for _, winner := range event.Winners {
		covers = append(covers, &pb.Cover{
			Domain: "player",
			Root:   &pb.UUID{Value: winner.PlayerRoot},
		})
	}
	return covers
}

// handlePotAwarded translates PotAwarded → DepositFunds for each winner.
func (s *HandPlayerSaga) handlePotAwarded(
	event *examples.PotAwarded,
	destinations []*pb.EventBook,
) ([]*pb.CommandBook, error) {
	// Build a map from player root to destination for sequence lookup
	destMap := make(map[string]*pb.EventBook)
	for _, dest := range destinations {
		if dest.Cover != nil && dest.Cover.Root != nil {
			key := hex.EncodeToString(dest.Cover.Root.Value)
			destMap[key] = dest
		}
	}

	var commands []*pb.CommandBook

	// Create DepositFunds commands for each winner
	for _, winner := range event.Winners {
		playerKey := hex.EncodeToString(winner.PlayerRoot)

		// Get sequence from destination state
		var destSeq uint32
		if dest, ok := destMap[playerKey]; ok {
			destSeq = angzarr.NextSequence(dest)
		}

		depositFunds := &examples.DepositFunds{
			Amount: &examples.Currency{
				Amount: winner.Amount,
			},
		}

		cmdAny, err := anypb.New(depositFunds)
		if err != nil {
			return nil, err
		}

		commands = append(commands, &pb.CommandBook{
			Cover: &pb.Cover{
				Domain: "player",
				Root:   &pb.UUID{Value: winner.PlayerRoot},
			},
			Pages: []*pb.CommandPage{
				{
					Sequence: destSeq,
					Payload:  &pb.CommandPage_Command{Command: cmdAny},
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
