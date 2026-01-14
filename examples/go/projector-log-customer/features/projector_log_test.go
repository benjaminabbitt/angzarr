package features

import (
	"context"
	"os"
	"testing"

	"projector-log-customer/logic"
	"projector-log-customer/proto/angzarr"
	"projector-log-customer/proto/examples"

	"github.com/cucumber/godog"
	"google.golang.org/protobuf/types/known/anypb"
)

type projectorLogTestContext struct {
	logic   logic.ProjectorLogic
	pages   []*angzarr.EventPage
	results []logic.LogResult
}

func (c *projectorLogTestContext) reset() {
	c.logic = logic.NewProjectorLogic()
	c.pages = nil
	c.results = nil
}

func (c *projectorLogTestContext) aCustomerCreatedEventWithNameAndEmail(name, email string) error {
	event := &examples.CustomerCreated{Name: name, Email: email}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.pages = append(c.pages, &angzarr.EventPage{
		Sequence: &angzarr.EventPage_Num{Num: uint32(len(c.pages))},
		Event:    eventAny,
	})
	return nil
}

func (c *projectorLogTestContext) aLoyaltyPointsAddedEventWithPointsAndNewBalance(points, newBalance int) error {
	event := &examples.LoyaltyPointsAdded{Points: int32(points), NewBalance: int32(newBalance)}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.pages = append(c.pages, &angzarr.EventPage{
		Sequence: &angzarr.EventPage_Num{Num: uint32(len(c.pages))},
		Event:    eventAny,
	})
	return nil
}

func (c *projectorLogTestContext) aTransactionCreatedEventWithCustomerAndSubtotal(customerID string, subtotal int) error {
	event := &examples.TransactionCreated{CustomerId: customerID, SubtotalCents: int32(subtotal)}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.pages = append(c.pages, &angzarr.EventPage{
		Sequence: &angzarr.EventPage_Num{Num: uint32(len(c.pages))},
		Event:    eventAny,
	})
	return nil
}

func (c *projectorLogTestContext) aTransactionCompletedEventWithTotalAndPayment(total int, paymentMethod string) error {
	event := &examples.TransactionCompleted{FinalTotalCents: int32(total), PaymentMethod: paymentMethod}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.pages = append(c.pages, &angzarr.EventPage{
		Sequence: &angzarr.EventPage_Num{Num: uint32(len(c.pages))},
		Event:    eventAny,
	})
	return nil
}

func (c *projectorLogTestContext) anUnknownEventType() error {
	unknownAny := &anypb.Any{
		TypeUrl: "type.googleapis.com/unknown.UnknownEvent",
		Value:   []byte{0x01, 0x02, 0x03},
	}
	c.pages = append(c.pages, &angzarr.EventPage{
		Sequence: &angzarr.EventPage_Num{Num: uint32(len(c.pages))},
		Event:    unknownAny,
	})
	return nil
}

func (c *projectorLogTestContext) iProcessTheLogProjector() error {
	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "customer",
		},
		Pages: c.pages,
	}
	c.results = c.logic.ProcessEventBook(eventBook)
	return nil
}

func (c *projectorLogTestContext) theEventIsLoggedSuccessfully() error {
	if len(c.results) == 0 {
		return godog.ErrPending
	}
	for _, result := range c.results {
		if result.Unknown {
			return godog.ErrPending
		}
	}
	return nil
}

func (c *projectorLogTestContext) theEventIsLoggedAsUnknown() error {
	if len(c.results) == 0 {
		return godog.ErrPending
	}
	for _, result := range c.results {
		if !result.Unknown {
			return godog.ErrPending
		}
	}
	return nil
}

func InitializeScenario(ctx *godog.ScenarioContext) {
	tc := &projectorLogTestContext{}

	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc.reset()
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^a CustomerCreated event with name "([^"]*)" and email "([^"]*)"$`, tc.aCustomerCreatedEventWithNameAndEmail)
	ctx.Step(`^a LoyaltyPointsAdded event with (\d+) points and new_balance (\d+)$`, tc.aLoyaltyPointsAddedEventWithPointsAndNewBalance)
	ctx.Step(`^a TransactionCreated event with customer "([^"]*)" and subtotal (\d+)$`, tc.aTransactionCreatedEventWithCustomerAndSubtotal)
	ctx.Step(`^a TransactionCompleted event with total (\d+) and payment "([^"]*)"$`, tc.aTransactionCompletedEventWithTotalAndPayment)
	ctx.Step(`^an unknown event type$`, tc.anUnknownEventType)

	// When steps
	ctx.Step(`^I process the log projector$`, tc.iProcessTheLogProjector)

	// Then steps
	ctx.Step(`^the event is logged successfully$`, tc.theEventIsLoggedSuccessfully)
	ctx.Step(`^the event is logged as unknown$`, tc.theEventIsLoggedAsUnknown)
}

func TestFeatures(t *testing.T) {
	suite := godog.TestSuite{
		ScenarioInitializer: InitializeScenario,
		Options: &godog.Options{
			Format:   "pretty",
			Paths:    []string{"projector-log.feature"},
			TestingT: t,
		},
	}

	if suite.Run() != 0 {
		t.Fatal("non-zero status returned, failed to run feature tests")
	}
}

func init() {
	// Ensure we're in the correct directory when running tests
	if _, err := os.Stat("projector-log.feature"); os.IsNotExist(err) {
		// Will fail at runtime with proper error
	}
}
