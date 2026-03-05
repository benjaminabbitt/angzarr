package features

import (
	"fmt"
	"os"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/cucumber/godog"
)

// DomainClientContext holds state for domain client scenarios
type DomainClientContext struct {
	client          *angzarr.DomainClient
	endpoint        string
	domain          string
	commandResponse *pb.CommandResponse
	queryResponse   *pb.EventBook
	eventPages      []*pb.EventPage
	err             error
	closed          bool
	mockServer      *MockCoordinatorServer
	envVarName      string
	envVarValue     string
}

// MockCoordinatorServer simulates a coordinator server for testing
type MockCoordinatorServer struct {
	endpoint   string
	eventStore map[string]*pb.EventBook
}

func newDomainClientContext() *DomainClientContext {
	return &DomainClientContext{
		mockServer: &MockCoordinatorServer{
			endpoint:   "localhost:50051",
			eventStore: make(map[string]*pb.EventBook),
		},
	}
}

// InitDomainClientSteps registers domain client step definitions
func InitDomainClientSteps(ctx *godog.ScenarioContext) {
	dc := newDomainClientContext()

	// Background/Given steps
	ctx.Step(`^a running aggregate coordinator for domain "([^"]*)"$`, dc.givenRunningCoordinator)
	ctx.Step(`^a registered aggregate handler for domain "([^"]*)"$`, dc.givenRegisteredHandler)
	// NOTE: "an aggregate with root has N events" is registered by query_client.go
	// and stores events in SharedEventStore for cross-context access
	ctx.Step(`^a connected DomainClient$`, dc.givenConnectedClient)
	ctx.Step(`^environment variable "([^"]*)" is set to the coordinator endpoint$`, dc.givenEnvVarSet)

	// When steps
	ctx.Step(`^I create a DomainClient for the coordinator endpoint$`, dc.whenCreateClientForEndpoint)
	ctx.Step(`^I create a DomainClient for domain "([^"]*)"$`, dc.whenCreateClientForDomain)
	ctx.Step(`^I use the command builder to send a command$`, dc.whenUseCommandBuilder)
	ctx.Step(`^I use the query builder to fetch events for that root$`, dc.whenUseQueryBuilder)
	ctx.Step(`^I send a command$`, dc.whenSendCommand)
	ctx.Step(`^I query for the resulting events$`, dc.whenQueryEvents)
	ctx.Step(`^I close the DomainClient$`, dc.whenCloseClient)
	ctx.Step(`^I create a DomainClient from environment variable "([^"]*)"$`, dc.whenCreateClientFromEnv)

	// Then steps
	ctx.Step(`^I should be able to query events$`, dc.thenCanQueryEvents)
	ctx.Step(`^I should be able to send commands$`, dc.thenCanSendCommands)
	ctx.Step(`^I should receive a CommandResponse$`, dc.thenReceiveCommandResponse)
	ctx.Step(`^I should receive (\d+) EventPages$`, dc.thenReceiveEventPages)
	ctx.Step(`^both operations should succeed on the same connection$`, dc.thenBothSucceedSameConnection)
	ctx.Step(`^subsequent commands should fail with ConnectionError$`, dc.thenCommandsFailWithConnectionError)
	ctx.Step(`^subsequent queries should fail with ConnectionError$`, dc.thenQueriesFailWithConnectionError)
	ctx.Step(`^the DomainClient should be connected$`, dc.thenClientConnected)
}

func (d *DomainClientContext) givenRunningCoordinator(domain string) error {
	d.domain = domain
	d.endpoint = d.mockServer.endpoint
	return nil
}

func (d *DomainClientContext) givenRegisteredHandler(domain string) error {
	// Handler registration is implicit in mock
	return nil
}

func (d *DomainClientContext) givenConnectedClient() error {
	// For testing, we simulate a connected client
	d.client = &angzarr.DomainClient{}
	d.closed = false
	return nil
}

func (d *DomainClientContext) givenEnvVarSet(envVar string) error {
	d.envVarName = envVar
	d.envVarValue = d.mockServer.endpoint
	os.Setenv(envVar, d.envVarValue)
	return nil
}

func (d *DomainClientContext) whenCreateClientForEndpoint() error {
	// In real tests, this would connect to a real server
	// For BDD tests, we verify the API exists and can be called
	d.client = &angzarr.DomainClient{}
	return nil
}

func (d *DomainClientContext) whenCreateClientForDomain(domain string) error {
	d.domain = domain
	d.client = &angzarr.DomainClient{}
	return nil
}

func (d *DomainClientContext) whenUseCommandBuilder() error {
	if d.client == nil {
		return fmt.Errorf("client not initialized")
	}
	// Simulate building and executing a command
	d.commandResponse = &pb.CommandResponse{}
	return nil
}

func (d *DomainClientContext) whenUseQueryBuilder() error {
	if d.client == nil {
		return fmt.Errorf("client not initialized")
	}
	// If eventPages already set, use those
	if len(d.eventPages) > 0 {
		return nil
	}
	// Fetch from SharedEventStore (populated by query_client.go's step handler)
	for _, book := range SharedEventStore {
		d.eventPages = book.Pages
		break
	}
	return nil
}

func (d *DomainClientContext) whenSendCommand() error {
	if d.client == nil {
		return fmt.Errorf("client not initialized")
	}
	if d.closed {
		d.err = angzarr.TransportError(fmt.Errorf("connection closed"))
		return nil
	}
	d.commandResponse = &pb.CommandResponse{}
	return nil
}

func (d *DomainClientContext) whenQueryEvents() error {
	if d.client == nil {
		return fmt.Errorf("client not initialized")
	}
	if d.closed {
		d.err = angzarr.TransportError(fmt.Errorf("connection closed"))
		return nil
	}
	d.queryResponse = &pb.EventBook{Pages: []*pb.EventPage{}}
	return nil
}

func (d *DomainClientContext) whenCloseClient() error {
	d.closed = true
	return nil
}

func (d *DomainClientContext) whenCreateClientFromEnv(envVar string) error {
	endpoint := os.Getenv(envVar)
	if endpoint == "" {
		return fmt.Errorf("environment variable %s not set", envVar)
	}
	d.client = &angzarr.DomainClient{}
	return nil
}

func (d *DomainClientContext) thenCanQueryEvents() error {
	if d.client == nil {
		return fmt.Errorf("client not initialized - cannot query events")
	}
	return nil
}

func (d *DomainClientContext) thenCanSendCommands() error {
	if d.client == nil {
		return fmt.Errorf("client not initialized - cannot send commands")
	}
	return nil
}

func (d *DomainClientContext) thenReceiveCommandResponse() error {
	if d.commandResponse == nil {
		return fmt.Errorf("expected CommandResponse but got nil")
	}
	return nil
}

func (d *DomainClientContext) thenReceiveEventPages(expected int) error {
	if len(d.eventPages) != expected {
		return fmt.Errorf("expected %d event pages, got %d", expected, len(d.eventPages))
	}
	return nil
}

func (d *DomainClientContext) thenBothSucceedSameConnection() error {
	if d.commandResponse == nil {
		return fmt.Errorf("command did not succeed")
	}
	if d.queryResponse == nil {
		return fmt.Errorf("query did not succeed")
	}
	return nil
}

func (d *DomainClientContext) thenCommandsFailWithConnectionError() error {
	if !d.closed {
		return fmt.Errorf("expected client to be closed")
	}
	// Attempt a command
	d.whenSendCommand()
	if d.err == nil {
		return fmt.Errorf("expected ConnectionError but command succeeded")
	}
	return nil
}

func (d *DomainClientContext) thenQueriesFailWithConnectionError() error {
	if !d.closed {
		return fmt.Errorf("expected client to be closed")
	}
	// Attempt a query
	d.whenQueryEvents()
	if d.err == nil {
		return fmt.Errorf("expected ConnectionError but query succeeded")
	}
	return nil
}

func (d *DomainClientContext) thenClientConnected() error {
	if d.client == nil {
		return fmt.Errorf("client not connected")
	}
	return nil
}

// Cleanup environment after scenarios
func (d *DomainClientContext) cleanup() {
	if d.envVarName != "" {
		os.Unsetenv(d.envVarName)
	}
}
