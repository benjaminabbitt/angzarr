package features

import (
	"context"
	"errors"
	"fmt"
	"os"
	"strings"
	"testing"

	"saga-loyalty/logic"
	"saga-loyalty/proto/angzarr"
	"saga-loyalty/proto/examples"

	"github.com/cucumber/godog"
	"google.golang.org/protobuf/types/known/anypb"
)

type sagaTestContext struct {
	logic       logic.SagaLogic
	priorEvents []*anypb.Any
	commands    []*logic.SagaCommand
	eventBook   *angzarr.EventBook
}

func (c *sagaTestContext) reset() {
	c.logic = logic.NewSagaLogic()
	c.priorEvents = nil
	c.commands = nil
	c.eventBook = nil
}

func (c *sagaTestContext) aTransactionCreatedEventWithCustomerAndSubtotal(customerID string, subtotal int) error {
	event := &examples.TransactionCreated{
		CustomerId:    customerID,
		SubtotalCents: int32(subtotal),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *sagaTestContext) aTransactionCompletedEventWithLoyaltyPointsEarned(points int) error {
	event := &examples.TransactionCompleted{
		LoyaltyPointsEarned: int32(points),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *sagaTestContext) buildEventBook() *angzarr.EventBook {
	if len(c.priorEvents) == 0 {
		return nil
	}
	pages := make([]*angzarr.EventPage, len(c.priorEvents))
	for i, event := range c.priorEvents {
		pages[i] = &angzarr.EventPage{Event: event}
	}
	return &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
			Root:   &angzarr.UUID{Value: []byte("txn-test-001")},
		},
		Pages: pages,
	}
}

func (c *sagaTestContext) iProcessTheSaga() error {
	c.eventBook = c.buildEventBook()
	c.commands = c.logic.ProcessEvents(c.eventBook)
	return nil
}

func (c *sagaTestContext) noCommandsAreGenerated() error {
	if len(c.commands) != 0 {
		return fmt.Errorf("expected no commands, got %d", len(c.commands))
	}
	return nil
}

func (c *sagaTestContext) anAddLoyaltyPointsCommandIsGenerated() error {
	if len(c.commands) == 0 {
		return errors.New("expected AddLoyaltyPoints command but none were generated")
	}
	if c.commands[0].Command == nil {
		return errors.New("expected AddLoyaltyPoints command but command is nil")
	}
	return nil
}

func (c *sagaTestContext) theCommandHasPoints(points int) error {
	if len(c.commands) == 0 {
		return errors.New("no commands generated")
	}
	cmd := c.commands[0]
	if int(cmd.Command.Points) != points {
		return fmt.Errorf("expected points %d, got %d", points, cmd.Command.Points)
	}
	return nil
}

func (c *sagaTestContext) theCommandHasDomain(domain string) error {
	if len(c.commands) == 0 {
		return errors.New("no commands generated")
	}
	cmd := c.commands[0]
	if cmd.Domain != domain {
		return fmt.Errorf("expected domain %q, got %q", domain, cmd.Domain)
	}
	return nil
}

func (c *sagaTestContext) theCommandReasonContains(substring string) error {
	if len(c.commands) == 0 {
		return errors.New("no commands generated")
	}
	cmd := c.commands[0]
	if !strings.Contains(cmd.Command.Reason, substring) {
		return fmt.Errorf("expected reason to contain %q, got %q", substring, cmd.Command.Reason)
	}
	return nil
}

func InitializeScenario(ctx *godog.ScenarioContext) {
	tc := &sagaTestContext{}

	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc.reset()
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^a TransactionCreated event with customer "([^"]*)" and subtotal (\d+)$`, tc.aTransactionCreatedEventWithCustomerAndSubtotal)
	ctx.Step(`^a TransactionCompleted event with (\d+) loyalty points earned$`, tc.aTransactionCompletedEventWithLoyaltyPointsEarned)

	// When steps
	ctx.Step(`^I process the saga$`, tc.iProcessTheSaga)

	// Then steps
	ctx.Step(`^no commands are generated$`, tc.noCommandsAreGenerated)
	ctx.Step(`^an AddLoyaltyPoints command is generated$`, tc.anAddLoyaltyPointsCommandIsGenerated)
	ctx.Step(`^the command has points (\d+)$`, tc.theCommandHasPoints)
	ctx.Step(`^the command has domain "([^"]*)"$`, tc.theCommandHasDomain)
	ctx.Step(`^the command reason contains "([^"]*)"$`, tc.theCommandReasonContains)
}

func TestFeatures(t *testing.T) {
	suite := godog.TestSuite{
		ScenarioInitializer: InitializeScenario,
		Options: &godog.Options{
			Format:   "pretty",
			Paths:    []string{"../../../features/saga-loyalty.feature"},
			TestingT: t,
		},
	}

	if suite.Run() != 0 {
		t.Fatal("non-zero status returned, failed to run feature tests")
	}
}

func init() {
	// Ensure we're in the correct directory when running tests
	if _, err := os.Stat("../../../features/saga-loyalty.feature"); os.IsNotExist(err) {
		// Try from the features directory itself
		if _, err := os.Stat("saga-loyalty.feature"); os.IsNotExist(err) {
			// Will fail at runtime with proper error
		}
	}
}
