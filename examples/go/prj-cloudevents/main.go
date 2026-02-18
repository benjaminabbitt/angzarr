// CloudEvents projector - publishes player events as CloudEvents.
//
// This projector transforms internal domain events into CloudEvents 1.0 format
// for external consumption via HTTP webhooks or Kafka.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/types/known/anypb"
)

// docs:start:cloudevents_oo
type PlayerCloudEventsProjector struct {
	angzarr.CloudEventsProjectorBase
}

func NewPlayerCloudEventsProjector() *PlayerCloudEventsProjector {
	p := &PlayerCloudEventsProjector{}
	p.Init("prj-player-cloudevents", "player")
	return p
}

func (p *PlayerCloudEventsProjector) OnPlayerRegistered(event *examples.PlayerRegistered) *pb.CloudEvent {
	// Filter sensitive fields, return public version
	public := &examples.PublicPlayerRegistered{
		DisplayName: event.DisplayName,
		PlayerType:  event.PlayerType,
	}
	data, _ := anypb.New(public)
	return &pb.CloudEvent{
		Type: "com.poker.player.registered",
		Data: data,
	}
}

func (p *PlayerCloudEventsProjector) OnFundsDeposited(event *examples.FundsDeposited) *pb.CloudEvent {
	public := &examples.PublicFundsDeposited{
		Amount: event.Amount,
	}
	data, _ := anypb.New(public)
	return &pb.CloudEvent{
		Type:       "com.poker.player.deposited",
		Data:       data,
		Extensions: map[string]string{"priority": "normal"},
	}
}

// docs:end:cloudevents_oo

// docs:start:cloudevents_router
func handlePlayerRegistered(event *examples.PlayerRegistered) *pb.CloudEvent {
	public := &examples.PublicPlayerRegistered{
		DisplayName: event.DisplayName,
		PlayerType:  event.PlayerType,
	}
	data, _ := anypb.New(public)
	return &pb.CloudEvent{Type: "com.poker.player.registered", Data: data}
}

func handleFundsDeposited(event *examples.FundsDeposited) *pb.CloudEvent {
	public := &examples.PublicFundsDeposited{Amount: event.Amount}
	data, _ := anypb.New(public)
	return &pb.CloudEvent{
		Type:       "com.poker.player.deposited",
		Data:       data,
		Extensions: map[string]string{"priority": "normal"},
	}
}

var router = angzarr.NewCloudEventsRouter("prj-player-cloudevents", "player").
	On("PlayerRegistered", handlePlayerRegistered).
	On("FundsDeposited", handleFundsDeposited)

// docs:end:cloudevents_router

func main() {
	angzarr.RunCloudEventsProjector("prj-player-cloudevents", "50291", router)
}
