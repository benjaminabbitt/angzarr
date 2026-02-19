// Saga: Hand → Table
//
// Reacts to HandComplete events from Hand domain.
// Sends EndHand commands to Table domain.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// prepareHandComplete declares the table aggregate as destination.
func prepareHandComplete(source *pb.EventBook, event *anypb.Any) []*pb.Cover {
	var handComplete examples.HandComplete
	if err := proto.Unmarshal(event.Value, &handComplete); err != nil {
		return nil
	}

	return []*pb.Cover{
		{
			Domain: "table",
			Root:   &pb.UUID{Value: handComplete.TableRoot},
		},
	}
}

// handleHandComplete translates HandComplete → EndHand.
func handleHandComplete(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
	var handComplete examples.HandComplete
	if err := proto.Unmarshal(event.Value, &handComplete); err != nil {
		return nil, err
	}

	// Get next sequence from destination state
	var destSeq uint32
	if len(destinations) > 0 {
		destSeq = angzarr.NextSequence(destinations[0])
	}

	// Get correlation ID and hand_root from source
	var correlationID string
	var handRoot []byte
	if source.Cover != nil {
		correlationID = source.Cover.CorrelationId
		if source.Cover.Root != nil {
			handRoot = source.Cover.Root.Value
		}
	}

	// Convert PotWinner to PotResult
	results := make([]*examples.PotResult, len(handComplete.Winners))
	for i, winner := range handComplete.Winners {
		results[i] = &examples.PotResult{
			WinnerRoot:  winner.PlayerRoot,
			Amount:      winner.Amount,
			PotType:     winner.PotType,
			WinningHand: winner.WinningHand,
		}
	}

	// Build EndHand command
	endHand := &examples.EndHand{
		HandRoot: handRoot,
		Results:  results,
	}

	cmdAny, err := anypb.New(endHand)
	if err != nil {
		return nil, err
	}

	return []*pb.CommandBook{
		{
			Cover: &pb.Cover{
				Domain:        "table",
				Root:          &pb.UUID{Value: handComplete.TableRoot},
				CorrelationId: correlationID,
			},
			Pages: []*pb.CommandPage{
				{
					Sequence: destSeq,
					Command:  cmdAny,
				},
			},
		},
	}, nil
}

func main() {
	router := angzarr.NewEventRouter("saga-hand-table").
		Domain("hand").
		Prepare("HandComplete", prepareHandComplete).
		On("HandComplete", handleHandComplete)

	angzarr.RunSagaServer("saga-hand-table", "50212", router)
}
