// Player domain upcaster gRPC server.
//
// Transforms old event versions to current versions during replay.
// This is a passthrough upcaster - no transformations yet.
//
// # Adding Transformations
//
// When schema evolution is needed, add transformations to the router:
//
//	func upcastPlayerRegisteredV1(old *anypb.Any) *anypb.Any {
//	    var v1 examples.PlayerRegisteredV1
//	    if err := old.UnmarshalTo(&v1); err != nil {
//	        return old // passthrough on error
//	    }
//	    current := &examples.PlayerRegistered{
//	        DisplayName: v1.DisplayName,
//	        Email:       v1.Email,
//	        PlayerType:  v1.PlayerType,
//	        AiModelId:   "", // New field with default
//	    }
//	    newAny, _ := anypb.New(current)
//	    return newAny
//	}
//
//	router := NewUpcasterRouter("player").
//	    On("examples.PlayerRegisteredV1", upcastPlayerRegisteredV1)
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
)

// docs:start:upcaster_router
// buildRouter creates the upcaster router for player domain.
//
// Currently a passthrough - add transformations as needed for schema evolution.
func buildRouter() *angzarr.UpcasterRouter {
	return angzarr.NewUpcasterRouter("player")
	// Example transformation (uncomment when needed):
	// .On("examples.PlayerRegisteredV1", upcastPlayerRegisteredV1)
}

// handleUpcast transforms player domain events.
//
// Delegates to the router for any registered transformations.
// Events without registered transformations pass through unchanged.
func handleUpcast(events []*pb.EventPage) []*pb.EventPage {
	router := buildRouter()
	return router.Upcast(events)
}

// docs:end:upcaster_router

func main() {
	// docs:start:upcaster_server
	handler := angzarr.NewUpcasterGrpcHandler("upcaster-player", "player").
		WithHandle(handleUpcast)

	angzarr.RunUpcasterServer("upcaster-player", "50401", handler)
	// docs:end:upcaster_server
}
