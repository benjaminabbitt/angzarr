// Process Manager: Hand Flow
//
// Orchestrates the flow of poker hands by:
// 1. Subscribing to table and hand domain events
// 2. Managing hand process state machines
// 3. Sending commands to drive hands forward
package main

import (
	"encoding/hex"
	"strings"
	"sync"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/proto"
)

// HandFlowManager manages all active hand processes.
type HandFlowManager struct {
	mu        sync.RWMutex
	processes map[string]*HandProcess // correlation_id -> HandProcess
}

// NewHandFlowManager creates a new hand flow manager.
func NewHandFlowManager() *HandFlowManager {
	return &HandFlowManager{
		processes: make(map[string]*HandProcess),
	}
}

// Prepare declares which additional destinations are needed.
func (m *HandFlowManager) Prepare(trigger, processState *pb.EventBook) []*pb.Cover {
	var destinations []*pb.Cover

	for _, page := range trigger.Pages {
		if page.Event == nil {
			continue
		}
		typeURL := page.Event.TypeUrl

		if strings.HasSuffix(typeURL, "HandStarted") {
			var event examples.HandStarted
			if err := proto.Unmarshal(page.Event.Value, &event); err == nil {
				destinations = append(destinations, &pb.Cover{
					Domain: "hand",
					Root:   &pb.UUID{Value: event.HandRoot},
				})
			}
		}
	}

	return destinations
}

// Handle processes events and produces commands.
func (m *HandFlowManager) Handle(trigger, processState *pb.EventBook, destinations []*pb.EventBook) ([]*pb.CommandBook, *pb.EventBook, error) {
	var commands []*pb.CommandBook

	// Get correlation ID from trigger
	var correlationID string
	if trigger.Cover != nil {
		correlationID = trigger.Cover.CorrelationId
	}

	// Build destination map for sequence lookup
	destMap := make(map[string]*pb.EventBook)
	for _, dest := range destinations {
		if dest.Cover != nil && dest.Cover.Root != nil {
			key := hex.EncodeToString(dest.Cover.Root.Value)
			destMap[key] = dest
		}
	}

	for _, page := range trigger.Pages {
		if page.Event == nil {
			continue
		}
		typeURL := page.Event.TypeUrl
		typeName := typeURL[strings.LastIndex(typeURL, ".")+1:]

		switch typeName {
		case "HandStarted":
			var event examples.HandStarted
			if err := proto.Unmarshal(page.Event.Value, &event); err == nil {
				cmds := m.handleHandStarted(&event, trigger.Cover.Root.Value, correlationID, destMap)
				commands = append(commands, cmds...)
			}

		case "CardsDealt":
			var event examples.CardsDealt
			if err := proto.Unmarshal(page.Event.Value, &event); err == nil {
				cmds := m.handleCardsDealt(&event, correlationID, destMap)
				commands = append(commands, cmds...)
			}

		case "BlindPosted":
			var event examples.BlindPosted
			if err := proto.Unmarshal(page.Event.Value, &event); err == nil {
				cmds := m.handleBlindPosted(&event, correlationID, destMap)
				commands = append(commands, cmds...)
			}

		case "ActionTaken":
			var event examples.ActionTaken
			if err := proto.Unmarshal(page.Event.Value, &event); err == nil {
				cmds := m.handleActionTaken(&event, correlationID, destMap)
				commands = append(commands, cmds...)
			}

		case "CommunityCardsDealt":
			var event examples.CommunityCardsDealt
			if err := proto.Unmarshal(page.Event.Value, &event); err == nil {
				cmds := m.handleCommunityDealt(&event, correlationID, destMap)
				commands = append(commands, cmds...)
			}

		case "PotAwarded":
			var event examples.PotAwarded
			if err := proto.Unmarshal(page.Event.Value, &event); err == nil {
				m.handlePotAwarded(&event, correlationID)
			}
		}
	}

	// PM doesn't emit its own events in this implementation
	return commands, nil, nil
}

func (m *HandFlowManager) handleHandStarted(event *examples.HandStarted, tableRoot []byte, correlationID string, destMap map[string]*pb.EventBook) []*pb.CommandBook {
	m.mu.Lock()
	defer m.mu.Unlock()

	process := NewHandProcess(event, tableRoot)
	m.processes[correlationID] = process

	// Hand started doesn't immediately produce commands
	// The saga-table-hand will send DealCards
	return nil
}

func (m *HandFlowManager) handleCardsDealt(event *examples.CardsDealt, correlationID string, destMap map[string]*pb.EventBook) []*pb.CommandBook {
	m.mu.Lock()
	defer m.mu.Unlock()

	process := m.processes[correlationID]
	if process == nil {
		return nil
	}

	cmds := process.HandleCardsDealt(event)
	return m.setSequences(cmds, destMap, correlationID)
}

func (m *HandFlowManager) handleBlindPosted(event *examples.BlindPosted, correlationID string, destMap map[string]*pb.EventBook) []*pb.CommandBook {
	m.mu.Lock()
	defer m.mu.Unlock()

	process := m.processes[correlationID]
	if process == nil {
		return nil
	}

	cmds := process.HandleBlindPosted(event)
	return m.setSequences(cmds, destMap, correlationID)
}

func (m *HandFlowManager) handleActionTaken(event *examples.ActionTaken, correlationID string, destMap map[string]*pb.EventBook) []*pb.CommandBook {
	m.mu.Lock()
	defer m.mu.Unlock()

	process := m.processes[correlationID]
	if process == nil {
		return nil
	}

	cmds := process.HandleActionTaken(event)
	return m.setSequences(cmds, destMap, correlationID)
}

func (m *HandFlowManager) handleCommunityDealt(event *examples.CommunityCardsDealt, correlationID string, destMap map[string]*pb.EventBook) []*pb.CommandBook {
	m.mu.Lock()
	defer m.mu.Unlock()

	process := m.processes[correlationID]
	if process == nil {
		return nil
	}

	cmds := process.HandleCommunityDealt(event)
	return m.setSequences(cmds, destMap, correlationID)
}

func (m *HandFlowManager) handlePotAwarded(event *examples.PotAwarded, correlationID string) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process := m.processes[correlationID]
	if process == nil {
		return
	}

	process.HandlePotAwarded(event)

	// Clean up completed process
	delete(m.processes, correlationID)
}

func (m *HandFlowManager) setSequences(cmds []*pb.CommandBook, destMap map[string]*pb.EventBook, correlationID string) []*pb.CommandBook {
	for _, cmd := range cmds {
		if cmd.Cover == nil || cmd.Cover.Root == nil {
			continue
		}

		// Set correlation ID
		cmd.Cover.CorrelationId = correlationID

		// Look up destination sequence
		key := hex.EncodeToString(cmd.Cover.Root.Value)
		if dest, ok := destMap[key]; ok {
			seq := angzarr.NextSequence(dest)
			for _, page := range cmd.Pages {
				page.Sequence = seq
			}
		}
	}
	return cmds
}

func main() {
	manager := NewHandFlowManager()

	handler := angzarr.NewProcessManagerHandler("hand-flow").
		ListenTo("table", "HandStarted", "HandEnded").
		ListenTo("hand",
			"CardsDealt",
			"BlindPosted",
			"ActionTaken",
			"BettingRoundComplete",
			"CommunityCardsDealt",
			"DrawCompleted",
			"ShowdownStarted",
			"CardsRevealed",
			"CardsMucked",
			"PotAwarded",
			"HandComplete",
		).
		WithPrepare(manager.Prepare).
		WithHandle(manager.Handle)

	angzarr.RunProcessManagerServer("hand-flow", "50291", handler)
}
