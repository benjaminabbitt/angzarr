// Saga: Player -> Table
//
// Propagates player sit-out/sit-in intent as facts to the table domain.
// Player events trigger corresponding facts in the table aggregate.
//
// Flow:
// - PlayerSittingOut -> PlayerSatOut fact to table
// - PlayerReturningToPlay -> PlayerSatIn fact to table
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// prepareSittingOut - no destinations needed (emits facts, not commands)
func prepareSittingOut(source *pb.EventBook, event *anypb.Any) []*pb.Cover {
	return nil
}

// prepareReturningToPlay - no destinations needed (emits facts, not commands)
func prepareReturningToPlay(source *pb.EventBook, event *anypb.Any) []*pb.Cover {
	return nil
}

// handleSittingOut translates PlayerSittingOut -> PlayerSatOut fact for table
func handleSittingOut(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) (*angzarr.SagaHandlerResponse, error) {
	var sittingOut examples.PlayerSittingOut
	if err := proto.Unmarshal(event.Value, &sittingOut); err != nil {
		return nil, err
	}

	// Get player root from source
	var playerRoot []byte
	if source.Cover != nil && source.Cover.Root != nil {
		playerRoot = source.Cover.Root.Value
	}

	// Create PlayerSatOut fact for the table
	satOut := &examples.PlayerSatOut{
		PlayerRoot: playerRoot,
		SatOutAt:   sittingOut.SatOutAt,
	}

	factAny, err := anypb.New(satOut)
	if err != nil {
		return nil, err
	}

	fact := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "table",
			Root:   &pb.UUID{Value: sittingOut.TableRoot},
		},
		Pages: []*pb.EventPage{
			{Event: factAny},
		},
	}

	return &angzarr.SagaHandlerResponse{
		Events: []*pb.EventBook{fact},
	}, nil
}

// handleReturningToPlay translates PlayerReturningToPlay -> PlayerSatIn fact for table
func handleReturningToPlay(source *pb.EventBook, event *anypb.Any, destinations []*pb.EventBook) (*angzarr.SagaHandlerResponse, error) {
	var returning examples.PlayerReturningToPlay
	if err := proto.Unmarshal(event.Value, &returning); err != nil {
		return nil, err
	}

	// Get player root from source
	var playerRoot []byte
	if source.Cover != nil && source.Cover.Root != nil {
		playerRoot = source.Cover.Root.Value
	}

	// Create PlayerSatIn fact for the table
	satIn := &examples.PlayerSatIn{
		PlayerRoot: playerRoot,
		SatInAt:    returning.SatInAt,
	}

	factAny, err := anypb.New(satIn)
	if err != nil {
		return nil, err
	}

	fact := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "table",
			Root:   &pb.UUID{Value: returning.TableRoot},
		},
		Pages: []*pb.EventPage{
			{Event: factAny},
		},
	}

	return &angzarr.SagaHandlerResponse{
		Events: []*pb.EventBook{fact},
	}, nil
}

func main() {
	router := angzarr.NewEventRouter("saga-player-table").
		Domain("player").
		Prepare("PlayerSittingOut", prepareSittingOut).
		On("PlayerSittingOut", handleSittingOut).
		Prepare("PlayerReturningToPlay", prepareReturningToPlay).
		On("PlayerReturningToPlay", handleReturningToPlay)

	angzarr.RunSagaServer("saga-player-table", "50214", router)
}
