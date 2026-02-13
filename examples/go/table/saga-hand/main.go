// Saga: Table → Hand
//
// Reacts to HandStarted events from Table domain.
// Sends DealCards commands to Hand domain.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// prepareHandStarted declares the hand aggregate as destination.
func prepareHandStarted(source *pb.EventBook, event *anypb.Any) []*pb.Cover {
	var handStarted examples.HandStarted
	if err := proto.Unmarshal(event.Value, &handStarted); err != nil {
		return nil
	}

	return []*pb.Cover{
		{
			Domain: "hand",
			Root:   &pb.UUID{Value: handStarted.HandRoot},
		},
	}
}

// handleHandStarted translates HandStarted → DealCards.
func handleHandStarted(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
	var handStarted examples.HandStarted
	if err := proto.Unmarshal(event.Value, &handStarted); err != nil {
		return nil, err
	}

	// Get next sequence from destination state
	var destSeq uint32
	if len(destinations) > 0 {
		destSeq = angzarr.NextSequence(destinations[0])
	}

	// Get correlation ID from source
	var correlationID string
	if source.Cover != nil {
		correlationID = source.Cover.CorrelationId
	}

	// Convert SeatSnapshot to PlayerInHand
	players := make([]*examples.PlayerInHand, len(handStarted.ActivePlayers))
	for i, seat := range handStarted.ActivePlayers {
		players[i] = &examples.PlayerInHand{
			PlayerRoot: seat.PlayerRoot,
			Position:   seat.Position,
			Stack:      seat.Stack,
		}
	}

	// Build DealCards command
	dealCards := &examples.DealCards{
		TableRoot:      handStarted.HandRoot,
		HandNumber:     handStarted.HandNumber,
		GameVariant:    handStarted.GameVariant,
		Players:        players,
		DealerPosition: handStarted.DealerPosition,
		SmallBlind:     handStarted.SmallBlind,
		BigBlind:       handStarted.BigBlind,
	}

	cmdAny, err := anypb.New(dealCards)
	if err != nil {
		return nil, err
	}

	return []*pb.CommandBook{
		{
			Cover: &pb.Cover{
				Domain:        "hand",
				Root:          &pb.UUID{Value: handStarted.HandRoot},
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
	router := angzarr.NewEventRouter("saga-table-hand", "table").
		Sends("hand", "DealCards").
		Prepare("HandStarted", prepareHandStarted).
		On("HandStarted", handleHandStarted)

	angzarr.RunSagaServer("saga-table-hand", "50211", router)
}
