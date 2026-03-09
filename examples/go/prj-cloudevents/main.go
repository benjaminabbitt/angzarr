// Projector: CloudEvents
//
// Transforms player domain events into CloudEvents format for external consumption.
// Filters sensitive fields (email, internal IDs) before publishing.
package main

import (
	"strings"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// docs:start:cloudevents_projector
func handlePlayerEvents(events *pb.EventBook) (*pb.Projection, error) {
	if events == nil || events.Cover == nil {
		return &pb.Projection{}, nil
	}

	var cloudEvents []*pb.CloudEvent
	var lastSeq uint32

	for _, page := range events.Pages {
		event := page.GetEvent()
		if event == nil {
			continue
		}
		lastSeq = page.GetHeader().GetSequence()

		typeURL := event.TypeUrl
		typeName := typeURL[strings.LastIndex(typeURL, ".")+1:]

		cloudEvent := transformToCloudEvent(typeName, event.Value)
		if cloudEvent != nil {
			cloudEvents = append(cloudEvents, cloudEvent)
		}
	}

	// Pack CloudEventsResponse into Projection.Projection field
	ceResponse := &pb.CloudEventsResponse{Events: cloudEvents}
	projectionAny, _ := anypb.New(ceResponse)

	return &pb.Projection{
		Cover:      events.Cover,
		Projector:  "prj-player-cloudevents",
		Sequence:   lastSeq,
		Projection: projectionAny,
	}, nil
}

func transformToCloudEvent(typeName string, data []byte) *pb.CloudEvent {
	switch typeName {
	case "PlayerRegistered":
		var e examples.PlayerRegistered
		if err := proto.Unmarshal(data, &e); err != nil {
			return nil
		}
		// Create public version - filter sensitive fields
		publicData := &examples.PlayerRegistered{
			DisplayName: e.DisplayName,
			PlayerType:  e.PlayerType,
			// Omit: Email (PII), AiModelId (internal)
		}
		dataAny, _ := anypb.New(publicData)
		return &pb.CloudEvent{
			Type: "com.poker.player.registered",
			Data: dataAny,
		}

	case "FundsDeposited":
		var e examples.FundsDeposited
		if err := proto.Unmarshal(data, &e); err != nil {
			return nil
		}
		// Create public version
		publicData := &examples.FundsDeposited{
			Amount: e.Amount,
			// Omit: NewBalance (sensitive account info)
		}
		dataAny, _ := anypb.New(publicData)
		return &pb.CloudEvent{
			Type:       "com.poker.player.deposited",
			Data:       dataAny,
			Extensions: map[string]string{"priority": "normal"},
		}

	case "FundsWithdrawn":
		var e examples.FundsWithdrawn
		if err := proto.Unmarshal(data, &e); err != nil {
			return nil
		}
		publicData := &examples.FundsWithdrawn{
			Amount: e.Amount,
		}
		dataAny, _ := anypb.New(publicData)
		return &pb.CloudEvent{
			Type: "com.poker.player.withdrawn",
			Data: dataAny,
		}
	}

	return nil
}

// docs:end:cloudevents_projector

func main() {
	handler := angzarr.NewProjectorHandler("prj-player-cloudevents", "player").
		WithHandle(handlePlayerEvents)

	angzarr.RunProjectorServer("prj-player-cloudevents", "50291", handler)
}
