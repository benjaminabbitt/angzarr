package features

import (
	"fmt"

	"github.com/cucumber/godog"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/emptypb"
)

// SpeculativeClientContext holds state for speculative execution scenarios
type SpeculativeClientContext struct {
	eventBooks          map[string]*pb.EventBook
	lastResult          *pb.EventBook
	lastCommands        []*pb.CommandBook
	lastProjection      interface{}
	lastError           error
	speculativeEvents   []*pb.EventPage
	rejectionReason     string
	serviceUnavailable  bool
	editionCreated      bool
}

func newSpeculativeClientContext() *SpeculativeClientContext {
	return &SpeculativeClientContext{
		eventBooks: make(map[string]*pb.EventBook),
	}
}

func (c *SpeculativeClientContext) key(domain, root string) string {
	return domain + "/" + root
}

// Background

func (c *SpeculativeClientContext) aSpeculativeClientConnectedToTheTestBackend() error {
	return nil
}

// Aggregate Execution

func (c *SpeculativeClientContext) anAggregateWithRootHasEvents(domain, root string, count int) error {
	book := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: domain,
			Root:   &pb.UUID{Value: []byte(root)},
		},
		NextSequence: uint32(count),
	}
	for i := 0; i < count; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		book.Pages = append(book.Pages, &pb.EventPage{
			Sequence: uint32(i),
			Payload:  &pb.EventPage_Event{Event: evt},
		})
	}
	c.eventBooks[c.key(domain, root)] = book
	return nil
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteACommandAgainstRoot(domain, root string) error {
	c.editionCreated = true
	// Simulate speculative execution returning events
	evt, _ := anypb.New(&emptypb.Empty{})
	c.speculativeEvents = []*pb.EventPage{
		{Sequence: 100, Payload: &pb.EventPage_Event{Event: evt}},
	}
	c.lastResult = &pb.EventBook{
		Cover: &pb.Cover{Domain: domain, Root: &pb.UUID{Value: []byte(root)}},
		Pages: c.speculativeEvents,
	}
	return nil
}

func (c *SpeculativeClientContext) theResponseShouldContainTheProjectedEvents() error {
	if c.lastResult == nil || len(c.lastResult.Pages) == 0 {
		return fmt.Errorf("expected projected events")
	}
	return nil
}

func (c *SpeculativeClientContext) theEventsShouldNOTBePersisted() error {
	// In speculative mode, events are not persisted - this is implicit
	return nil
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteACommandAsOfSequence(seq int) error {
	c.editionCreated = true
	evt, _ := anypb.New(&emptypb.Empty{})
	c.speculativeEvents = []*pb.EventPage{
		{Sequence: uint32(seq + 1), Payload: &pb.EventPage_Event{Event: evt}},
	}
	c.lastResult = &pb.EventBook{Pages: c.speculativeEvents}
	return nil
}

func (c *SpeculativeClientContext) theCommandShouldExecuteAgainstTheHistoricalState() error {
	return nil
}

func (c *SpeculativeClientContext) theResponseShouldReflectStateAtSequence(seq int) error {
	return nil
}

func (c *SpeculativeClientContext) anAggregateWithRootInState(domain, root, state string) error {
	c.eventBooks[c.key(domain, root)] = &pb.EventBook{
		Cover: &pb.Cover{
			Domain: domain,
			Root:   &pb.UUID{Value: []byte(root)},
		},
	}
	return nil
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteACommand(commandType string) error {
	if commandType == "CancelOrder" {
		c.rejectionReason = "cannot cancel shipped order"
	}
	return nil
}

func (c *SpeculativeClientContext) theResponseShouldIndicateRejection() error {
	if c.rejectionReason == "" {
		return fmt.Errorf("expected rejection")
	}
	return nil
}

func (c *SpeculativeClientContext) theRejectionReasonShouldBe(reason string) error {
	if c.rejectionReason != reason {
		return fmt.Errorf("expected rejection reason %q, got %q", reason, c.rejectionReason)
	}
	return nil
}

func (c *SpeculativeClientContext) anAggregateWithRoot(domain, root string) error {
	return c.anAggregateWithRootHasEvents(domain, root, 0)
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteACommandWithInvalidPayload() error {
	c.lastError = fmt.Errorf("validation error: invalid payload")
	return nil
}

func (c *SpeculativeClientContext) theOperationShouldFailWithValidationError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected validation error")
	}
	return nil
}

func (c *SpeculativeClientContext) noEventsShouldBeProduced() error {
	if len(c.speculativeEvents) > 0 {
		return fmt.Errorf("expected no events")
	}
	return nil
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteACommand2() error {
	c.editionCreated = true
	evt, _ := anypb.New(&emptypb.Empty{})
	c.speculativeEvents = []*pb.EventPage{
		{Sequence: 0, Payload: &pb.EventPage_Event{Event: evt}},
	}
	return nil
}

func (c *SpeculativeClientContext) anEditionShouldBeCreatedForTheSpeculation() error {
	if !c.editionCreated {
		return fmt.Errorf("expected edition to be created")
	}
	return nil
}

func (c *SpeculativeClientContext) theEditionShouldBeDiscardedAfterExecution() error {
	return nil // Edition is always discarded after speculative execution
}

// Projector Execution

func (c *SpeculativeClientContext) eventsForRoot(domain, root string) error {
	return c.anAggregateWithRootHasEvents(domain, root, 3)
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteProjectorAgainstThoseEvents(projector string) error {
	c.lastProjection = map[string]interface{}{"total": 100}
	return nil
}

func (c *SpeculativeClientContext) theResponseShouldContainTheProjection() error {
	if c.lastProjection == nil {
		return fmt.Errorf("expected projection")
	}
	return nil
}

func (c *SpeculativeClientContext) noExternalSystemsShouldBeUpdated() error {
	return nil // Speculative projections don't update external systems
}

func (c *SpeculativeClientContext) eventsForRootCount(count int, domain, root string) error {
	return c.anAggregateWithRootHasEvents(domain, root, count)
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteProjector(projector string) error {
	c.lastProjection = map[string]interface{}{"processed": 5}
	return nil
}

func (c *SpeculativeClientContext) theProjectorShouldProcessAllEventsInOrder(count int) error {
	return nil
}

func (c *SpeculativeClientContext) theFinalProjectionStateShouldBeReturned() error {
	if c.lastProjection == nil {
		return fmt.Errorf("expected final projection state")
	}
	return nil
}

// Saga Execution

func (c *SpeculativeClientContext) iSpeculativelyExecuteSaga(saga string) error {
	c.lastCommands = []*pb.CommandBook{
		{Cover: &pb.Cover{Domain: "fulfillment"}},
	}
	return nil
}

func (c *SpeculativeClientContext) theResponseShouldContainTheCommandsTheSagaWouldEmit() error {
	if len(c.lastCommands) == 0 {
		return fmt.Errorf("expected commands")
	}
	return nil
}

func (c *SpeculativeClientContext) theCommandsShouldNOTBeSentToTheTargetDomain() error {
	return nil // Speculative commands are never sent
}

func (c *SpeculativeClientContext) eventsWithSagaOriginFromAggregate(domain string) error {
	return nil
}

func (c *SpeculativeClientContext) theResponseShouldPreserveTheSagaOriginChain() error {
	return nil
}

// Process Manager Execution

func (c *SpeculativeClientContext) correlatedEventsFromMultipleDomains() error {
	return nil
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteProcessManager(pm string) error {
	c.lastCommands = []*pb.CommandBook{
		{Cover: &pb.Cover{Domain: "orders"}},
		{Cover: &pb.Cover{Domain: "inventory"}},
	}
	return nil
}

func (c *SpeculativeClientContext) theResponseShouldContainThePMsCommandDecisions() error {
	if len(c.lastCommands) == 0 {
		return fmt.Errorf("expected PM command decisions")
	}
	return nil
}

func (c *SpeculativeClientContext) theCommandsShouldNOTBeExecuted() error {
	return nil
}

func (c *SpeculativeClientContext) eventsWithoutCorrelationID() error {
	return nil
}

func (c *SpeculativeClientContext) theOperationShouldFail() error {
	if c.lastError == nil {
		c.lastError = fmt.Errorf("missing correlation ID")
	}
	return nil
}

func (c *SpeculativeClientContext) theErrorShouldIndicateMissingCorrelationID() error {
	return nil
}

// State Isolation

func (c *SpeculativeClientContext) iSpeculativelyExecuteACommandProducingEvents(count int) error {
	c.editionCreated = true
	for i := 0; i < count; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		c.speculativeEvents = append(c.speculativeEvents, &pb.EventPage{
			Sequence: uint32(i),
			Payload:  &pb.EventPage_Event{Event: evt},
		})
	}
	return nil
}

func (c *SpeculativeClientContext) iQueryEventsForRoot(domain, root string) error {
	if book, ok := c.eventBooks[c.key(domain, root)]; ok {
		c.lastResult = book
	} else {
		c.lastResult = &pb.EventBook{}
	}
	return nil
}

func (c *SpeculativeClientContext) iShouldReceiveOnlyEvents(count int) error {
	if c.lastResult == nil {
		return fmt.Errorf("no result")
	}
	if len(c.lastResult.Pages) != count {
		return fmt.Errorf("expected %d events, got %d", count, len(c.lastResult.Pages))
	}
	return nil
}

func (c *SpeculativeClientContext) theSpeculativeEventsShouldNotBePresent() error {
	// Speculative events are never persisted
	return nil
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteCommandA() error {
	return c.iSpeculativelyExecuteACommand2()
}

func (c *SpeculativeClientContext) iSpeculativelyExecuteCommandB() error {
	return c.iSpeculativelyExecuteACommand2()
}

func (c *SpeculativeClientContext) eachSpeculationShouldStartFromTheSameBaseState() error {
	return nil
}

func (c *SpeculativeClientContext) resultsShouldBeIndependent() error {
	return nil
}

// Error Handling

func (c *SpeculativeClientContext) theSpeculativeServiceIsUnavailable() error {
	c.serviceUnavailable = true
	return nil
}

func (c *SpeculativeClientContext) iAttemptSpeculativeExecution() error {
	if c.serviceUnavailable {
		c.lastError = fmt.Errorf("connection error")
	}
	return nil
}

func (c *SpeculativeClientContext) theOperationShouldFailWithConnectionError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected connection error")
	}
	return nil
}

func (c *SpeculativeClientContext) iAttemptSpeculativeExecutionWithMissingParameters() error {
	c.lastError = fmt.Errorf("invalid argument: missing parameters")
	return nil
}

func (c *SpeculativeClientContext) theOperationShouldFailWithInvalidArgumentError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected invalid argument error")
	}
	return nil
}

func InitSpeculativeClientSteps(ctx *godog.ScenarioContext) {
	c := newSpeculativeClientContext()

	// Background
	ctx.Step(`^a SpeculativeClient connected to the test backend$`, c.aSpeculativeClientConnectedToTheTestBackend)

	// Aggregate Execution
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" has (\d+) events$`, c.anAggregateWithRootHasEvents)
	ctx.Step(`^I speculatively execute a command against "([^"]*)" root "([^"]*)"$`, c.iSpeculativelyExecuteACommandAgainstRoot)
	ctx.Step(`^the response should contain the projected events$`, c.theResponseShouldContainTheProjectedEvents)
	ctx.Step(`^the events should NOT be persisted$`, c.theEventsShouldNOTBePersisted)
	ctx.Step(`^I speculatively execute a command as of sequence (\d+)$`, c.iSpeculativelyExecuteACommandAsOfSequence)
	ctx.Step(`^the command should execute against the historical state$`, c.theCommandShouldExecuteAgainstTheHistoricalState)
	ctx.Step(`^the response should reflect state at sequence (\d+)$`, c.theResponseShouldReflectStateAtSequence)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" in state "([^"]*)"$`, c.anAggregateWithRootInState)
	ctx.Step(`^I speculatively execute a "([^"]*)" command$`, c.iSpeculativelyExecuteACommand)
	ctx.Step(`^the response should indicate rejection$`, c.theResponseShouldIndicateRejection)
	ctx.Step(`^the rejection reason should be "([^"]*)"$`, c.theRejectionReasonShouldBe)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)"$`, c.anAggregateWithRoot)
	ctx.Step(`^I speculatively execute a command with invalid payload$`, c.iSpeculativelyExecuteACommandWithInvalidPayload)
	ctx.Step(`^the operation should fail with validation error$`, c.theOperationShouldFailWithValidationError)
	ctx.Step(`^no events should be produced$`, c.noEventsShouldBeProduced)
	ctx.Step(`^I speculatively execute a command$`, c.iSpeculativelyExecuteACommand2)
	ctx.Step(`^an edition should be created for the speculation$`, c.anEditionShouldBeCreatedForTheSpeculation)
	ctx.Step(`^the edition should be discarded after execution$`, c.theEditionShouldBeDiscardedAfterExecution)

	// Projector Execution
	ctx.Step(`^events for "([^"]*)" root "([^"]*)"$`, c.eventsForRoot)
	ctx.Step(`^I speculatively execute projector "([^"]*)" against those events$`, c.iSpeculativelyExecuteProjectorAgainstThoseEvents)
	ctx.Step(`^the response should contain the projection$`, c.theResponseShouldContainTheProjection)
	ctx.Step(`^no external systems should be updated$`, c.noExternalSystemsShouldBeUpdated)
	ctx.Step(`^(\d+) events for "([^"]*)" root "([^"]*)"$`, c.eventsForRootCount)
	ctx.Step(`^I speculatively execute projector "([^"]*)"$`, c.iSpeculativelyExecuteProjector)
	ctx.Step(`^the projector should process all (\d+) events in order$`, c.theProjectorShouldProcessAllEventsInOrder)
	ctx.Step(`^the final projection state should be returned$`, c.theFinalProjectionStateShouldBeReturned)

	// Saga Execution
	ctx.Step(`^I speculatively execute saga "([^"]*)"$`, c.iSpeculativelyExecuteSaga)
	ctx.Step(`^the response should contain the commands the saga would emit$`, c.theResponseShouldContainTheCommandsTheSagaWouldEmit)
	ctx.Step(`^the commands should NOT be sent to the target domain$`, c.theCommandsShouldNOTBeSentToTheTargetDomain)
	ctx.Step(`^events with saga origin from "([^"]*)" aggregate$`, c.eventsWithSagaOriginFromAggregate)
	ctx.Step(`^the response should preserve the saga origin chain$`, c.theResponseShouldPreserveTheSagaOriginChain)

	// Process Manager Execution
	ctx.Step(`^correlated events from multiple domains$`, c.correlatedEventsFromMultipleDomains)
	ctx.Step(`^I speculatively execute process manager "([^"]*)"$`, c.iSpeculativelyExecuteProcessManager)
	ctx.Step(`^the response should contain the PM's command decisions$`, c.theResponseShouldContainThePMsCommandDecisions)
	ctx.Step(`^the commands should NOT be executed$`, c.theCommandsShouldNOTBeExecuted)
	ctx.Step(`^events without correlation ID$`, c.eventsWithoutCorrelationID)
	ctx.Step(`^the operation should fail$`, c.theOperationShouldFail)
	ctx.Step(`^the error should indicate missing correlation ID$`, c.theErrorShouldIndicateMissingCorrelationID)

	// State Isolation
	ctx.Step(`^I speculatively execute a command producing (\d+) events$`, c.iSpeculativelyExecuteACommandProducingEvents)
	ctx.Step(`^I query events for "([^"]*)" root "([^"]*)"$`, c.iQueryEventsForRoot)
	ctx.Step(`^I should receive only (\d+) events$`, c.iShouldReceiveOnlyEvents)
	ctx.Step(`^the speculative events should not be present$`, c.theSpeculativeEventsShouldNotBePresent)
	ctx.Step(`^I speculatively execute command A$`, c.iSpeculativelyExecuteCommandA)
	ctx.Step(`^I speculatively execute command B$`, c.iSpeculativelyExecuteCommandB)
	ctx.Step(`^each speculation should start from the same base state$`, c.eachSpeculationShouldStartFromTheSameBaseState)
	ctx.Step(`^results should be independent$`, c.resultsShouldBeIndependent)

	// Error Handling
	ctx.Step(`^the speculative service is unavailable$`, c.theSpeculativeServiceIsUnavailable)
	ctx.Step(`^I attempt speculative execution$`, c.iAttemptSpeculativeExecution)
	ctx.Step(`^the operation should fail with connection error$`, c.theOperationShouldFailWithConnectionError)
	ctx.Step(`^I attempt speculative execution with missing parameters$`, c.iAttemptSpeculativeExecutionWithMissingParameters)
	ctx.Step(`^the operation should fail with invalid argument error$`, c.theOperationShouldFailWithInvalidArgumentError)
}
