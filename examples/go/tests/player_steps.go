package tests

import (
	"context"
	"encoding/hex"
	"fmt"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/benjaminabbitt/angzarr/examples/go/player/agg/handlers"
	"github.com/cucumber/godog"
	"github.com/google/uuid"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// PlayerContext holds state for player aggregate scenarios
type PlayerContext struct {
	eventPages  []*pb.EventPage
	state       handlers.PlayerState
	resultEvent *anypb.Any
	resultBook  *pb.EventBook
	lastError   error
}

func newPlayerContext() *PlayerContext {
	return &PlayerContext{
		eventPages: []*pb.EventPage{},
		state:      handlers.NewPlayerState(),
	}
}

// InitPlayerSteps registers player aggregate step definitions
func InitPlayerSteps(ctx *godog.ScenarioContext) {
	pc := newPlayerContext()

	// Reset before each scenario
	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		pc.eventPages = []*pb.EventPage{}
		pc.state = handlers.NewPlayerState()
		pc.resultEvent = nil
		pc.resultBook = nil
		pc.lastError = nil
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^no prior events for the player aggregate$`, pc.noPriorEvents)
	ctx.Step(`^a PlayerRegistered event for "([^"]*)"$`, pc.playerRegisteredFor)
	ctx.Step(`^a FundsDeposited event with amount (\d+)$`, pc.fundsDepositedWithAmount)
	ctx.Step(`^a FundsReserved event with amount (\d+) for table "([^"]*)"$`, pc.fundsReservedForTable)

	// When steps
	ctx.Step(`^I handle a RegisterPlayer command with name "([^"]*)" and email "([^"]*)"$`, pc.handleRegisterPlayer)
	ctx.Step(`^I handle a RegisterPlayer command with name "([^"]*)" and email "([^"]*)" as AI$`, pc.handleRegisterPlayerAI)
	ctx.Step(`^I handle a DepositFunds command with amount (\d+)$`, pc.handleDepositFunds)
	ctx.Step(`^I handle a WithdrawFunds command with amount (\d+)$`, pc.handleWithdrawFunds)
	ctx.Step(`^I handle a ReserveFunds command with amount (\d+) for table "([^"]*)"$`, pc.handleReserveFunds)
	ctx.Step(`^I handle a ReleaseFunds command for table "([^"]*)"$`, pc.handleReleaseFunds)
	ctx.Step(`^I rebuild the player state$`, pc.rebuildPlayerState)

	// Then steps
	ctx.Step(`^the result is a (?:examples\.)?PlayerRegistered event$`, pc.resultIsPlayerRegistered)
	ctx.Step(`^the result is a (?:examples\.)?FundsDeposited event$`, pc.resultIsFundsDeposited)
	ctx.Step(`^the result is a (?:examples\.)?FundsWithdrawn event$`, pc.resultIsFundsWithdrawn)
	ctx.Step(`^the result is a (?:examples\.)?FundsReserved event$`, pc.resultIsFundsReserved)
	ctx.Step(`^the result is a (?:examples\.)?FundsReleased event$`, pc.resultIsFundsReleased)
	ctx.Step(`^the player event has display_name "([^"]*)"$`, pc.eventHasDisplayName)
	ctx.Step(`^the player event has player_type "([^"]*)"$`, pc.eventHasPlayerType)
	ctx.Step(`^the player event has amount (\d+)$`, pc.eventHasAmount)
	ctx.Step(`^the player event has new_balance (\d+)$`, pc.eventHasNewBalance)
	ctx.Step(`^the player event has new_available_balance (\d+)$`, pc.eventHasNewAvailableBalance)
	ctx.Step(`^the player state has bankroll (\d+)$`, pc.stateHasBankroll)
	ctx.Step(`^the player state has reserved_funds (\d+)$`, pc.stateHasReservedFunds)
	ctx.Step(`^the player state has available_balance (\d+)$`, pc.stateHasAvailableBalance)
	// Note: "command fails with status" and "error message contains" are registered in common_steps.go
}

// Helper functions

func (pc *PlayerContext) makeEventPage(event *anypb.Any) *pb.EventPage {
	return &pb.EventPage{
		Header:    &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: uint32(len(pc.eventPages))}},
		CreatedAt: timestamppb.Now(),
		Payload:   &pb.EventPage_Event{Event: event},
	}
}

func (pc *PlayerContext) addEvent(event *anypb.Any) {
	pc.eventPages = append(pc.eventPages, pc.makeEventPage(event))
	pc.rebuildState()
}

func (pc *PlayerContext) rebuildState() {
	id := uuid.New()
	eventBook := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "player",
			Root:   &pb.UUID{Value: id[:]},
		},
		Pages:        pc.eventPages,
		NextSequence: uint32(len(pc.eventPages)),
	}
	pc.state = handlers.RebuildState(eventBook)
}

func (pc *PlayerContext) makeCommandBook() *pb.CommandBook {
	id := uuid.New()
	return &pb.CommandBook{
		Cover: &pb.Cover{
			Domain: "player",
			Root:   &pb.UUID{Value: id[:]},
		},
	}
}

// Given step implementations

func (pc *PlayerContext) noPriorEvents() error {
	pc.eventPages = []*pb.EventPage{}
	pc.state = handlers.NewPlayerState()
	return nil
}

func (pc *PlayerContext) playerRegisteredFor(name string) error {
	event := &examples.PlayerRegistered{
		DisplayName:  name,
		Email:        strings.ToLower(name) + "@example.com",
		PlayerType:   examples.PlayerType_HUMAN,
		RegisteredAt: timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	pc.addEvent(eventAny)
	return nil
}

func (pc *PlayerContext) fundsDepositedWithAmount(amount int) error {
	newBalance := pc.state.Bankroll + int64(amount)
	event := &examples.FundsDeposited{
		Amount:      &examples.Currency{Amount: int64(amount), CurrencyCode: "CHIPS"},
		NewBalance:  &examples.Currency{Amount: newBalance, CurrencyCode: "CHIPS"},
		DepositedAt: timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	pc.addEvent(eventAny)
	return nil
}

func (pc *PlayerContext) fundsReservedForTable(amount int, tableID string) error {
	newReserved := pc.state.ReservedFunds + int64(amount)
	newAvailable := pc.state.Bankroll - newReserved
	event := &examples.FundsReserved{
		Amount:              &examples.Currency{Amount: int64(amount), CurrencyCode: "CHIPS"},
		TableRoot:           []byte(tableID),
		NewAvailableBalance: &examples.Currency{Amount: newAvailable, CurrencyCode: "CHIPS"},
		NewReservedBalance:  &examples.Currency{Amount: newReserved, CurrencyCode: "CHIPS"},
		ReservedAt:          timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	pc.addEvent(eventAny)
	return nil
}

// When step implementations

func (pc *PlayerContext) handleRegisterPlayer(name, email string) error {
	cmd := &examples.RegisterPlayer{
		DisplayName: name,
		Email:       email,
		PlayerType:  examples.PlayerType_HUMAN,
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	result, err := handlers.HandleRegisterPlayer(pc.makeCommandBook(), cmdAny, pc.state, uint32(len(pc.eventPages)))
	pc.lastError = err
	SetLastError(pc.lastError)
	if err == nil && result != nil && len(result.Pages) > 0 {
		pc.resultBook = result
		if event, ok := result.Pages[0].Payload.(*pb.EventPage_Event); ok {
			pc.resultEvent = event.Event
		}
	}
	return nil
}

func (pc *PlayerContext) handleRegisterPlayerAI(name, email string) error {
	cmd := &examples.RegisterPlayer{
		DisplayName: name,
		Email:       email,
		PlayerType:  examples.PlayerType_AI,
		AiModelId:   "gpt-4",
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	result, err := handlers.HandleRegisterPlayer(pc.makeCommandBook(), cmdAny, pc.state, uint32(len(pc.eventPages)))
	pc.lastError = err
	SetLastError(pc.lastError)
	if err == nil && result != nil && len(result.Pages) > 0 {
		pc.resultBook = result
		if event, ok := result.Pages[0].Payload.(*pb.EventPage_Event); ok {
			pc.resultEvent = event.Event
		}
	}
	return nil
}

func (pc *PlayerContext) handleDepositFunds(amount int) error {
	cmd := &examples.DepositFunds{
		Amount: &examples.Currency{Amount: int64(amount), CurrencyCode: "CHIPS"},
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	result, err := handlers.HandleDepositFunds(pc.makeCommandBook(), cmdAny, pc.state, uint32(len(pc.eventPages)))
	pc.lastError = err
	SetLastError(pc.lastError)
	if err == nil && result != nil && len(result.Pages) > 0 {
		pc.resultBook = result
		if event, ok := result.Pages[0].Payload.(*pb.EventPage_Event); ok {
			pc.resultEvent = event.Event
		}
	}
	return nil
}

func (pc *PlayerContext) handleWithdrawFunds(amount int) error {
	cmd := &examples.WithdrawFunds{
		Amount: &examples.Currency{Amount: int64(amount), CurrencyCode: "CHIPS"},
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	result, err := handlers.HandleWithdrawFunds(pc.makeCommandBook(), cmdAny, pc.state, uint32(len(pc.eventPages)))
	pc.lastError = err
	SetLastError(pc.lastError)
	if err == nil && result != nil && len(result.Pages) > 0 {
		pc.resultBook = result
		if event, ok := result.Pages[0].Payload.(*pb.EventPage_Event); ok {
			pc.resultEvent = event.Event
		}
	}
	return nil
}

func (pc *PlayerContext) handleReserveFunds(amount int, tableID string) error {
	cmd := &examples.ReserveFunds{
		Amount:    &examples.Currency{Amount: int64(amount), CurrencyCode: "CHIPS"},
		TableRoot: []byte(tableID),
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	result, err := handlers.HandleReserveFunds(pc.makeCommandBook(), cmdAny, pc.state, uint32(len(pc.eventPages)))
	pc.lastError = err
	SetLastError(pc.lastError)
	if err == nil && result != nil && len(result.Pages) > 0 {
		pc.resultBook = result
		if event, ok := result.Pages[0].Payload.(*pb.EventPage_Event); ok {
			pc.resultEvent = event.Event
		}
	}
	return nil
}

func (pc *PlayerContext) handleReleaseFunds(tableID string) error {
	cmd := &examples.ReleaseFunds{
		TableRoot: []byte(tableID),
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	result, err := handlers.HandleReleaseFunds(pc.makeCommandBook(), cmdAny, pc.state, uint32(len(pc.eventPages)))
	pc.lastError = err
	SetLastError(pc.lastError)
	if err == nil && result != nil && len(result.Pages) > 0 {
		pc.resultBook = result
		if event, ok := result.Pages[0].Payload.(*pb.EventPage_Event); ok {
			pc.resultEvent = event.Event
		}
	}
	return nil
}

func (pc *PlayerContext) rebuildPlayerState() error {
	pc.rebuildState()
	return nil
}

// Then step implementations

func (pc *PlayerContext) resultIsPlayerRegistered() error {
	if pc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", pc.lastError)
	}
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !pc.resultEvent.MessageIs(&examples.PlayerRegistered{}) {
		return fmt.Errorf("expected PlayerRegistered event, got %s", pc.resultEvent.TypeUrl)
	}
	return nil
}

func (pc *PlayerContext) resultIsFundsDeposited() error {
	if pc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", pc.lastError)
	}
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !pc.resultEvent.MessageIs(&examples.FundsDeposited{}) {
		return fmt.Errorf("expected FundsDeposited event, got %s", pc.resultEvent.TypeUrl)
	}
	return nil
}

func (pc *PlayerContext) resultIsFundsWithdrawn() error {
	if pc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", pc.lastError)
	}
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !pc.resultEvent.MessageIs(&examples.FundsWithdrawn{}) {
		return fmt.Errorf("expected FundsWithdrawn event, got %s", pc.resultEvent.TypeUrl)
	}
	return nil
}

func (pc *PlayerContext) resultIsFundsReserved() error {
	if pc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", pc.lastError)
	}
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !pc.resultEvent.MessageIs(&examples.FundsReserved{}) {
		return fmt.Errorf("expected FundsReserved event, got %s", pc.resultEvent.TypeUrl)
	}
	return nil
}

func (pc *PlayerContext) resultIsFundsReleased() error {
	if pc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", pc.lastError)
	}
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !pc.resultEvent.MessageIs(&examples.FundsReleased{}) {
		return fmt.Errorf("expected FundsReleased event, got %s", pc.resultEvent.TypeUrl)
	}
	return nil
}

func (pc *PlayerContext) eventHasDisplayName(name string) error {
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.PlayerRegistered
	if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.DisplayName != name {
		return fmt.Errorf("expected display_name=%s, got %s", name, event.DisplayName)
	}
	return nil
}

func (pc *PlayerContext) eventHasPlayerType(ptype string) error {
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.PlayerRegistered
	if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	expected := examples.PlayerType(examples.PlayerType_value[ptype])
	if event.PlayerType != expected {
		return fmt.Errorf("expected player_type=%s, got %s", ptype, event.PlayerType.String())
	}
	return nil
}

func (pc *PlayerContext) eventHasAmount(amount int) error {
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}

	// Try different event types
	if pc.resultEvent.MessageIs(&examples.FundsDeposited{}) {
		var event examples.FundsDeposited
		if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.Amount.Amount != int64(amount) {
			return fmt.Errorf("expected amount=%d, got %d", amount, event.Amount.Amount)
		}
		return nil
	}
	if pc.resultEvent.MessageIs(&examples.FundsWithdrawn{}) {
		var event examples.FundsWithdrawn
		if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.Amount.Amount != int64(amount) {
			return fmt.Errorf("expected amount=%d, got %d", amount, event.Amount.Amount)
		}
		return nil
	}
	if pc.resultEvent.MessageIs(&examples.FundsReserved{}) {
		var event examples.FundsReserved
		if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.Amount.Amount != int64(amount) {
			return fmt.Errorf("expected amount=%d, got %d", amount, event.Amount.Amount)
		}
		return nil
	}
	if pc.resultEvent.MessageIs(&examples.FundsReleased{}) {
		var event examples.FundsReleased
		if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.Amount.Amount != int64(amount) {
			return fmt.Errorf("expected amount=%d, got %d", amount, event.Amount.Amount)
		}
		return nil
	}

	return fmt.Errorf("unknown event type: %s", pc.resultEvent.TypeUrl)
}

func (pc *PlayerContext) eventHasNewBalance(balance int) error {
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}

	if pc.resultEvent.MessageIs(&examples.FundsDeposited{}) {
		var event examples.FundsDeposited
		if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.NewBalance.Amount != int64(balance) {
			return fmt.Errorf("expected new_balance=%d, got %d", balance, event.NewBalance.Amount)
		}
		return nil
	}
	if pc.resultEvent.MessageIs(&examples.FundsWithdrawn{}) {
		var event examples.FundsWithdrawn
		if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.NewBalance.Amount != int64(balance) {
			return fmt.Errorf("expected new_balance=%d, got %d", balance, event.NewBalance.Amount)
		}
		return nil
	}

	return fmt.Errorf("event type %s does not have new_balance", pc.resultEvent.TypeUrl)
}

func (pc *PlayerContext) eventHasNewAvailableBalance(balance int) error {
	if pc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}

	if pc.resultEvent.MessageIs(&examples.FundsReserved{}) {
		var event examples.FundsReserved
		if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.NewAvailableBalance.Amount != int64(balance) {
			return fmt.Errorf("expected new_available_balance=%d, got %d", balance, event.NewAvailableBalance.Amount)
		}
		return nil
	}
	if pc.resultEvent.MessageIs(&examples.FundsReleased{}) {
		var event examples.FundsReleased
		if err := pc.resultEvent.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.NewAvailableBalance.Amount != int64(balance) {
			return fmt.Errorf("expected new_available_balance=%d, got %d", balance, event.NewAvailableBalance.Amount)
		}
		return nil
	}

	return fmt.Errorf("event type %s does not have new_available_balance", pc.resultEvent.TypeUrl)
}

func (pc *PlayerContext) stateHasBankroll(amount int) error {
	if pc.state.Bankroll != int64(amount) {
		return fmt.Errorf("expected bankroll=%d, got %d", amount, pc.state.Bankroll)
	}
	return nil
}

func (pc *PlayerContext) stateHasReservedFunds(amount int) error {
	if pc.state.ReservedFunds != int64(amount) {
		return fmt.Errorf("expected reserved_funds=%d, got %d", amount, pc.state.ReservedFunds)
	}
	return nil
}

func (pc *PlayerContext) stateHasAvailableBalance(amount int) error {
	available := pc.state.AvailableBalance()
	if available != int64(amount) {
		return fmt.Errorf("expected available_balance=%d, got %d", amount, available)
	}
	return nil
}

// Ensure TableReservations key uses hex encoding consistent with handlers
func init() {
	_ = hex.EncodeToString([]byte{})
}
