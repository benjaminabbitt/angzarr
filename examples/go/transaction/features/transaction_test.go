package features

import (
	"context"
	"errors"
	"fmt"
	"os"
	"strings"
	"testing"

	"transaction/logic"
	"transaction/proto/angzarr"
	"transaction/proto/examples"

	"github.com/cucumber/godog"
	"google.golang.org/protobuf/types/known/anypb"
)

type transactionTestContext struct {
	logic       logic.TransactionLogic
	priorEvents []*anypb.Any
	resultEvent interface{}
	err         error
	state       *logic.TransactionState
	items       []*examples.LineItem
}

func (c *transactionTestContext) reset() {
	c.logic = logic.NewTransactionLogic()
	c.priorEvents = nil
	c.resultEvent = nil
	c.err = nil
	c.state = nil
	c.items = nil
}

func (c *transactionTestContext) noPriorEventsForTheAggregate() error {
	c.priorEvents = nil
	return nil
}

func (c *transactionTestContext) aTransactionCreatedEventWithCustomerAndSubtotal(customerID string, subtotal int) error {
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

func (c *transactionTestContext) aDiscountAppliedEventWithCentsDiscount(discountCents int) error {
	event := &examples.DiscountApplied{
		DiscountCents: int32(discountCents),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *transactionTestContext) aTransactionCompletedEvent() error {
	event := &examples.TransactionCompleted{
		FinalTotalCents: 0,
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *transactionTestContext) aTransactionCancelledEvent() error {
	event := &examples.TransactionCancelled{
		Reason: "test",
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *transactionTestContext) buildEventBook() *angzarr.EventBook {
	if len(c.priorEvents) == 0 {
		return nil
	}
	pages := make([]*angzarr.EventPage, len(c.priorEvents))
	for i, event := range c.priorEvents {
		pages[i] = &angzarr.EventPage{Event: event}
	}
	return &angzarr.EventBook{Pages: pages}
}

func (c *transactionTestContext) iHandleACreateTransactionCommandWithCustomerAndItems(customerID string, itemsTable *godog.Table) error {
	items := []*examples.LineItem{}
	for i, row := range itemsTable.Rows {
		if i == 0 {
			continue // skip header
		}
		var productID, name string
		var quantity, unitPrice int
		fmt.Sscan(row.Cells[0].Value, &productID)
		fmt.Sscan(row.Cells[1].Value, &name)
		fmt.Sscan(row.Cells[2].Value, &quantity)
		fmt.Sscan(row.Cells[3].Value, &unitPrice)
		items = append(items, &examples.LineItem{
			ProductId:      row.Cells[0].Value,
			Name:           row.Cells[1].Value,
			Quantity:       int32(quantity),
			UnitPriceCents: int32(unitPrice),
		})
	}

	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.resultEvent, c.err = c.logic.HandleCreateTransaction(c.state, customerID, items)
	return nil
}

func (c *transactionTestContext) iHandleACreateTransactionCommandWithCustomerAndNoItems(customerID string) error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.resultEvent, c.err = c.logic.HandleCreateTransaction(c.state, customerID, nil)
	return nil
}

func (c *transactionTestContext) iHandleAnApplyDiscountCommandWithTypeAndValue(discountType string, value int) error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.resultEvent, c.err = c.logic.HandleApplyDiscount(c.state, discountType, int32(value), "")
	return nil
}

func (c *transactionTestContext) iHandleACompleteTransactionCommandWithPaymentMethod(paymentMethod string) error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.resultEvent, c.err = c.logic.HandleCompleteTransaction(c.state, paymentMethod)
	return nil
}

func (c *transactionTestContext) iHandleACancelTransactionCommandWithReason(reason string) error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.resultEvent, c.err = c.logic.HandleCancelTransaction(c.state, reason)
	return nil
}

func (c *transactionTestContext) iRebuildTheTransactionState() error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	return nil
}

func (c *transactionTestContext) theResultIsATransactionCreatedEvent() error {
	if c.err != nil {
		return fmt.Errorf("expected result but got error: %v", c.err)
	}
	if _, ok := c.resultEvent.(*examples.TransactionCreated); !ok {
		return errors.New("expected TransactionCreated event")
	}
	return nil
}

func (c *transactionTestContext) theResultIsADiscountAppliedEvent() error {
	if c.err != nil {
		return fmt.Errorf("expected result but got error: %v", c.err)
	}
	if _, ok := c.resultEvent.(*examples.DiscountApplied); !ok {
		return errors.New("expected DiscountApplied event")
	}
	return nil
}

func (c *transactionTestContext) theResultIsATransactionCompletedEvent() error {
	if c.err != nil {
		return fmt.Errorf("expected result but got error: %v", c.err)
	}
	if _, ok := c.resultEvent.(*examples.TransactionCompleted); !ok {
		return errors.New("expected TransactionCompleted event")
	}
	return nil
}

func (c *transactionTestContext) theResultIsATransactionCancelledEvent() error {
	if c.err != nil {
		return fmt.Errorf("expected result but got error: %v", c.err)
	}
	if _, ok := c.resultEvent.(*examples.TransactionCancelled); !ok {
		return errors.New("expected TransactionCancelled event")
	}
	return nil
}

func (c *transactionTestContext) theCommandFailsWithStatus(statusName string) error {
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

func (c *transactionTestContext) theErrorMessageContains(substring string) error {
	if c.err == nil {
		return errors.New("expected error but command succeeded")
	}
	if !strings.Contains(strings.ToLower(c.err.Error()), strings.ToLower(substring)) {
		return fmt.Errorf("expected error message to contain %q, got %q", substring, c.err.Error())
	}
	return nil
}

func (c *transactionTestContext) theEventHasCustomerId(customerID string) error {
	event, ok := c.resultEvent.(*examples.TransactionCreated)
	if !ok {
		return errors.New("expected TransactionCreated event")
	}
	if event.CustomerId != customerID {
		return fmt.Errorf("expected customer_id %q, got %q", customerID, event.CustomerId)
	}
	return nil
}

func (c *transactionTestContext) theEventHasSubtotalCents(subtotal int) error {
	event, ok := c.resultEvent.(*examples.TransactionCreated)
	if !ok {
		return errors.New("expected TransactionCreated event")
	}
	if int(event.SubtotalCents) != subtotal {
		return fmt.Errorf("expected subtotal_cents %d, got %d", subtotal, event.SubtotalCents)
	}
	return nil
}

func (c *transactionTestContext) theEventHasDiscountCents(discountCents int) error {
	event, ok := c.resultEvent.(*examples.DiscountApplied)
	if !ok {
		return errors.New("expected DiscountApplied event")
	}
	if int(event.DiscountCents) != discountCents {
		return fmt.Errorf("expected discount_cents %d, got %d", discountCents, event.DiscountCents)
	}
	return nil
}

func (c *transactionTestContext) theEventHasFinalTotalCents(finalTotal int) error {
	event, ok := c.resultEvent.(*examples.TransactionCompleted)
	if !ok {
		return errors.New("expected TransactionCompleted event")
	}
	if int(event.FinalTotalCents) != finalTotal {
		return fmt.Errorf("expected final_total_cents %d, got %d", finalTotal, event.FinalTotalCents)
	}
	return nil
}

func (c *transactionTestContext) theEventHasPaymentMethod(paymentMethod string) error {
	event, ok := c.resultEvent.(*examples.TransactionCompleted)
	if !ok {
		return errors.New("expected TransactionCompleted event")
	}
	if event.PaymentMethod != paymentMethod {
		return fmt.Errorf("expected payment_method %q, got %q", paymentMethod, event.PaymentMethod)
	}
	return nil
}

func (c *transactionTestContext) theEventHasLoyaltyPointsEarned(points int) error {
	event, ok := c.resultEvent.(*examples.TransactionCompleted)
	if !ok {
		return errors.New("expected TransactionCompleted event")
	}
	if int(event.LoyaltyPointsEarned) != points {
		return fmt.Errorf("expected loyalty_points_earned %d, got %d", points, event.LoyaltyPointsEarned)
	}
	return nil
}

func (c *transactionTestContext) theEventHasReason(reason string) error {
	event, ok := c.resultEvent.(*examples.TransactionCancelled)
	if !ok {
		return errors.New("expected TransactionCancelled event")
	}
	if event.Reason != reason {
		return fmt.Errorf("expected reason %q, got %q", reason, event.Reason)
	}
	return nil
}

func (c *transactionTestContext) theStateHasCustomerId(customerID string) error {
	if c.state == nil {
		return errors.New("state is nil")
	}
	if c.state.CustomerID != customerID {
		return fmt.Errorf("expected customer_id %q, got %q", customerID, c.state.CustomerID)
	}
	return nil
}

func (c *transactionTestContext) theStateHasSubtotalCents(subtotal int) error {
	if c.state == nil {
		return errors.New("state is nil")
	}
	if int(c.state.SubtotalCents) != subtotal {
		return fmt.Errorf("expected subtotal_cents %d, got %d", subtotal, c.state.SubtotalCents)
	}
	return nil
}

func (c *transactionTestContext) theStateHasStatus(status string) error {
	if c.state == nil {
		return errors.New("state is nil")
	}
	if c.state.Status != status {
		return fmt.Errorf("expected status %q, got %q", status, c.state.Status)
	}
	return nil
}

func InitializeScenario(ctx *godog.ScenarioContext) {
	tc := &transactionTestContext{}

	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc.reset()
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^no prior events for the aggregate$`, tc.noPriorEventsForTheAggregate)
	ctx.Step(`^a TransactionCreated event with customer "([^"]*)" and subtotal (\d+)$`, tc.aTransactionCreatedEventWithCustomerAndSubtotal)
	ctx.Step(`^a DiscountApplied event with (\d+) cents discount$`, tc.aDiscountAppliedEventWithCentsDiscount)
	ctx.Step(`^a TransactionCompleted event$`, tc.aTransactionCompletedEvent)
	ctx.Step(`^a TransactionCancelled event$`, tc.aTransactionCancelledEvent)

	// When steps
	ctx.Step(`^I handle a CreateTransaction command with customer "([^"]*)" and items:$`, tc.iHandleACreateTransactionCommandWithCustomerAndItems)
	ctx.Step(`^I handle a CreateTransaction command with customer "([^"]*)" and no items$`, tc.iHandleACreateTransactionCommandWithCustomerAndNoItems)
	ctx.Step(`^I handle an ApplyDiscount command with type "([^"]*)" and value (\d+)$`, tc.iHandleAnApplyDiscountCommandWithTypeAndValue)
	ctx.Step(`^I handle a CompleteTransaction command with payment method "([^"]*)"$`, tc.iHandleACompleteTransactionCommandWithPaymentMethod)
	ctx.Step(`^I handle a CancelTransaction command with reason "([^"]*)"$`, tc.iHandleACancelTransactionCommandWithReason)
	ctx.Step(`^I rebuild the transaction state$`, tc.iRebuildTheTransactionState)

	// Then steps
	ctx.Step(`^the result is a TransactionCreated event$`, tc.theResultIsATransactionCreatedEvent)
	ctx.Step(`^the result is a DiscountApplied event$`, tc.theResultIsADiscountAppliedEvent)
	ctx.Step(`^the result is a TransactionCompleted event$`, tc.theResultIsATransactionCompletedEvent)
	ctx.Step(`^the result is a TransactionCancelled event$`, tc.theResultIsATransactionCancelledEvent)
	ctx.Step(`^the command fails with status "([^"]*)"$`, tc.theCommandFailsWithStatus)
	ctx.Step(`^the error message contains "([^"]*)"$`, tc.theErrorMessageContains)
	ctx.Step(`^the event has customer_id "([^"]*)"$`, tc.theEventHasCustomerId)
	ctx.Step(`^the event has subtotal_cents (\d+)$`, tc.theEventHasSubtotalCents)
	ctx.Step(`^the event has discount_cents (\d+)$`, tc.theEventHasDiscountCents)
	ctx.Step(`^the event has final_total_cents (\d+)$`, tc.theEventHasFinalTotalCents)
	ctx.Step(`^the event has payment_method "([^"]*)"$`, tc.theEventHasPaymentMethod)
	ctx.Step(`^the event has loyalty_points_earned (\d+)$`, tc.theEventHasLoyaltyPointsEarned)
	ctx.Step(`^the event has reason "([^"]*)"$`, tc.theEventHasReason)
	ctx.Step(`^the state has customer_id "([^"]*)"$`, tc.theStateHasCustomerId)
	ctx.Step(`^the state has subtotal_cents (\d+)$`, tc.theStateHasSubtotalCents)
	ctx.Step(`^the state has status "([^"]*)"$`, tc.theStateHasStatus)
}

func TestFeatures(t *testing.T) {
	suite := godog.TestSuite{
		ScenarioInitializer: InitializeScenario,
		Options: &godog.Options{
			Format:   "pretty",
			Paths:    []string{"../../../features/transaction.feature"},
			TestingT: t,
		},
	}

	if suite.Run() != 0 {
		t.Fatal("non-zero status returned, failed to run feature tests")
	}
}

func init() {
	// Ensure we're in the correct directory when running tests
	if _, err := os.Stat("../../../features/transaction.feature"); os.IsNotExist(err) {
		// Try from the features directory itself
		if _, err := os.Stat("transaction.feature"); os.IsNotExist(err) {
			// Will fail at runtime with proper error
		}
	}
}
