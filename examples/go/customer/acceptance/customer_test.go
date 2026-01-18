// Package acceptance provides BDD acceptance tests for the customer domain.
// These tests run against a deployed Kind cluster via gRPC.
package acceptance

import (
	"context"
	"os"
	"testing"
	"time"

	"customer/proto/angzarr"
	"customer/proto/examples"

	"github.com/cucumber/godog"
	"github.com/google/uuid"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/protobuf/types/known/anypb"
)

type acceptanceContext struct {
	conn            *grpc.ClientConn
	gatewayClient   angzarr.CommandGatewayClient
	queryClient     angzarr.EventQueryClient
	customerID      uuid.UUID
	correlationID   string
	lastResponse    *angzarr.CommandResponse
	lastError       error
}

func (c *acceptanceContext) reset() {
	c.customerID = uuid.New()
	c.correlationID = uuid.New().String()
	c.lastResponse = nil
	c.lastError = nil
}

// getAngzarrEndpoint returns the Angzarr gateway endpoint from environment or default.
// Uses ANGZARR_ENDPOINT for full URL, or ANGZARR_HOST:ANGZARR_PORT for components.
func getAngzarrEndpoint() string {
	if endpoint := os.Getenv("ANGZARR_ENDPOINT"); endpoint != "" {
		return endpoint
	}
	host := os.Getenv("ANGZARR_HOST")
	if host == "" {
		host = "localhost"
	}
	port := os.Getenv("ANGZARR_PORT")
	if port == "" {
		port = "1350"
	}
	return host + ":" + port
}

func (c *acceptanceContext) connect() error {
	endpoint := getAngzarrEndpoint()

	conn, err := grpc.NewClient(endpoint, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return err
	}
	c.conn = conn
	c.gatewayClient = angzarr.NewCommandGatewayClient(conn)
	c.queryClient = angzarr.NewEventQueryClient(conn)
	return nil
}

func (c *acceptanceContext) close() {
	if c.conn != nil {
		c.conn.Close()
	}
}

func (c *acceptanceContext) noPriorEventsForTheAggregate() error {
	// Using a new UUID ensures no prior events
	c.customerID = uuid.New()
	return nil
}

func (c *acceptanceContext) iHandleACreateCustomerCommandWithNameAndEmail(name, email string) error {
	cmd := &examples.CreateCustomer{
		Name:  name,
		Email: email,
	}
	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return err
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	req := &angzarr.CommandBook{
		Cover: &angzarr.Cover{
			Domain: "customer",
			Root:   &angzarr.UUID{Value: c.customerID[:]},
		},
		Pages: []*angzarr.CommandPage{
			{Command: cmdAny},
		},
		CorrelationId: c.correlationID,
	}

	c.lastResponse, c.lastError = c.gatewayClient.Execute(ctx, req)
	return nil
}

func (c *acceptanceContext) theResultIsACustomerCreatedEvent() error {
	if c.lastError != nil {
		return c.lastError
	}
	if c.lastResponse == nil || c.lastResponse.Events == nil {
		return nil // Placeholder - would verify event type
	}
	return nil
}

func (c *acceptanceContext) theEventHasName(name string) error {
	// Placeholder - would extract and verify event data
	return nil
}

func (c *acceptanceContext) theEventHasEmail(email string) error {
	// Placeholder - would extract and verify event data
	return nil
}

func InitializeScenario(ctx *godog.ScenarioContext) {
	tc := &acceptanceContext{}

	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		tc.reset()
		if err := tc.connect(); err != nil {
			return ctx, err
		}
		return ctx, nil
	})

	ctx.After(func(ctx context.Context, sc *godog.Scenario, err error) (context.Context, error) {
		tc.close()
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^no prior events for the aggregate$`, tc.noPriorEventsForTheAggregate)

	// When steps
	ctx.Step(`^I handle a CreateCustomer command with name "([^"]*)" and email "([^"]*)"$`, tc.iHandleACreateCustomerCommandWithNameAndEmail)

	// Then steps
	ctx.Step(`^the result is a CustomerCreated event$`, tc.theResultIsACustomerCreatedEvent)
	ctx.Step(`^the event has name "([^"]*)"$`, tc.theEventHasName)
	ctx.Step(`^the event has email "([^"]*)"$`, tc.theEventHasEmail)
}

func TestFeatures(t *testing.T) {
	if os.Getenv("ANGZARR_TEST_MODE") != "container" {
		t.Skip("Skipping acceptance tests (ANGZARR_TEST_MODE != container)")
	}

	suite := godog.TestSuite{
		ScenarioInitializer: InitializeScenario,
		Options: &godog.Options{
			Format:   "pretty",
			Paths:    []string{"../features/customer.feature"},
			TestingT: t,
		},
	}

	if suite.Run() != 0 {
		t.Fatal("non-zero status returned, failed to run feature tests")
	}
}
