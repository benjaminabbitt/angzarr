package logic

import (
	"strings"
	"testing"

	"saga-loyalty/proto/angzarr"
	"saga-loyalty/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

func TestProcessEvents_NilEventBook(t *testing.T) {
	logic := NewSagaLogic()
	commands := logic.ProcessEvents(nil)

	if len(commands) != 0 {
		t.Errorf("expected no commands, got %d", len(commands))
	}
}

func TestProcessEvents_EmptyEventBook(t *testing.T) {
	logic := NewSagaLogic()
	commands := logic.ProcessEvents(&angzarr.EventBook{})

	if len(commands) != 0 {
		t.Errorf("expected no commands, got %d", len(commands))
	}
}

func TestProcessEvents_TransactionCreatedDoesNotGenerateCommand(t *testing.T) {
	logic := NewSagaLogic()

	event := &examples.TransactionCreated{
		CustomerId:    "cust-001",
		SubtotalCents: 2000,
	}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
			Root:   &angzarr.UUID{Value: []byte("txn-001")},
		},
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	commands := logic.ProcessEvents(eventBook)

	if len(commands) != 0 {
		t.Errorf("expected no commands for TransactionCreated, got %d", len(commands))
	}
}

func TestProcessEvents_TransactionCompletedGeneratesCommand(t *testing.T) {
	logic := NewSagaLogic()

	event := &examples.TransactionCompleted{
		LoyaltyPointsEarned: 20,
	}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
			Root:   &angzarr.UUID{Value: []byte("txn-001")},
		},
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	commands := logic.ProcessEvents(eventBook)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}

	cmd := commands[0]
	if cmd.Domain != "customer" {
		t.Errorf("expected domain 'customer', got %q", cmd.Domain)
	}
	if cmd.Command.Points != 20 {
		t.Errorf("expected points 20, got %d", cmd.Command.Points)
	}
}

func TestProcessEvents_ZeroPointsDoesNotGenerateCommand(t *testing.T) {
	logic := NewSagaLogic()

	event := &examples.TransactionCompleted{
		LoyaltyPointsEarned: 0,
	}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
			Root:   &angzarr.UUID{Value: []byte("txn-001")},
		},
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	commands := logic.ProcessEvents(eventBook)

	if len(commands) != 0 {
		t.Errorf("expected no commands for zero points, got %d", len(commands))
	}
}

func TestProcessEvents_NegativePointsDoesNotGenerateCommand(t *testing.T) {
	logic := NewSagaLogic()

	event := &examples.TransactionCompleted{
		LoyaltyPointsEarned: -10,
	}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
			Root:   &angzarr.UUID{Value: []byte("txn-001")},
		},
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	commands := logic.ProcessEvents(eventBook)

	if len(commands) != 0 {
		t.Errorf("expected no commands for negative points, got %d", len(commands))
	}
}

func TestProcessEvents_ReasonContainsTransaction(t *testing.T) {
	logic := NewSagaLogic()

	event := &examples.TransactionCompleted{
		LoyaltyPointsEarned: 50,
	}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
			Root:   &angzarr.UUID{Value: []byte("txn-001")},
		},
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	commands := logic.ProcessEvents(eventBook)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}

	if !strings.Contains(commands[0].Command.Reason, "transaction") {
		t.Errorf("expected reason to contain 'transaction', got %q", commands[0].Command.Reason)
	}
}

func TestProcessEvents_MultiplePages(t *testing.T) {
	logic := NewSagaLogic()

	created := &examples.TransactionCreated{
		CustomerId:    "cust-001",
		SubtotalCents: 2000,
	}
	createdAny, _ := anypb.New(created)

	completed := &examples.TransactionCompleted{
		LoyaltyPointsEarned: 30,
	}
	completedAny, _ := anypb.New(completed)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
			Root:   &angzarr.UUID{Value: []byte("txn-001")},
		},
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: completedAny},
		},
	}

	commands := logic.ProcessEvents(eventBook)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}

	if commands[0].Command.Points != 30 {
		t.Errorf("expected points 30, got %d", commands[0].Command.Points)
	}
}

func TestProcessEvents_NilPageEvent(t *testing.T) {
	logic := NewSagaLogic()

	event := &examples.TransactionCompleted{
		LoyaltyPointsEarned: 20,
	}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
			Root:   &angzarr.UUID{Value: []byte("txn-001")},
		},
		Pages: []*angzarr.EventPage{
			{Event: nil},
			{Event: eventAny},
		},
	}

	commands := logic.ProcessEvents(eventBook)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}
}

func TestPackCommand(t *testing.T) {
	cmd := &SagaCommand{
		Domain: "customer",
		RootID: []byte("cust-001"),
		Command: &examples.AddLoyaltyPoints{
			Points: 100,
			Reason: "transaction:txn-001",
		},
	}

	commandBook, err := PackCommand(cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if commandBook.Cover.Domain != "customer" {
		t.Errorf("expected domain 'customer', got %q", commandBook.Cover.Domain)
	}

	if string(commandBook.Cover.Root.Value) != "cust-001" {
		t.Errorf("expected root 'cust-001', got %q", string(commandBook.Cover.Root.Value))
	}

	if len(commandBook.Pages) != 1 {
		t.Fatalf("expected 1 page, got %d", len(commandBook.Pages))
	}

	var addPoints examples.AddLoyaltyPoints
	if err := commandBook.Pages[0].Command.UnmarshalTo(&addPoints); err != nil {
		t.Fatalf("failed to unmarshal command: %v", err)
	}

	if addPoints.Points != 100 {
		t.Errorf("expected points 100, got %d", addPoints.Points)
	}
}
