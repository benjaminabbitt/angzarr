package features

import (
	"context"
	"errors"
	"fmt"
	"os"
	"strings"
	"testing"

	"projector-receipt/logic"
	"projector-receipt/proto/angzarr"
	"projector-receipt/proto/examples"

	"github.com/cucumber/godog"
	"google.golang.org/protobuf/types/known/anypb"
)

type projectorTestContext struct {
	logic       logic.ReceiptProjectorLogic
	priorEvents []*anypb.Any
	state       *logic.TransactionState
	receipt     *examples.Receipt
}

func (c *projectorTestContext) reset() {
	c.logic = logic.NewReceiptProjectorLogic()
	c.priorEvents = nil
	c.state = nil
	c.receipt = nil
}

func (c *projectorTestContext) buildEventBook() *angzarr.EventBook {
	if len(c.priorEvents) == 0 {
		return nil
	}
	pages := make([]*angzarr.EventPage, len(c.priorEvents))
	for i, event := range c.priorEvents {
		pages[i] = &angzarr.EventPage{Event: event}
	}
	return &angzarr.EventBook{Pages: pages}
}

// Given steps

func (c *projectorTestContext) aTransactionCreatedEventWithCustomerAndSubtotal(customerID string, subtotal int) error {
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

func (c *projectorTestContext) aTransactionCreatedEventWithCustomerAndItems(customerID string, items *godog.Table) error {
	lineItems := make([]*examples.LineItem, 0)
	var subtotal int32 = 0

	// Skip header row
	for _, row := range items.Rows[1:] {
		productID := row.Cells[0].Value
		name := row.Cells[1].Value
		var quantity int32
		var unitPriceCents int32
		fmt.Sscanf(row.Cells[2].Value, "%d", &quantity)
		fmt.Sscanf(row.Cells[3].Value, "%d", &unitPriceCents)

		lineItems = append(lineItems, &examples.LineItem{
			ProductId:      productID,
			Name:           name,
			Quantity:       quantity,
			UnitPriceCents: unitPriceCents,
		})
		subtotal += quantity * unitPriceCents
	}

	event := &examples.TransactionCreated{
		CustomerId:    customerID,
		Items:         lineItems,
		SubtotalCents: subtotal,
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *projectorTestContext) aDiscountAppliedEventWithCentsDiscount(discountCents int) error {
	event := &examples.DiscountApplied{
		DiscountType:  "loyalty",
		DiscountCents: int32(discountCents),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *projectorTestContext) aTransactionCompletedEventWithTotalAndPayment(total int, payment string) error {
	event := &examples.TransactionCompleted{
		FinalTotalCents: int32(total),
		PaymentMethod:   payment,
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

func (c *projectorTestContext) aTransactionCompletedEventWithTotalAndPaymentEarningPoints(total int, payment string, points int) error {
	event := &examples.TransactionCompleted{
		FinalTotalCents:     int32(total),
		PaymentMethod:       payment,
		LoyaltyPointsEarned: int32(points),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	c.priorEvents = append(c.priorEvents, eventAny)
	return nil
}

// When steps

func (c *projectorTestContext) iProjectTheEvents() error {
	eventBook := c.buildEventBook()
	c.state = c.logic.RebuildState(eventBook)
	c.receipt = c.logic.GenerateReceipt("test-transaction-id", c.state)
	return nil
}

// Then steps

func (c *projectorTestContext) noProjectionIsGenerated() error {
	if c.receipt != nil {
		return errors.New("expected no projection but got a receipt")
	}
	return nil
}

func (c *projectorTestContext) aReceiptProjectionIsGenerated() error {
	if c.receipt == nil {
		return errors.New("expected a Receipt projection but got nil")
	}
	return nil
}

func (c *projectorTestContext) theReceiptHasCustomerID(customerID string) error {
	if c.receipt == nil {
		return errors.New("receipt is nil")
	}
	if c.receipt.CustomerId != customerID {
		return fmt.Errorf("expected customer_id %q, got %q", customerID, c.receipt.CustomerId)
	}
	return nil
}

func (c *projectorTestContext) theReceiptHasFinalTotalCents(total int) error {
	if c.receipt == nil {
		return errors.New("receipt is nil")
	}
	if int(c.receipt.FinalTotalCents) != total {
		return fmt.Errorf("expected final_total_cents %d, got %d", total, c.receipt.FinalTotalCents)
	}
	return nil
}

func (c *projectorTestContext) theReceiptHasPaymentMethod(payment string) error {
	if c.receipt == nil {
		return errors.New("receipt is nil")
	}
	if c.receipt.PaymentMethod != payment {
		return fmt.Errorf("expected payment_method %q, got %q", payment, c.receipt.PaymentMethod)
	}
	return nil
}

func (c *projectorTestContext) theReceiptHasSubtotalCents(subtotal int) error {
	if c.receipt == nil {
		return errors.New("receipt is nil")
	}
	if int(c.receipt.SubtotalCents) != subtotal {
		return fmt.Errorf("expected subtotal_cents %d, got %d", subtotal, c.receipt.SubtotalCents)
	}
	return nil
}

func (c *projectorTestContext) theReceiptHasDiscountCents(discount int) error {
	if c.receipt == nil {
		return errors.New("receipt is nil")
	}
	if int(c.receipt.DiscountCents) != discount {
		return fmt.Errorf("expected discount_cents %d, got %d", discount, c.receipt.DiscountCents)
	}
	return nil
}

func (c *projectorTestContext) theReceiptHasLoyaltyPointsEarned(points int) error {
	if c.receipt == nil {
		return errors.New("receipt is nil")
	}
	if int(c.receipt.LoyaltyPointsEarned) != points {
		return fmt.Errorf("expected loyalty_points_earned %d, got %d", points, c.receipt.LoyaltyPointsEarned)
	}
	return nil
}

func (c *projectorTestContext) theReceiptFormattedTextContains(substring string) error {
	if c.receipt == nil {
		return errors.New("receipt is nil")
	}
	if !strings.Contains(c.receipt.FormattedText, substring) {
		return fmt.Errorf("expected formatted_text to contain %q, got %q", substring, c.receipt.FormattedText)
	}
	return nil
}

func InitializeScenario(ctx *godog.ScenarioContext) {
	tc := &projectorTestContext{}

	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc.reset()
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^a TransactionCreated event with customer "([^"]*)" and subtotal (\d+)$`, tc.aTransactionCreatedEventWithCustomerAndSubtotal)
	ctx.Step(`^a TransactionCreated event with customer "([^"]*)" and items:$`, tc.aTransactionCreatedEventWithCustomerAndItems)
	ctx.Step(`^a DiscountApplied event with (\d+) cents discount$`, tc.aDiscountAppliedEventWithCentsDiscount)
	ctx.Step(`^a TransactionCompleted event with total (\d+) and payment "([^"]*)"$`, tc.aTransactionCompletedEventWithTotalAndPayment)
	ctx.Step(`^a TransactionCompleted event with total (\d+) and payment "([^"]*)" earning (\d+) points$`, tc.aTransactionCompletedEventWithTotalAndPaymentEarningPoints)

	// When steps
	ctx.Step(`^I project the events$`, tc.iProjectTheEvents)

	// Then steps
	ctx.Step(`^no projection is generated$`, tc.noProjectionIsGenerated)
	ctx.Step(`^a Receipt projection is generated$`, tc.aReceiptProjectionIsGenerated)
	ctx.Step(`^the receipt has customer_id "([^"]*)"$`, tc.theReceiptHasCustomerID)
	ctx.Step(`^the receipt has final_total_cents (\d+)$`, tc.theReceiptHasFinalTotalCents)
	ctx.Step(`^the receipt has payment_method "([^"]*)"$`, tc.theReceiptHasPaymentMethod)
	ctx.Step(`^the receipt has subtotal_cents (\d+)$`, tc.theReceiptHasSubtotalCents)
	ctx.Step(`^the receipt has discount_cents (\d+)$`, tc.theReceiptHasDiscountCents)
	ctx.Step(`^the receipt has loyalty_points_earned (\d+)$`, tc.theReceiptHasLoyaltyPointsEarned)
	ctx.Step(`^the receipt formatted_text contains "([^"]*)"$`, tc.theReceiptFormattedTextContains)
}

func TestFeatures(t *testing.T) {
	suite := godog.TestSuite{
		ScenarioInitializer: InitializeScenario,
		Options: &godog.Options{
			Format:   "pretty",
			Paths:    []string{"projector-receipt.feature"},
			TestingT: t,
		},
	}

	if suite.Run() != 0 {
		t.Fatal("non-zero status returned, failed to run feature tests")
	}
}

func init() {
	// Ensure we're in the correct directory when running tests
	if _, err := os.Stat("projector-receipt.feature"); os.IsNotExist(err) {
		// Will fail at runtime with proper error
	}
}
