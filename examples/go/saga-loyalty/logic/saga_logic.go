// Package logic provides pure business logic for the loyalty saga.
// This package has no gRPC dependencies and can be tested in isolation.
package logic

import (
	"encoding/hex"
	"fmt"

	"saga-loyalty/proto/angzarr"
	"saga-loyalty/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

// SagaCommand represents a command to be emitted by the saga.
type SagaCommand struct {
	Domain  string
	RootID  []byte
	Command *examples.AddLoyaltyPoints
}

// SagaLogic provides business logic operations for the loyalty saga.
type SagaLogic interface {
	// ProcessEvents processes an event book and returns commands to emit.
	ProcessEvents(eventBook *angzarr.EventBook) []*SagaCommand
}

// DefaultSagaLogic is the default implementation of SagaLogic.
type DefaultSagaLogic struct{}

// NewSagaLogic creates a new SagaLogic instance.
func NewSagaLogic() SagaLogic {
	return &DefaultSagaLogic{}
}

// ProcessEvents extracts TransactionCompleted events and generates AddLoyaltyPoints commands.
func (l *DefaultSagaLogic) ProcessEvents(eventBook *angzarr.EventBook) []*SagaCommand {
	if eventBook == nil || len(eventBook.Pages) == 0 {
		return nil
	}

	var commands []*SagaCommand

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		// Check if this is a TransactionCompleted event
		if !page.Event.MessageIs(&examples.TransactionCompleted{}) {
			continue
		}

		var event examples.TransactionCompleted
		if err := page.Event.UnmarshalTo(&event); err != nil {
			continue
		}

		points := event.LoyaltyPointsEarned
		if points <= 0 {
			continue
		}

		// Get root ID from the transaction cover
		var rootID []byte
		if eventBook.Cover != nil && eventBook.Cover.Root != nil {
			rootID = eventBook.Cover.Root.Value
		}

		transactionID := hex.EncodeToString(rootID)

		// Create AddLoyaltyPoints command
		addPointsCmd := &examples.AddLoyaltyPoints{
			Points: points,
			Reason: fmt.Sprintf("transaction:%s", transactionID),
		}

		commands = append(commands, &SagaCommand{
			Domain:  "customer",
			RootID:  rootID,
			Command: addPointsCmd,
		})
	}

	return commands
}

// PackCommand wraps a SagaCommand into a CommandBook for gRPC transmission.
func PackCommand(cmd *SagaCommand) (*angzarr.CommandBook, error) {
	cmdAny, err := anypb.New(cmd.Command)
	if err != nil {
		return nil, err
	}

	return &angzarr.CommandBook{
		Cover: &angzarr.Cover{
			Domain: cmd.Domain,
			Root:   &angzarr.UUID{Value: cmd.RootID},
		},
		Pages: []*angzarr.CommandPage{
			{
				Sequence:    0,
				Synchronous: false,
				Command:     cmdAny,
			},
		},
	}, nil
}
