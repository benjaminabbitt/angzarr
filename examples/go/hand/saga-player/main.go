// Saga: Hand → Player
//
// Reacts to PotAwarded events from Hand domain.
// Sends DepositFunds commands to Player domain.
package main

import (
	"encoding/hex"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// preparePotAwarded declares all winners as destinations.
func preparePotAwarded(source *pb.EventBook, event *anypb.Any) []*pb.Cover {
	var potAwarded examples.PotAwarded
	if err := proto.Unmarshal(event.Value, &potAwarded); err != nil {
		return nil
	}

	var covers []*pb.Cover
	for _, winner := range potAwarded.Winners {
		covers = append(covers, &pb.Cover{
			Domain: "player",
			Root:   &pb.UUID{Value: winner.PlayerRoot},
		})
	}
	return covers
}

// handlePotAwarded translates PotAwarded → DepositFunds for each winner.
func handlePotAwarded(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
	var potAwarded examples.PotAwarded
	if err := proto.Unmarshal(event.Value, &potAwarded); err != nil {
		return nil, err
	}

	// Get correlation ID from source
	var correlationID string
	if source.Cover != nil {
		correlationID = source.Cover.CorrelationId
	}

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
	for _, winner := range potAwarded.Winners {
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
				Domain:        "player",
				Root:          &pb.UUID{Value: winner.PlayerRoot},
				CorrelationId: correlationID,
			},
			Pages: []*pb.CommandPage{
				{
					Sequence: destSeq,
					Command:  cmdAny,
				},
			},
		})
	}

	return commands, nil
}

func main() {
	router := angzarr.NewEventRouter("saga-hand-player", "hand").
		Sends("player", "DepositFunds").
		Prepare("PotAwarded", preparePotAwarded).
		On("PotAwarded", handlePotAwarded)

	angzarr.RunSagaServer("saga-hand-player", "50214", router)
}
