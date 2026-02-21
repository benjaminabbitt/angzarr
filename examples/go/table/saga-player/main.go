// Saga: Table → Player
//
// Reacts to HandEnded events from Table domain.
// Sends ReleaseFunds commands to Player domain.
package main

import (
	"encoding/hex"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// prepareHandEnded declares all players in StackChanges as destinations.
func prepareHandEnded(source *pb.EventBook, event *anypb.Any) []*pb.Cover {
	var handEnded examples.HandEnded
	if err := proto.Unmarshal(event.Value, &handEnded); err != nil {
		return nil
	}

	var covers []*pb.Cover
	for playerHex := range handEnded.StackChanges {
		playerRoot, err := hex.DecodeString(playerHex)
		if err != nil {
			continue
		}
		covers = append(covers, &pb.Cover{
			Domain: "player",
			Root:   &pb.UUID{Value: playerRoot},
		})
	}
	return covers
}

// handleHandEnded translates HandEnded → ReleaseFunds for each player.
func handleHandEnded(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) ([]*pb.CommandBook, error) {
	var handEnded examples.HandEnded
	if err := proto.Unmarshal(event.Value, &handEnded); err != nil {
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

	// Create ReleaseFunds commands for all players
	for playerHex := range handEnded.StackChanges {
		playerRoot, err := hex.DecodeString(playerHex)
		if err != nil {
			continue
		}

		// Get sequence from destination state
		var destSeq uint32
		if dest, ok := destMap[playerHex]; ok {
			destSeq = angzarr.NextSequence(dest)
		}

		releaseFunds := &examples.ReleaseFunds{
			TableRoot: handEnded.HandRoot,
		}

		cmdAny, err := anypb.New(releaseFunds)
		if err != nil {
			return nil, err
		}

		commands = append(commands, &pb.CommandBook{
			Cover: &pb.Cover{
				Domain:        "player",
				Root:          &pb.UUID{Value: playerRoot},
				CorrelationId: correlationID,
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
	router := angzarr.NewEventRouter("saga-table-player").
		Domain("table").
		Prepare("HandEnded", prepareHandEnded).
		On("HandEnded", handleHandEnded)

	angzarr.RunSagaServer("saga-table-player", "50213", router)
}
