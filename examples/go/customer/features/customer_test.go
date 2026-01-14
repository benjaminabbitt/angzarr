package features

import (
	"context"
	"errors"
	"fmt"
	"os"
	"strings"
	"testing"

	"customer/logic"
	"customer/proto/angzarr"
	"customer/proto/examples"

	"github.com/cucumber/godog"
	"google.golang.org/protobuf/types/known/anypb"
)

type customerTestContext struct {
	logic       logic.CustomerLogic
	priorEvents []*anypb.Any
	resultEvent interface{}
	err         error
	state       *logic.CustomerState
}

func (c *customerTestContext) reset() {
	c.logic = logic.NewCustomerLogic()
	c.priorEvents = nil
	c.resultEvent = nil
	c.err = nil
	c.state = nil
}

func (c *customerTestContext) noPriorEventsForTheAggregate() error {
	c.priorEvents = nil
	return nil
}

func (c *customerTestContext) aCustomerCreatedEventWithNameAndEmail(name, email string) error {
	event := &examples.CustomerCreated{Name: name, Email: email}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *customerTestContext) aLoyaltyPointsAddedEventWithPointsAndNewBalance(points, newBalance int) error {
	event := &examples.LoyaltyPointsAdded{Points: int32(points), NewBalance: int32(newBalance)}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *customerTestContext) aLoyaltyPointsRedeemedEventWithPointsAndNewBalance(points, newBalance int) error {
	event := &examples.LoyaltyPointsRedeemed{Points: int32(points), NewBalance: int32(newBalance)}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *customerTestContext) buildEventBook() *angzarr.EventBook {
	if len(c.priorEvents) == 0 {
		return nil
	}
	pages := make([]*angzarr.EventPage, len(c.priorEvents))
	for i, event := range c.priorEvents {
		pages[i] = &angzarr.EventPage{Event: event}
	}
	return &angzarr.EventBook{Pages: pages}
}

func (c *customerTestContext) iHandleACreateCustomerCommandWithNameAndEmail(name, email string) error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.resultEvent, c.err = c.logic.HandleCreateCustomer(c.state, name, email)
	return nil
}

func (c *customerTestContext) iHandleAnAddLoyaltyPointsCommandWithPointsAndReason(points int, reason string) error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.resultEvent, c.err = c.logic.HandleAddLoyaltyPoints(c.state, int32(points), reason)
	return nil
}

func (c *customerTestContext) iHandleARedeemLoyaltyPointsCommandWithPointsAndType(points int, redemptionType string) error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.resultEvent, c.err = c.logic.HandleRedeemLoyaltyPoints(c.state, int32(points), redemptionType)
	return nil
}

func (c *customerTestContext) iRebuildTheCustomerState() error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	return nil
}

func (c *customerTestContext) theResultIsACustomerCreatedEvent() error {
	if c.err != nil {
		return fmt.Errorf("expected result but got error: %v", c.err)
	}
	if _, ok := c.resultEvent.(*examples.CustomerCreated); !ok {
		return errors.New("expected CustomerCreated event")
	}
	return nil
}

func (c *customerTestContext) theResultIsALoyaltyPointsAddedEvent() error {
	if c.err != nil {
		return fmt.Errorf("expected result but got error: %v", c.err)
	}
	if _, ok := c.resultEvent.(*examples.LoyaltyPointsAdded); !ok {
		return errors.New("expected LoyaltyPointsAdded event")
	}
	return nil
}

func (c *customerTestContext) theResultIsALoyaltyPointsRedeemedEvent() error {
	if c.err != nil {
		return fmt.Errorf("expected result but got error: %v", c.err)
	}
	if _, ok := c.resultEvent.(*examples.LoyaltyPointsRedeemed); !ok {
		return errors.New("expected LoyaltyPointsRedeemed event")
	}
	return nil
}

func (c *customerTestContext) theCommandFailsWithStatus(statusName string) error {
	if c.err == nil {
		return errors.New("expected command to fail but it succeeded")
	}
	cmdErr, ok := c.err.(*logic.CommandError)
	if !ok {
		return fmt.Errorf("expected CommandError, got %T", c.err)
	}
	expectedCode := statusName
	if cmdErr.Code.String() != expectedCode {
		return fmt.Errorf("expected status %s, got %s", expectedCode, cmdErr.Code.String())
	}
	return nil
}

func (c *customerTestContext) theErrorMessageContains(substring string) error {
	if c.err == nil {
		return errors.New("expected error but command succeeded")
	}
	if !strings.Contains(strings.ToLower(c.err.Error()), strings.ToLower(substring)) {
		return fmt.Errorf("expected error message to contain %q, got %q", substring, c.err.Error())
	}
	return nil
}

func (c *customerTestContext) theEventHasName(name string) error {
	event, ok := c.resultEvent.(*examples.CustomerCreated)
	if !ok {
		return errors.New("expected CustomerCreated event")
	}
	if event.Name != name {
		return fmt.Errorf("expected name %q, got %q", name, event.Name)
	}
	return nil
}

func (c *customerTestContext) theEventHasEmail(email string) error {
	event, ok := c.resultEvent.(*examples.CustomerCreated)
	if !ok {
		return errors.New("expected CustomerCreated event")
	}
	if event.Email != email {
		return fmt.Errorf("expected email %q, got %q", email, event.Email)
	}
	return nil
}

func (c *customerTestContext) theEventHasPoints(points int) error {
	switch e := c.resultEvent.(type) {
	case *examples.LoyaltyPointsAdded:
		if int(e.Points) != points {
			return fmt.Errorf("expected points %d, got %d", points, e.Points)
		}
	case *examples.LoyaltyPointsRedeemed:
		if int(e.Points) != points {
			return fmt.Errorf("expected points %d, got %d", points, e.Points)
		}
	default:
		return fmt.Errorf("expected points event, got %T", c.resultEvent)
	}
	return nil
}

func (c *customerTestContext) theEventHasNewBalance(newBalance int) error {
	switch e := c.resultEvent.(type) {
	case *examples.LoyaltyPointsAdded:
		if int(e.NewBalance) != newBalance {
			return fmt.Errorf("expected new_balance %d, got %d", newBalance, e.NewBalance)
		}
	case *examples.LoyaltyPointsRedeemed:
		if int(e.NewBalance) != newBalance {
			return fmt.Errorf("expected new_balance %d, got %d", newBalance, e.NewBalance)
		}
	default:
		return fmt.Errorf("expected points event, got %T", c.resultEvent)
	}
	return nil
}

func (c *customerTestContext) theEventHasReason(reason string) error {
	event, ok := c.resultEvent.(*examples.LoyaltyPointsAdded)
	if !ok {
		return errors.New("expected LoyaltyPointsAdded event")
	}
	if event.Reason != reason {
		return fmt.Errorf("expected reason %q, got %q", reason, event.Reason)
	}
	return nil
}

func (c *customerTestContext) theEventHasRedemptionType(redemptionType string) error {
	event, ok := c.resultEvent.(*examples.LoyaltyPointsRedeemed)
	if !ok {
		return errors.New("expected LoyaltyPointsRedeemed event")
	}
	if event.RedemptionType != redemptionType {
		return fmt.Errorf("expected redemption_type %q, got %q", redemptionType, event.RedemptionType)
	}
	return nil
}

func (c *customerTestContext) theStateHasName(name string) error {
	if c.state == nil {
		return errors.New("state is nil")
	}
	if c.state.Name != name {
		return fmt.Errorf("expected name %q, got %q", name, c.state.Name)
	}
	return nil
}

func (c *customerTestContext) theStateHasEmail(email string) error {
	if c.state == nil {
		return errors.New("state is nil")
	}
	if c.state.Email != email {
		return fmt.Errorf("expected email %q, got %q", email, c.state.Email)
	}
	return nil
}

func (c *customerTestContext) theStateHasLoyaltyPoints(points int) error {
	if c.state == nil {
		return errors.New("state is nil")
	}
	if int(c.state.LoyaltyPoints) != points {
		return fmt.Errorf("expected loyalty_points %d, got %d", points, c.state.LoyaltyPoints)
	}
	return nil
}

func (c *customerTestContext) theStateHasLifetimePoints(points int) error {
	if c.state == nil {
		return errors.New("state is nil")
	}
	if int(c.state.LifetimePoints) != points {
		return fmt.Errorf("expected lifetime_points %d, got %d", points, c.state.LifetimePoints)
	}
	return nil
}

func InitializeScenario(ctx *godog.ScenarioContext) {
	tc := &customerTestContext{}

	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc.reset()
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^no prior events for the aggregate$`, tc.noPriorEventsForTheAggregate)
	ctx.Step(`^a CustomerCreated event with name "([^"]*)" and email "([^"]*)"$`, tc.aCustomerCreatedEventWithNameAndEmail)
	ctx.Step(`^a LoyaltyPointsAdded event with (\d+) points and new_balance (\d+)$`, tc.aLoyaltyPointsAddedEventWithPointsAndNewBalance)
	ctx.Step(`^a LoyaltyPointsRedeemed event with (\d+) points and new_balance (\d+)$`, tc.aLoyaltyPointsRedeemedEventWithPointsAndNewBalance)

	// When steps
	ctx.Step(`^I handle a CreateCustomer command with name "([^"]*)" and email "([^"]*)"$`, tc.iHandleACreateCustomerCommandWithNameAndEmail)
	ctx.Step(`^I handle an AddLoyaltyPoints command with (-?\d+) points and reason "([^"]*)"$`, tc.iHandleAnAddLoyaltyPointsCommandWithPointsAndReason)
	ctx.Step(`^I handle a RedeemLoyaltyPoints command with (\d+) points and type "([^"]*)"$`, tc.iHandleARedeemLoyaltyPointsCommandWithPointsAndType)
	ctx.Step(`^I rebuild the customer state$`, tc.iRebuildTheCustomerState)

	// Then steps
	ctx.Step(`^the result is a CustomerCreated event$`, tc.theResultIsACustomerCreatedEvent)
	ctx.Step(`^the result is a LoyaltyPointsAdded event$`, tc.theResultIsALoyaltyPointsAddedEvent)
	ctx.Step(`^the result is a LoyaltyPointsRedeemed event$`, tc.theResultIsALoyaltyPointsRedeemedEvent)
	ctx.Step(`^the command fails with status "([^"]*)"$`, tc.theCommandFailsWithStatus)
	ctx.Step(`^the error message contains "([^"]*)"$`, tc.theErrorMessageContains)
	ctx.Step(`^the event has name "([^"]*)"$`, tc.theEventHasName)
	ctx.Step(`^the event has email "([^"]*)"$`, tc.theEventHasEmail)
	ctx.Step(`^the event has points (\d+)$`, tc.theEventHasPoints)
	ctx.Step(`^the event has new_balance (\d+)$`, tc.theEventHasNewBalance)
	ctx.Step(`^the event has reason "([^"]*)"$`, tc.theEventHasReason)
	ctx.Step(`^the event has redemption_type "([^"]*)"$`, tc.theEventHasRedemptionType)
	ctx.Step(`^the state has name "([^"]*)"$`, tc.theStateHasName)
	ctx.Step(`^the state has email "([^"]*)"$`, tc.theStateHasEmail)
	ctx.Step(`^the state has loyalty_points (\d+)$`, tc.theStateHasLoyaltyPoints)
	ctx.Step(`^the state has lifetime_points (\d+)$`, tc.theStateHasLifetimePoints)
}

func TestFeatures(t *testing.T) {
	suite := godog.TestSuite{
		ScenarioInitializer: InitializeScenario,
		Options: &godog.Options{
			Format:   "pretty",
			Paths:    []string{"../../../features/customer.feature"},
			TestingT: t,
		},
	}

	if suite.Run() != 0 {
		t.Fatal("non-zero status returned, failed to run feature tests")
	}
}

func init() {
	// Ensure we're in the correct directory when running tests
	if _, err := os.Stat("../../../features/customer.feature"); os.IsNotExist(err) {
		// Try from the features directory itself
		if _, err := os.Stat("customer.feature"); os.IsNotExist(err) {
			// Will fail at runtime with proper error
		}
	}
}
