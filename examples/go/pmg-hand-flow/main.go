// Hand Flow Process Manager - orchestrates poker hand phases across domains.
//
// This PM coordinates the workflow between table and hand domains,
// tracking phase transitions and dispatching commands as the hand progresses.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
)

// docs:start:pm_state
type HandPhase int

const (
	AwaitingDeal HandPhase = iota
	Dealing
	Blinds
	Betting
	Complete
)

type HandFlowState struct {
	HandID      string
	Phase       HandPhase
	PlayerCount int32
}

// docs:end:pm_state

// docs:start:pm_handler
type HandFlowPM struct{}

func (pm *HandFlowPM) HandleHandStarted(event *examples.HandStarted, state *HandFlowState) []*pb.CommandBook {
	state.HandID = event.HandId
	state.Phase = Dealing
	state.PlayerCount = event.PlayerCount

	return []*pb.CommandBook{
		angzarr.BuildCommand("hand", &examples.DealCards{
			HandId:      event.HandId,
			PlayerCount: event.PlayerCount,
		}),
	}
}

func (pm *HandFlowPM) HandleCardsDealt(event *examples.CardsDealt, state *HandFlowState) []*pb.CommandBook {
	state.Phase = Blinds
	return []*pb.CommandBook{
		angzarr.BuildCommand("hand", &examples.PostBlinds{HandId: state.HandID}),
	}
}

func (pm *HandFlowPM) HandleHandComplete(event *examples.HandComplete, state *HandFlowState) []*pb.CommandBook {
	state.Phase = Complete
	return []*pb.CommandBook{
		angzarr.BuildCommand("table", &examples.EndHand{
			HandId:   state.HandID,
			WinnerId: event.WinnerId,
		}),
	}
}

// docs:end:pm_handler

func main() {
	pm := &HandFlowPM{}
	angzarr.RunProcessManager("pmg-hand-flow", "50392", pm)
}
