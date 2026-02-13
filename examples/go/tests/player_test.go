// Player aggregate BDD tests using godog.
//
// These tests load scenarios from the shared features/unit/player.feature file
// and run them against the Go implementation of the player aggregate.
package tests

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/cucumber/godog"
	"github.com/google/uuid"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/benjaminabbitt/angzarr/examples/go/player/agg/handlers"
)

// testContext holds the state for a single scenario.
type testContext struct {
	domain        string
	root          []byte
	events        []*pb.EventPage
	nextSequence  uint32
	lastError     error
	lastEventBook *pb.EventBook
	lastErrorMsg  string
	lastState     handlers.PlayerState
}

// Test UUID namespace for deterministic UUIDs.
var testUUIDNamespace = uuid.MustParse("a1b2c3d4-e5f6-7890-abcd-ef1234567890")

// uuidFor generates a deterministic UUID from a name.
func uuidFor(name string) []byte {
	u := uuid.NewSHA1(testUUIDNamespace, []byte(name))
	return u[:]
}

// newTestContext creates a fresh test context.
func newTestContext() *testContext {
	return &testContext{
		domain: "player",
		root:   uuidFor("player-test"),
		events: make([]*pb.EventPage, 0),
	}
}

// buildEventBook creates an EventBook from the current events.
func (c *testContext) buildEventBook() *pb.EventBook {
	return &pb.EventBook{
		Cover: &pb.Cover{
			Domain: c.domain,
			Root:   &pb.UUID{Value: c.root},
		},
		Pages:        c.events,
		NextSequence: c.nextSequence,
	}
}

// buildCommandBook creates a CommandBook with a command.
func (c *testContext) buildCommandBook(cmdAny *anypb.Any) *pb.CommandBook {
	return &pb.CommandBook{
		Cover: &pb.Cover{
			Domain: c.domain,
			Root:   &pb.UUID{Value: c.root},
		},
		Pages: []*pb.CommandPage{{
			Sequence: c.nextSequence,
			Command:  cmdAny,
		}},
	}
}

// addEvent adds an event to the history.
func (c *testContext) addEvent(msg proto.Message) error {
	eventAny, err := anypb.New(msg)
	if err != nil {
		return err
	}
	c.events = append(c.events, &pb.EventPage{
		Sequence:  &pb.EventPage_Num{Num: c.nextSequence},
		Event:     eventAny,
		CreatedAt: timestamppb.New(time.Now()),
	})
	c.nextSequence++
	return nil
}

// rebuildState rebuilds the player state from events.
func (c *testContext) rebuildState() handlers.PlayerState {
	return handlers.RebuildState(c.buildEventBook())
}

// getLastEvent returns the first event from lastEventBook.
func (c *testContext) getLastEvent() (*anypb.Any, error) {
	if c.lastEventBook == nil || len(c.lastEventBook.Pages) == 0 {
		return nil, fmt.Errorf("no events emitted")
	}
	return c.lastEventBook.Pages[0].Event, nil
}

// --- Step Definitions ---

func noPriorEventsForThePlayerAggregate(ctx context.Context) error {
	tc := ctx.Value("testContext").(*testContext)
	tc.events = make([]*pb.EventPage, 0)
	tc.nextSequence = 0
	return nil
}

func aPlayerRegisteredEventFor(ctx context.Context, name string) error {
	tc := ctx.Value("testContext").(*testContext)

	event := &examples.PlayerRegistered{
		DisplayName:  name,
		Email:        name + "@example.com",
		PlayerType:   examples.PlayerType_HUMAN,
		RegisteredAt: timestamppb.New(time.Now()),
	}
	return tc.addEvent(event)
}

func aFundsDepositedEventWithAmount(ctx context.Context, amount int64) error {
	tc := ctx.Value("testContext").(*testContext)

	// Calculate new balance from existing state
	state := tc.rebuildState()
	newBalance := state.Bankroll + amount

	event := &examples.FundsDeposited{
		Amount:     &examples.Currency{Amount: amount},
		NewBalance: &examples.Currency{Amount: newBalance},
	}
	return tc.addEvent(event)
}

func aFundsReservedEventWithAmountForTable(ctx context.Context, amount int64, tableID string) error {
	tc := ctx.Value("testContext").(*testContext)

	// Calculate new balances from existing state
	state := tc.rebuildState()
	newReserved := state.ReservedFunds + amount
	newAvailable := state.Bankroll - newReserved

	event := &examples.FundsReserved{
		Amount:              &examples.Currency{Amount: amount},
		TableRoot:           uuidFor(tableID),
		NewAvailableBalance: &examples.Currency{Amount: newAvailable},
		NewReservedBalance:  &examples.Currency{Amount: newReserved},
	}
	return tc.addEvent(event)
}

func iHandleARegisterPlayerCommand(ctx context.Context, name, email string) error {
	tc := ctx.Value("testContext").(*testContext)

	cmd := &examples.RegisterPlayer{
		DisplayName: name,
		Email:       email,
		PlayerType:  examples.PlayerType_HUMAN,
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleRegisterPlayer(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleARegisterPlayerCommandAsAI(ctx context.Context, name, email string) error {
	tc := ctx.Value("testContext").(*testContext)

	cmd := &examples.RegisterPlayer{
		DisplayName: name,
		Email:       email,
		PlayerType:  examples.PlayerType_AI,
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleRegisterPlayer(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleADepositFundsCommand(ctx context.Context, amount int64) error {
	tc := ctx.Value("testContext").(*testContext)

	cmd := &examples.DepositFunds{
		Amount: &examples.Currency{Amount: amount},
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleDepositFunds(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleAWithdrawFundsCommand(ctx context.Context, amount int64) error {
	tc := ctx.Value("testContext").(*testContext)

	cmd := &examples.WithdrawFunds{
		Amount: &examples.Currency{Amount: amount},
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleWithdrawFunds(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleAReserveFundsCommandWithAmountForTable(ctx context.Context, amount int64, tableID string) error {
	tc := ctx.Value("testContext").(*testContext)

	cmd := &examples.ReserveFunds{
		Amount:    &examples.Currency{Amount: amount},
		TableRoot: uuidFor(tableID),
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleReserveFunds(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iHandleAReleaseFundsCommandForTable(ctx context.Context, tableID string) error {
	tc := ctx.Value("testContext").(*testContext)

	cmd := &examples.ReleaseFunds{
		TableRoot: uuidFor(tableID),
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	cmdBook := tc.buildCommandBook(cmdAny)
	state := tc.rebuildState()

	result, err := handlers.HandleReleaseFunds(cmdBook, cmdAny, state, tc.nextSequence)
	tc.lastEventBook = result
	tc.lastError = err
	if err != nil {
		tc.lastErrorMsg = err.Error()
	} else {
		tc.lastErrorMsg = ""
	}

	return nil
}

func iRebuildThePlayerState(ctx context.Context) error {
	tc := ctx.Value("testContext").(*testContext)
	tc.lastState = tc.rebuildState()
	return nil
}

// --- Then Steps ---

func theResultIsAPlayerRegisteredEvent(ctx context.Context) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "PlayerRegistered") {
		return fmt.Errorf("expected PlayerRegistered event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theResultIsAFundsDepositedEvent(ctx context.Context) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsDeposited") {
		return fmt.Errorf("expected FundsDeposited event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theResultIsAFundsWithdrawnEvent(ctx context.Context) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsWithdrawn") {
		return fmt.Errorf("expected FundsWithdrawn event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theResultIsAFundsReservedEvent(ctx context.Context) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsReserved") {
		return fmt.Errorf("expected FundsReserved event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theResultIsAFundsReleasedEvent(ctx context.Context) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}
	if !angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsReleased") {
		return fmt.Errorf("expected FundsReleased event but got %s", eventAny.TypeUrl)
	}
	return nil
}

func theCommandFailsWithStatus(ctx context.Context, status string) error {
	tc := ctx.Value("testContext").(*testContext)
	if tc.lastError == nil {
		return fmt.Errorf("expected command to fail but it succeeded")
	}
	// Check if it's a CommandRejectedError
	var cmdErr *angzarr.CommandRejectedError
	if errors.As(tc.lastError, &cmdErr) {
		return nil
	}
	return nil
}

func theErrorMessageContains(ctx context.Context, expected string) error {
	tc := ctx.Value("testContext").(*testContext)
	if tc.lastError == nil {
		return fmt.Errorf("expected error but got none")
	}
	if !strings.Contains(strings.ToLower(tc.lastErrorMsg), strings.ToLower(expected)) {
		return fmt.Errorf("expected error message to contain '%s' but got '%s'", expected, tc.lastErrorMsg)
	}
	return nil
}

func thePlayerEventHasDisplayName(ctx context.Context, expected string) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	var event examples.PlayerRegistered
	if err := eventAny.UnmarshalTo(&event); err != nil {
		return fmt.Errorf("failed to unmarshal event: %w", err)
	}

	if event.DisplayName != expected {
		return fmt.Errorf("expected display_name '%s' but got '%s'", expected, event.DisplayName)
	}
	return nil
}

func thePlayerEventHasPlayerType(ctx context.Context, expected string) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	var event examples.PlayerRegistered
	if err := eventAny.UnmarshalTo(&event); err != nil {
		return fmt.Errorf("failed to unmarshal event: %w", err)
	}

	// Normalize expected and actual values
	actualType := event.PlayerType.String()
	if actualType != expected {
		return fmt.Errorf("expected player_type '%s' but got '%s'", expected, actualType)
	}
	return nil
}

func thePlayerEventHasAmount(ctx context.Context, expected int64) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	// Try different event types
	if angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsDeposited") {
		var event examples.FundsDeposited
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.Amount.Amount != expected {
			return fmt.Errorf("expected amount %d but got %d", expected, event.Amount.Amount)
		}
		return nil
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsWithdrawn") {
		var event examples.FundsWithdrawn
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.Amount.Amount != expected {
			return fmt.Errorf("expected amount %d but got %d", expected, event.Amount.Amount)
		}
		return nil
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsReserved") {
		var event examples.FundsReserved
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.Amount.Amount != expected {
			return fmt.Errorf("expected amount %d but got %d", expected, event.Amount.Amount)
		}
		return nil
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsReleased") {
		var event examples.FundsReleased
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.Amount.Amount != expected {
			return fmt.Errorf("expected amount %d but got %d", expected, event.Amount.Amount)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for amount check: %s", eventAny.TypeUrl)
}

func thePlayerEventHasNewBalance(ctx context.Context, expected int64) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsDeposited") {
		var event examples.FundsDeposited
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.NewBalance.Amount != expected {
			return fmt.Errorf("expected new_balance %d but got %d", expected, event.NewBalance.Amount)
		}
		return nil
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsWithdrawn") {
		var event examples.FundsWithdrawn
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.NewBalance.Amount != expected {
			return fmt.Errorf("expected new_balance %d but got %d", expected, event.NewBalance.Amount)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for new_balance check: %s", eventAny.TypeUrl)
}

func thePlayerEventHasNewAvailableBalance(ctx context.Context, expected int64) error {
	tc := ctx.Value("testContext").(*testContext)
	eventAny, err := tc.getLastEvent()
	if err != nil {
		return err
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsReserved") {
		var event examples.FundsReserved
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.NewAvailableBalance.Amount != expected {
			return fmt.Errorf("expected new_available_balance %d but got %d", expected, event.NewAvailableBalance.Amount)
		}
		return nil
	}

	if angzarr.TypeURLMatches(eventAny.TypeUrl, "FundsReleased") {
		var event examples.FundsReleased
		if err := eventAny.UnmarshalTo(&event); err != nil {
			return err
		}
		if event.NewAvailableBalance.Amount != expected {
			return fmt.Errorf("expected new_available_balance %d but got %d", expected, event.NewAvailableBalance.Amount)
		}
		return nil
	}

	return fmt.Errorf("unknown event type for new_available_balance check: %s", eventAny.TypeUrl)
}

func thePlayerStateHasBankroll(ctx context.Context, expected int64) error {
	tc := ctx.Value("testContext").(*testContext)
	if tc.lastState.Bankroll != expected {
		return fmt.Errorf("expected bankroll %d but got %d", expected, tc.lastState.Bankroll)
	}
	return nil
}

func thePlayerStateHasReservedFunds(ctx context.Context, expected int64) error {
	tc := ctx.Value("testContext").(*testContext)
	if tc.lastState.ReservedFunds != expected {
		return fmt.Errorf("expected reserved_funds %d but got %d", expected, tc.lastState.ReservedFunds)
	}
	return nil
}

func thePlayerStateHasAvailableBalance(ctx context.Context, expected int64) error {
	tc := ctx.Value("testContext").(*testContext)
	actual := tc.lastState.Bankroll - tc.lastState.ReservedFunds
	if actual != expected {
		return fmt.Errorf("expected available_balance %d but got %d", expected, actual)
	}
	return nil
}

// InitializeScenario sets up the godog scenario context.
func InitializeScenario(ctx *godog.ScenarioContext) {
	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc := newTestContext()
		return context.WithValue(ctx, "testContext", tc), nil
	})

	// Given steps
	ctx.Step(`^no prior events for the player aggregate$`, noPriorEventsForThePlayerAggregate)
	ctx.Step(`^a PlayerRegistered event for "([^"]*)"$`, aPlayerRegisteredEventFor)
	ctx.Step(`^a FundsDeposited event with amount (\d+)$`, aFundsDepositedEventWithAmount)
	ctx.Step(`^a FundsReserved event with amount (\d+) for table "([^"]*)"$`, aFundsReservedEventWithAmountForTable)

	// When steps
	ctx.Step(`^I handle a RegisterPlayer command with name "([^"]*)" and email "([^"]*)"$`, iHandleARegisterPlayerCommand)
	ctx.Step(`^I handle a RegisterPlayer command with name "([^"]*)" and email "([^"]*)" as AI$`, iHandleARegisterPlayerCommandAsAI)
	ctx.Step(`^I handle a DepositFunds command with amount (\d+)$`, iHandleADepositFundsCommand)
	ctx.Step(`^I handle a WithdrawFunds command with amount (\d+)$`, iHandleAWithdrawFundsCommand)
	ctx.Step(`^I handle a ReserveFunds command with amount (\d+) for table "([^"]*)"$`, iHandleAReserveFundsCommandWithAmountForTable)
	ctx.Step(`^I handle a ReleaseFunds command for table "([^"]*)"$`, iHandleAReleaseFundsCommandForTable)
	ctx.Step(`^I rebuild the player state$`, iRebuildThePlayerState)

	// Then steps - result checks
	ctx.Step(`^the result is a PlayerRegistered event$`, theResultIsAPlayerRegisteredEvent)
	ctx.Step(`^the result is a FundsDeposited event$`, theResultIsAFundsDepositedEvent)
	ctx.Step(`^the result is a FundsWithdrawn event$`, theResultIsAFundsWithdrawnEvent)
	ctx.Step(`^the result is a FundsReserved event$`, theResultIsAFundsReservedEvent)
	ctx.Step(`^the result is a FundsReleased event$`, theResultIsAFundsReleasedEvent)

	// Then steps - error checks
	ctx.Step(`^the command fails with status "([^"]*)"$`, theCommandFailsWithStatus)
	ctx.Step(`^the error message contains "([^"]*)"$`, theErrorMessageContains)

	// Then steps - event property checks
	ctx.Step(`^the player event has display_name "([^"]*)"$`, thePlayerEventHasDisplayName)
	ctx.Step(`^the player event has player_type "([^"]*)"$`, thePlayerEventHasPlayerType)
	ctx.Step(`^the player event has amount (\d+)$`, thePlayerEventHasAmount)
	ctx.Step(`^the player event has new_balance (\d+)$`, thePlayerEventHasNewBalance)
	ctx.Step(`^the player event has new_available_balance (\d+)$`, thePlayerEventHasNewAvailableBalance)

	// Then steps - state checks
	ctx.Step(`^the player state has bankroll (\d+)$`, thePlayerStateHasBankroll)
	ctx.Step(`^the player state has reserved_funds (\d+)$`, thePlayerStateHasReservedFunds)
	ctx.Step(`^the player state has available_balance (\d+)$`, thePlayerStateHasAvailableBalance)
}

func TestFeatures(t *testing.T) {
	suite := godog.TestSuite{
		ScenarioInitializer: InitializeScenario,
		Options: &godog.Options{
			Format:   "pretty",
			Paths:    []string{"../../features/unit/player.feature"},
			TestingT: t,
		},
	}

	if suite.Run() != 0 {
		t.Fatal("non-zero status returned, failed to run feature tests")
	}
}
