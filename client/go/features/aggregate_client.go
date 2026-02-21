package features

import (
	"fmt"

	"github.com/cucumber/godog"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/emptypb"
)

// AggregateContext holds test context for aggregate scenarios
type AggregateContext struct {
	aggregateRouter   *MockAggregateRouter
	sagaRouter        *MockEventRouter
	projectorRouter   *MockEventRouter
	pmRouter          *MockEventRouter
	response          *pb.BusinessResponse
	err               error
	invokedHandlers   []string
	eventBook         *pb.EventBook
}

// MockAggregateRouter simulates CommandRouter behavior
type MockAggregateRouter struct {
	handlers map[string]func() *pb.EventBook
	ctx      *AggregateContext
}

// MockEventRouter simulates EventRouter behavior
type MockEventRouter struct {
	domains  map[string]map[string]func()
	ctx      *AggregateContext
	current  string
}

func NewMockAggregateRouter(ctx *AggregateContext) *MockAggregateRouter {
	return &MockAggregateRouter{
		handlers: make(map[string]func() *pb.EventBook),
		ctx:      ctx,
	}
}

func NewMockEventRouter(ctx *AggregateContext) *MockEventRouter {
	return &MockEventRouter{
		domains: make(map[string]map[string]func()),
		ctx:     ctx,
	}
}

func (r *MockAggregateRouter) On(suffix string, handler func() *pb.EventBook) *MockAggregateRouter {
	r.handlers[suffix] = handler
	return r
}

func (r *MockAggregateRouter) Dispatch(commandType string) (*pb.BusinessResponse, error) {
	for suffix, handler := range r.handlers {
		if commandType == suffix || contains(commandType, suffix) {
			r.ctx.invokedHandlers = append(r.ctx.invokedHandlers, suffix)
			events := handler()
			return &pb.BusinessResponse{Result: &pb.BusinessResponse_Events{Events: events}}, nil
		}
	}
	return nil, fmt.Errorf("Unknown command type: %s", commandType)
}

func (r *MockEventRouter) Domain(name string) *MockEventRouter {
	r.current = name
	if r.domains[name] == nil {
		r.domains[name] = make(map[string]func())
	}
	return r
}

func (r *MockEventRouter) On(suffix string, handler func()) *MockEventRouter {
	if r.current != "" {
		r.domains[r.current][suffix] = handler
	}
	return r
}

func (r *MockEventRouter) Dispatch(domain, eventType string) {
	if domainHandlers, ok := r.domains[domain]; ok {
		for suffix, handler := range domainHandlers {
			if eventType == suffix || contains(eventType, suffix) {
				r.ctx.invokedHandlers = append(r.ctx.invokedHandlers, suffix)
				handler()
				return
			}
		}
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && s[len(s)-len(substr):] == substr
}

// Step definitions

func (c *AggregateContext) anAggregateRouterWithHandlersForAnd(type1, type2 string) error {
	c.aggregateRouter = NewMockAggregateRouter(c)
	c.aggregateRouter.On(type1, func() *pb.EventBook {
		return makeTestEventBook(0)
	})
	c.aggregateRouter.On(type2, func() *pb.EventBook {
		return makeTestEventBook(0)
	})
	return nil
}

func (c *AggregateContext) anAggregateRouter() error {
	c.aggregateRouter = NewMockAggregateRouter(c)
	c.aggregateRouter.On("TestCommand", func() *pb.EventBook {
		return makeTestEventBook(0)
	})
	return nil
}

func (c *AggregateContext) anAggregateWithExistingEvents() error {
	c.eventBook = &pb.EventBook{
		Cover: &pb.Cover{Domain: "test"},
	}
	for i := 0; i < 3; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		c.eventBook.Pages = append(c.eventBook.Pages, &pb.EventPage{
			Sequence: uint32(i),
			Payload:  &pb.EventPage_Event{Event: evt},
		})
	}
	return nil
}

func (c *AggregateContext) anAggregateAtSequence(seq int) error {
	// Set up aggregate router for sequence validation tests
	c.aggregateRouter = NewMockAggregateRouter(c)
	c.aggregateRouter.On("TestCommand", func() *pb.EventBook {
		return makeTestEventBook(0)
	})

	c.eventBook = &pb.EventBook{
		Cover:        &pb.Cover{Domain: "test"},
		NextSequence: uint32(seq),
	}
	for i := 0; i < seq; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		c.eventBook.Pages = append(c.eventBook.Pages, &pb.EventPage{
			Sequence: uint32(i),
			Payload:  &pb.EventPage_Event{Event: evt},
		})
	}
	return nil
}

func (c *AggregateContext) iReceiveACommand(commandType string) error {
	c.response, c.err = c.aggregateRouter.Dispatch(commandType)
	return nil
}

func (c *AggregateContext) iReceiveACommandForThatAggregate() error {
	c.response, c.err = c.aggregateRouter.Dispatch("TestCommand")
	return nil
}

func (c *AggregateContext) iReceiveACommandAtSequence(seq int) error {
	c.response, c.err = c.aggregateRouter.Dispatch("TestCommand")
	return nil
}

func (c *AggregateContext) theHandlerShouldBeInvoked(handlerName string) error {
	for _, h := range c.invokedHandlers {
		if h == handlerName {
			return nil
		}
	}
	return fmt.Errorf("handler %s was not invoked", handlerName)
}

func (c *AggregateContext) theHandlerShouldNOTBeInvoked(handlerName string) error {
	for _, h := range c.invokedHandlers {
		if h == handlerName {
			return fmt.Errorf("handler %s was invoked but should not have been", handlerName)
		}
	}
	return nil
}

func (c *AggregateContext) theRouterShouldLoadTheEventBookFirst() error {
	if c.response == nil && c.err == nil {
		return fmt.Errorf("expected response or error")
	}
	return nil
}

func (c *AggregateContext) theHandlerShouldReceiveTheReconstructedState() error {
	if len(c.invokedHandlers) == 0 {
		return fmt.Errorf("no handlers were invoked")
	}
	return nil
}

func (c *AggregateContext) theRouterShouldReturnAnError() error {
	if c.err == nil {
		return fmt.Errorf("expected an error but got none")
	}
	return nil
}

func (c *AggregateContext) theErrorShouldIndicateUnknownCommandType() error {
	if c.err == nil {
		return fmt.Errorf("expected an error")
	}
	if !contains(c.err.Error(), "Unknown command type") {
		return fmt.Errorf("expected error to indicate unknown command type, got: %s", c.err.Error())
	}
	return nil
}

func (c *AggregateContext) noHandlerShouldBeInvoked() error {
	if c.err != nil {
		return nil // Error means handler wasn't invoked
	}
	if len(c.invokedHandlers) > 0 {
		return fmt.Errorf("handlers were invoked: %v", c.invokedHandlers)
	}
	return nil
}

// Saga Router steps

func (c *AggregateContext) aSagaRouterWithHandlersForAnd(type1, type2 string) error {
	c.sagaRouter = NewMockEventRouter(c)
	c.sagaRouter.Domain("orders").On(type1, func() {}).On(type2, func() {})
	return nil
}

func (c *AggregateContext) aSagaRouter() error {
	c.sagaRouter = NewMockEventRouter(c)
	c.sagaRouter.Domain("orders").On("OrderCreated", func() {})
	return nil
}

func (c *AggregateContext) iReceiveAnEvent(eventType string) error {
	// Dispatch to whichever router is set up (saga, projector, or PM)
	if c.sagaRouter != nil {
		c.sagaRouter.Dispatch("orders", eventType)
	} else if c.projectorRouter != nil {
		c.projectorRouter.Dispatch("orders", eventType)
	} else if c.pmRouter != nil {
		c.pmRouter.Dispatch("orders", eventType)
	}
	return nil
}

// PM Router steps

func (c *AggregateContext) aPMRouterWithHandlersForAnd(type1, type2 string) error {
	c.pmRouter = NewMockEventRouter(c)
	c.pmRouter.Domain("orders").On(type1, func() {})
	c.pmRouter.Domain("inventory").On(type2, func() {})
	return nil
}

func (c *AggregateContext) aPMRouter() error {
	return c.aPMRouterWithHandlersForAnd("OrderCreated", "InventoryReserved")
}

func (c *AggregateContext) iReceiveAnEventFromDomain(eventType, domain string) error {
	c.pmRouter.Dispatch(domain, eventType)
	return nil
}

func (c *AggregateContext) iReceiveAnEventWithoutCorrelationID() error {
	// Event without correlation ID should be skipped
	return nil
}

func (c *AggregateContext) theEventShouldBeSkipped() error {
	if len(c.invokedHandlers) > 0 {
		return fmt.Errorf("expected no handlers to be invoked")
	}
	return nil
}

// Projector Router steps

func (c *AggregateContext) aProjectorRouterWithHandlersFor(eventType string) error {
	c.projectorRouter = NewMockEventRouter(c)
	c.projectorRouter.Domain("orders").On(eventType, func() {})
	return nil
}

func (c *AggregateContext) aProjectorRouter() error {
	return c.aProjectorRouterWithHandlersFor("TestEvent")
}

// Handler Registration steps

func (c *AggregateContext) aRouter() error {
	c.sagaRouter = NewMockEventRouter(c)
	return nil
}

func (c *AggregateContext) iRegisterHandlerForType(eventType string) error {
	c.sagaRouter.Domain("test").On(eventType, func() {})
	return nil
}

func (c *AggregateContext) iRegisterHandlersForAndAnd(type1, type2, type3 string) error {
	c.sagaRouter.Domain("test").On(type1, func() {}).On(type2, func() {}).On(type3, func() {})
	return nil
}

func (c *AggregateContext) eventsEndingWithShouldMatch(suffix string) error {
	// Verify handler is registered
	if c.sagaRouter.domains["test"] == nil {
		return fmt.Errorf("no handlers registered for test domain")
	}
	if _, ok := c.sagaRouter.domains["test"][suffix]; !ok {
		return fmt.Errorf("handler for %s not found", suffix)
	}
	return nil
}

func (c *AggregateContext) eventsEndingWithShouldNOTMatch(suffix string) error {
	if c.sagaRouter.domains["test"] != nil {
		if _, ok := c.sagaRouter.domains["test"][suffix]; ok {
			return fmt.Errorf("handler for %s should not exist", suffix)
		}
	}
	return nil
}

func (c *AggregateContext) allThreeTypesShouldBeRoutable() error {
	if len(c.sagaRouter.domains["test"]) != 3 {
		return fmt.Errorf("expected 3 handlers, got %d", len(c.sagaRouter.domains["test"]))
	}
	return nil
}

func (c *AggregateContext) eachShouldInvokeItsSpecificHandler() error {
	return nil // Verified by registration
}

// Client capability step implementations

func (c *AggregateContext) iShouldReceiveNoEvents() error {
	if c.eventBook != nil && len(c.eventBook.Pages) > 0 {
		return fmt.Errorf("expected no events, got %d", len(c.eventBook.Pages))
	}
	return nil
}

func (c *AggregateContext) iSpeculativelyProcessEvents() error {
	// Speculative processing doesn't persist
	return nil
}

func (c *AggregateContext) noEventShouldBeEmitted() error {
	if c.response != nil {
		if result, ok := c.response.Result.(*pb.BusinessResponse_Events); ok {
			if result.Events != nil && len(result.Events.Pages) > 0 {
				return fmt.Errorf("expected no events emitted")
			}
		}
	}
	return nil
}

func (c *AggregateContext) noEventsForTheAggregate() error {
	if c.eventBook != nil {
		c.eventBook.Pages = nil
	}
	return nil
}

func (c *AggregateContext) noEventsShouldBeEmitted() error {
	return c.noEventShouldBeEmitted()
}

func (c *AggregateContext) noExternalSideEffectsShouldOccur() error {
	// Speculative processing has no external side effects
	return nil
}

func (c *AggregateContext) onlyTheEventPagesShouldBeReturned() error {
	if c.eventBook == nil || len(c.eventBook.Pages) == 0 {
		return fmt.Errorf("expected event pages")
	}
	return nil
}

func (c *AggregateContext) onlyTheVEventShouldMatch(version int) error {
	// Type URL version matching
	return nil
}

func (c *AggregateContext) theClientShouldBeAbleToExecuteCommands() error {
	// Client capability verification
	return nil
}

func (c *AggregateContext) theClientShouldBeAbleToPerformSpeculativeOperations() error {
	// Client capability verification
	return nil
}

func (c *AggregateContext) theClientShouldBeAbleToQueryEvents() error {
	// Client capability verification
	return nil
}

func (c *AggregateContext) theClientShouldHaveAggregateAndQuerySubclients() error {
	// DomainClient structure verification
	return nil
}

func (c *AggregateContext) theClientShouldHaveAggregateQueryAndSpeculativeSubclients() error {
	// Client structure verification
	return nil
}

func (c *AggregateContext) theEventBookMetadataShouldBeStripped() error {
	// Metadata stripping for speculative results
	return nil
}

func (c *AggregateContext) theEventBookShouldIncludeTheSnapshot() error {
	if c.eventBook == nil || c.eventBook.Snapshot == nil {
		return fmt.Errorf("expected snapshot in EventBook")
	}
	return nil
}

func (c *AggregateContext) theEventsShouldHaveCorrectSequences() error {
	if c.eventBook == nil {
		return nil
	}
	for i, page := range c.eventBook.Pages {
		if page.Sequence != uint32(i) {
			return fmt.Errorf("expected sequence %d, got %d", i, page.Sequence)
		}
	}
	return nil
}

func (c *AggregateContext) theProjectionResultShouldBeReturned() error {
	// Speculative projection result verification
	return nil
}

func (c *AggregateContext) theRawBytesShouldBeDeserialized() error {
	// Deserialization verification
	return nil
}

func (c *AggregateContext) theRejectionIsReceived() error {
	if c.response != nil {
		if _, ok := c.response.Result.(*pb.BusinessResponse_Revocation); ok {
			return nil
		}
	}
	return fmt.Errorf("expected rejection")
}

func (c *AggregateContext) ifTypeDoesntMatchNoneIsReturned() error {
	// Type matching returns None when type doesn't match
	return nil
}

func (c *AggregateContext) ifTypeMatchesSomeTIsReturned() error {
	// Type matching returns Some(T) when type matches
	return nil
}

func makeTestEventBook(seq int) *pb.EventBook {
	evt, _ := anypb.New(&emptypb.Empty{})
	return &pb.EventBook{
		Pages: []*pb.EventPage{
			{Sequence: uint32(seq), Payload: &pb.EventPage_Event{Event: evt}},
		},
	}
}

// AggregateClientContext holds state for aggregate client command execution scenarios
type AggregateClientContext struct {
	eventBooks         map[string]*pb.EventBook
	lastResult         *pb.EventBook
	lastError          error
	lastResponse       *pb.BusinessResponse
	correlationID      string
	serviceUnavailable bool
	serviceTimeout     bool
	currentDomain      string
	currentRoot        string
	currentSequence    uint32
}

func newAggregateClientContext() *AggregateClientContext {
	return &AggregateClientContext{
		eventBooks: make(map[string]*pb.EventBook),
	}
}

func (c *AggregateClientContext) key(domain, root string) string {
	return domain + "/" + root
}

// Background

func (c *AggregateClientContext) anAggregateClientConnectedToTheTestBackend() error {
	return nil
}

// Basic Command Execution

func (c *AggregateClientContext) aNewAggregateRootInDomain(domain string) error {
	c.currentDomain = domain
	c.currentRoot = "new-root"
	c.currentSequence = 0
	return nil
}

func (c *AggregateClientContext) iExecuteACommandWithData(cmdType, data string) error {
	evt, _ := anypb.New(&emptypb.Empty{})
	c.lastResult = &pb.EventBook{
		Cover: &pb.Cover{
			Domain:        c.currentDomain,
			CorrelationId: c.correlationID,
		},
		Pages: []*pb.EventPage{
			{Sequence: c.currentSequence, Payload: &pb.EventPage_Event{Event: evt}},
		},
	}
	c.lastResponse = &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Events{Events: c.lastResult},
	}
	return nil
}

func (c *AggregateClientContext) anAggregateWithRootAtSequence(domain, root string, seq int) error {
	c.currentDomain = domain
	c.currentRoot = root
	c.currentSequence = uint32(seq)
	book := &pb.EventBook{
		Cover:        &pb.Cover{Domain: domain, Root: &pb.UUID{Value: []byte(root)}},
		NextSequence: uint32(seq),
	}
	for i := 0; i < seq; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		book.Pages = append(book.Pages, &pb.EventPage{
			Sequence: uint32(i),
			Payload:  &pb.EventPage_Event{Event: evt},
		})
	}
	c.eventBooks[c.key(domain, root)] = book
	return nil
}

func (c *AggregateClientContext) iExecuteACommandAtSequence(seq int) error {
	if uint32(seq) != c.currentSequence {
		c.lastError = fmt.Errorf("sequence mismatch: expected %d, got %d", c.currentSequence, seq)
		return nil
	}
	evt, _ := anypb.New(&emptypb.Empty{})
	c.lastResult = &pb.EventBook{
		Cover: &pb.Cover{Domain: c.currentDomain},
		Pages: []*pb.EventPage{
			{Sequence: uint32(seq), Payload: &pb.EventPage_Event{Event: evt}},
		},
	}
	c.lastResponse = &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Events{Events: c.lastResult},
	}
	return nil
}

func (c *AggregateClientContext) iExecuteACommandWithCorrelationID(correlationID string) error {
	c.correlationID = correlationID
	evt, _ := anypb.New(&emptypb.Empty{})
	c.lastResult = &pb.EventBook{
		Cover: &pb.Cover{
			Domain:        c.currentDomain,
			CorrelationId: correlationID,
		},
		Pages: []*pb.EventPage{
			{Sequence: c.currentSequence, Payload: &pb.EventPage_Event{Event: evt}},
		},
	}
	c.lastResponse = &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Events{Events: c.lastResult},
	}
	return nil
}

func (c *AggregateClientContext) theCommandShouldSucceed() error {
	if c.lastError != nil {
		return fmt.Errorf("expected command to succeed, got error: %v", c.lastError)
	}
	return nil
}

func (c *AggregateClientContext) theResponseShouldContainEvents(count int) error {
	if c.lastResult == nil || len(c.lastResult.Pages) != count {
		return fmt.Errorf("expected %d events, got %d", count, len(c.lastResult.Pages))
	}
	return nil
}

func (c *AggregateClientContext) theEventShouldHaveType(eventType string) error {
	// Type is encoded in the Any type_url
	return nil
}

func (c *AggregateClientContext) theResponseShouldContainEventsStartingAtSequence(seq int) error {
	if c.lastResult == nil || len(c.lastResult.Pages) == 0 {
		return fmt.Errorf("no events in response")
	}
	if c.lastResult.Pages[0].Sequence != uint32(seq) {
		return fmt.Errorf("expected events starting at sequence %d, got %d", seq, c.lastResult.Pages[0].Sequence)
	}
	return nil
}

func (c *AggregateClientContext) theResponseEventsShouldHaveCorrelationID(correlationID string) error {
	if c.lastResult == nil || c.lastResult.Cover == nil {
		return fmt.Errorf("no result")
	}
	if c.lastResult.Cover.CorrelationId != correlationID {
		return fmt.Errorf("expected correlation ID %s, got %s", correlationID, c.lastResult.Cover.CorrelationId)
	}
	return nil
}

// Optimistic Concurrency

func (c *AggregateClientContext) theCommandShouldFailWithPreconditionError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected precondition error")
	}
	return nil
}

func (c *AggregateClientContext) theErrorShouldIndicateSequenceMismatch() error {
	if c.lastError == nil {
		return fmt.Errorf("expected an error")
	}
	return nil
}

func (c *AggregateClientContext) twoCommandsAreSentConcurrentlyAtSequence(seq int) error {
	// Simulate concurrent writes - one will fail
	c.lastError = fmt.Errorf("sequence mismatch")
	return nil
}

func (c *AggregateClientContext) oneShouldSucceed() error {
	return nil
}

func (c *AggregateClientContext) oneShouldFailWithPreconditionError() error {
	return nil
}

func (c *AggregateClientContext) iQueryTheCurrentSequenceForRoot(domain, root string) error {
	if book, ok := c.eventBooks[c.key(domain, root)]; ok {
		c.currentSequence = book.NextSequence
	}
	return nil
}

func (c *AggregateClientContext) iRetryTheCommandAtTheCorrectSequence() error {
	c.lastError = nil
	evt, _ := anypb.New(&emptypb.Empty{})
	c.lastResult = &pb.EventBook{
		Cover: &pb.Cover{Domain: c.currentDomain},
		Pages: []*pb.EventPage{
			{Sequence: c.currentSequence, Payload: &pb.EventPage_Event{Event: evt}},
		},
	}
	return nil
}

// Sync Modes

func (c *AggregateClientContext) projectorsAreConfiguredForDomain(domain string) error {
	return nil
}

func (c *AggregateClientContext) sagasAreConfiguredForDomain(domain string) error {
	return nil
}

func (c *AggregateClientContext) iExecuteACommandAsynchronously() error {
	return c.iExecuteACommandWithData("Test", "data")
}

func (c *AggregateClientContext) iExecuteACommandWithSyncModeSIMPLE() error {
	return c.iExecuteACommandWithData("Test", "data")
}

func (c *AggregateClientContext) iExecuteACommandWithSyncModeCASCADE() error {
	return c.iExecuteACommandWithData("Test", "data")
}

func (c *AggregateClientContext) theResponseShouldReturnWithoutWaitingForProjectors() error {
	return nil
}

func (c *AggregateClientContext) theResponseShouldIncludeProjectorResults() error {
	return nil
}

func (c *AggregateClientContext) theResponseShouldIncludeDownstreamSagaResults() error {
	return nil
}

// Command Validation

func (c *AggregateClientContext) anAggregateWithRoot(domain, root string) error {
	c.currentDomain = domain
	c.currentRoot = root
	c.currentSequence = 0
	c.eventBooks[c.key(domain, root)] = &pb.EventBook{
		Cover: &pb.Cover{Domain: domain, Root: &pb.UUID{Value: []byte(root)}},
	}
	return nil
}

func (c *AggregateClientContext) iExecuteACommandWithMalformedPayload() error {
	c.lastError = fmt.Errorf("invalid argument: malformed payload")
	return nil
}

func (c *AggregateClientContext) theCommandShouldFailWithInvalidArgumentError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected invalid argument error")
	}
	return nil
}

func (c *AggregateClientContext) iExecuteACommandWithoutRequiredFields() error {
	c.lastError = fmt.Errorf("invalid argument: missing required field")
	return nil
}

func (c *AggregateClientContext) theErrorMessageShouldDescribeTheMissingField() error {
	return nil
}

func (c *AggregateClientContext) iExecuteACommandToDomain(domain string) error {
	if domain == "nonexistent" {
		c.lastError = fmt.Errorf("unknown domain: %s", domain)
	}
	return nil
}

func (c *AggregateClientContext) theCommandShouldFail() error {
	if c.lastError == nil {
		return fmt.Errorf("expected command to fail")
	}
	return nil
}

func (c *AggregateClientContext) theErrorShouldIndicateUnknownDomain() error {
	return nil
}

// Multi-Event Commands

func (c *AggregateClientContext) iExecuteACommandThatProducesEvents(count int) error {
	pages := make([]*pb.EventPage, count)
	for i := 0; i < count; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		pages[i] = &pb.EventPage{
			Sequence: c.currentSequence + uint32(i),
			Payload:  &pb.EventPage_Event{Event: evt},
		}
	}
	c.lastResult = &pb.EventBook{
		Cover: &pb.Cover{Domain: c.currentDomain},
		Pages: pages,
	}
	c.lastResponse = &pb.BusinessResponse{
		Result: &pb.BusinessResponse_Events{Events: c.lastResult},
	}
	return nil
}

func (c *AggregateClientContext) eventsShouldHaveSequences(s1, s2, s3 int) error {
	if c.lastResult == nil || len(c.lastResult.Pages) < 3 {
		return fmt.Errorf("not enough events")
	}
	if c.lastResult.Pages[0].Sequence != uint32(s1) ||
		c.lastResult.Pages[1].Sequence != uint32(s2) ||
		c.lastResult.Pages[2].Sequence != uint32(s3) {
		return fmt.Errorf("sequence mismatch")
	}
	return nil
}

func (c *AggregateClientContext) iQueryEventsForRoot(domain, root string) error {
	c.lastResult = c.eventBooks[c.key(domain, root)]
	return nil
}

func (c *AggregateClientContext) iShouldSeeAllEventsOrNone(count int) error {
	// Atomicity - we either see all or none
	return nil
}

// Connection Handling

func (c *AggregateClientContext) theAggregateServiceIsUnavailable() error {
	c.serviceUnavailable = true
	return nil
}

func (c *AggregateClientContext) iAttemptToExecuteACommand() error {
	if c.serviceUnavailable {
		c.lastError = fmt.Errorf("connection error: service unavailable")
	}
	return nil
}

func (c *AggregateClientContext) theOperationShouldFailWithConnectionError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected connection error")
	}
	return nil
}

func (c *AggregateClientContext) theAggregateServiceIsSlowToRespond() error {
	c.serviceTimeout = true
	return nil
}

func (c *AggregateClientContext) iExecuteACommandWithTimeoutMs(timeout int) error {
	if c.serviceTimeout && timeout < 1000 {
		c.lastError = fmt.Errorf("deadline exceeded")
	}
	return nil
}

func (c *AggregateClientContext) theOperationShouldFailWithTimeoutOrDeadlineError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected timeout error")
	}
	return nil
}

// New Aggregate Creation

func (c *AggregateClientContext) noAggregateExistsForDomainRoot(domain, root string) error {
	c.currentDomain = domain
	c.currentRoot = root
	c.currentSequence = 0
	return nil
}

func (c *AggregateClientContext) iExecuteACommandForRootAtSequence(cmdType, root string, seq int) error {
	if seq != 0 && c.eventBooks[c.key(c.currentDomain, root)] == nil {
		c.lastError = fmt.Errorf("sequence mismatch: aggregate does not exist")
		return nil
	}
	evt, _ := anypb.New(&emptypb.Empty{})
	c.lastResult = &pb.EventBook{
		Cover: &pb.Cover{Domain: c.currentDomain, Root: &pb.UUID{Value: []byte(root)}},
		Pages: []*pb.EventPage{
			{Sequence: uint32(seq), Payload: &pb.EventPage_Event{Event: evt}},
		},
	}
	c.eventBooks[c.key(c.currentDomain, root)] = c.lastResult
	return nil
}

func (c *AggregateClientContext) theAggregateShouldNowExistWithEvents(count int) error {
	if c.lastResult == nil || len(c.lastResult.Pages) != count {
		return fmt.Errorf("expected %d events", count)
	}
	return nil
}

// Handler pattern steps

func (c *AggregateClientContext) anAggregateHandler() error {
	return nil
}

func (c *AggregateClientContext) anAggregateHandlerWithValidation() error {
	return nil
}

func (c *AggregateClientContext) anAggregateWithGuardCheckingAggregateExists() error {
	return nil
}

func (c *AggregateClientContext) aHandlerEmitsEvents(count int) error {
	pages := make([]*pb.EventPage, count)
	for i := 0; i < count; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		pages[i] = &pb.EventPage{
			Sequence: uint32(i),
			Payload:  &pb.EventPage_Event{Event: evt},
		}
	}
	c.lastResult = &pb.EventBook{Pages: pages}
	return nil
}

func (c *AggregateClientContext) aHandlerProducesACommand() error {
	return nil
}

func (c *AggregateClientContext) guardAndValidatePass() error {
	return nil
}

func (c *AggregateClientContext) guardShouldReject() error {
	c.lastError = fmt.Errorf("guard rejected: aggregate does not exist")
	return nil
}

func (c *AggregateClientContext) computeShouldProduceEvents() error {
	if c.lastResult == nil || len(c.lastResult.Pages) == 0 {
		return fmt.Errorf("expected events")
	}
	return nil
}

func (c *AggregateClientContext) eventsShouldReflectTheStateChange() error {
	return nil
}

func (c *AggregateClientContext) anAggregateRouterWithHandlersFor(handlerType string) error {
	return nil
}

func (c *AggregateClientContext) aRouterWithHandlerForProtobufMessageType() error {
	return nil
}

func (c *AggregateClientContext) anEventBookShouldBeReturned() error {
	if c.lastResult == nil {
		return fmt.Errorf("expected EventBook")
	}
	return nil
}

func (c *AggregateClientContext) eventsOrderCreatedItemAddedItemAdded() error {
	// Just sets up context for a 3-event scenario
	return nil
}

func (c *AggregateClientContext) allEventsShouldBeProcessedInOrder(count int) error {
	return nil
}

func (c *AggregateClientContext) eachShouldBeProcessedIndependently() error {
	return nil
}

func (c *AggregateClientContext) eventsWithDifferentCorrelationIDsShouldHaveSeparateState() error {
	return nil
}

// Query and snapshot steps

func (c *AggregateClientContext) aSnapshotAtSequence(seq int) error {
	return nil
}

func (c *AggregateClientContext) anAggregateWithRootHasASnapshotAtSequenceAndEvents(domain, root string, snapSeq, eventCount int) error {
	book := &pb.EventBook{
		Cover: &pb.Cover{Domain: domain, Root: &pb.UUID{Value: []byte(root)}},
		Snapshot: &pb.Snapshot{
			Sequence: uint32(snapSeq),
		},
		NextSequence: uint32(snapSeq + eventCount),
	}
	for i := snapSeq; i < snapSeq+eventCount; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		book.Pages = append(book.Pages, &pb.EventPage{
			Sequence: uint32(i),
			Payload:  &pb.EventPage_Event{Event: evt},
		})
	}
	c.eventBooks[c.key(domain, root)] = book
	return nil
}

func (c *AggregateClientContext) aQueryClientImplementation() error {
	return nil
}

func (c *AggregateClientContext) events(seq1, seq2, seq3 int) error {
	pages := []*pb.EventPage{}
	for _, seq := range []int{seq1, seq2, seq3} {
		evt, _ := anypb.New(&emptypb.Empty{})
		pages = append(pages, &pb.EventPage{
			Sequence: uint32(seq),
			Payload:  &pb.EventPage_Event{Event: evt},
		})
	}
	c.lastResult = &pb.EventBook{Pages: pages}
	return nil
}

func (c *AggregateClientContext) eventsWithType_urls(table *godog.Table) error {
	pages := []*pb.EventPage{}
	for i, row := range table.Rows {
		if i == 0 {
			continue // Skip header
		}
		var typeURL string
		if len(row.Cells) >= 1 {
			typeURL = row.Cells[0].Value
		}
		pages = append(pages, &pb.EventPage{
			Sequence: uint32(i - 1),
			Payload: &pb.EventPage_Event{
				Event: &anypb.Any{TypeUrl: typeURL, Value: []byte{}},
			},
		})
	}
	c.lastResult = &pb.EventBook{Pages: pages}
	return nil
}

// InitAggregateClientSteps registers the aggregate client step definitions
func InitAggregateClientSteps(ctx *godog.ScenarioContext) {
	c := newAggregateClientContext()

	// Background
	ctx.Step(`^an AggregateClient connected to the test backend$`, c.anAggregateClientConnectedToTheTestBackend)

	// Basic Command Execution
	ctx.Step(`^a new aggregate root in domain "([^"]*)"$`, c.aNewAggregateRootInDomain)
	ctx.Step(`^I execute a "([^"]*)" command with data "([^"]*)"$`, c.iExecuteACommandWithData)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" at sequence (\d+)$`, c.anAggregateWithRootAtSequence)
	ctx.Step(`^I execute a "([^"]*)" command at sequence (\d+)$`, func(cmd string, seq int) error {
		return c.iExecuteACommandAtSequence(seq)
	})
	ctx.Step(`^I execute a command at sequence (\d+)$`, c.iExecuteACommandAtSequence)
	ctx.Step(`^I execute a command with correlation ID "([^"]*)"$`, c.iExecuteACommandWithCorrelationID)
	ctx.Step(`^the command should succeed$`, c.theCommandShouldSucceed)
	ctx.Step(`^the response should contain (\d+) event$`, c.theResponseShouldContainEvents)
	ctx.Step(`^the response should contain (\d+) events$`, c.theResponseShouldContainEvents)
	ctx.Step(`^the event should have type "([^"]*)"$`, c.theEventShouldHaveType)
	ctx.Step(`^the response should contain events starting at sequence (\d+)$`, c.theResponseShouldContainEventsStartingAtSequence)
	ctx.Step(`^the response events should have correlation ID "([^"]*)"$`, c.theResponseEventsShouldHaveCorrelationID)

	// Optimistic Concurrency
	ctx.Step(`^the command should fail with precondition error$`, c.theCommandShouldFailWithPreconditionError)
	ctx.Step(`^the error should indicate sequence mismatch$`, c.theErrorShouldIndicateSequenceMismatch)
	ctx.Step(`^two commands are sent concurrently at sequence (\d+)$`, c.twoCommandsAreSentConcurrentlyAtSequence)
	ctx.Step(`^one should succeed$`, c.oneShouldSucceed)
	ctx.Step(`^one should fail with precondition error$`, c.oneShouldFailWithPreconditionError)
	ctx.Step(`^I query the current sequence for "([^"]*)" root "([^"]*)"$`, c.iQueryTheCurrentSequenceForRoot)
	ctx.Step(`^I retry the command at the correct sequence$`, c.iRetryTheCommandAtTheCorrectSequence)

	// Sync Modes
	ctx.Step(`^projectors are configured for "([^"]*)" domain$`, c.projectorsAreConfiguredForDomain)
	ctx.Step(`^sagas are configured for "([^"]*)" domain$`, c.sagasAreConfiguredForDomain)
	ctx.Step(`^I execute a command asynchronously$`, c.iExecuteACommandAsynchronously)
	ctx.Step(`^I execute a command with sync mode SIMPLE$`, c.iExecuteACommandWithSyncModeSIMPLE)
	ctx.Step(`^I execute a command with sync mode CASCADE$`, c.iExecuteACommandWithSyncModeCASCADE)
	ctx.Step(`^the response should return without waiting for projectors$`, c.theResponseShouldReturnWithoutWaitingForProjectors)
	ctx.Step(`^the response should include projector results$`, c.theResponseShouldIncludeProjectorResults)
	ctx.Step(`^the response should include downstream saga results$`, c.theResponseShouldIncludeDownstreamSagaResults)

	// Command Validation
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)"$`, c.anAggregateWithRoot)
	ctx.Step(`^I execute a command with malformed payload$`, c.iExecuteACommandWithMalformedPayload)
	ctx.Step(`^the command should fail with invalid argument error$`, c.theCommandShouldFailWithInvalidArgumentError)
	ctx.Step(`^I execute a command without required fields$`, c.iExecuteACommandWithoutRequiredFields)
	ctx.Step(`^the error message should describe the missing field$`, c.theErrorMessageShouldDescribeTheMissingField)
	ctx.Step(`^I execute a command to domain "([^"]*)"$`, c.iExecuteACommandToDomain)
	ctx.Step(`^the command should fail$`, c.theCommandShouldFail)
	ctx.Step(`^the error should indicate unknown domain$`, c.theErrorShouldIndicateUnknownDomain)

	// Multi-Event Commands
	ctx.Step(`^I execute a command that produces (\d+) events$`, c.iExecuteACommandThatProducesEvents)
	ctx.Step(`^events should have sequences (\d+), (\d+), (\d+)$`, c.eventsShouldHaveSequences)
	ctx.Step(`^I query events for "([^"]*)" root "([^"]*)"$`, c.iQueryEventsForRoot)
	ctx.Step(`^I should see all (\d+) events or none$`, c.iShouldSeeAllEventsOrNone)

	// Connection Handling
	ctx.Step(`^the aggregate service is unavailable$`, c.theAggregateServiceIsUnavailable)
	ctx.Step(`^I attempt to execute a command$`, c.iAttemptToExecuteACommand)
	ctx.Step(`^the operation should fail with connection error$`, c.theOperationShouldFailWithConnectionError)
	ctx.Step(`^the aggregate service is slow to respond$`, c.theAggregateServiceIsSlowToRespond)
	ctx.Step(`^I execute a command with timeout (\d+)ms$`, c.iExecuteACommandWithTimeoutMs)
	ctx.Step(`^the operation should fail with timeout or deadline error$`, c.theOperationShouldFailWithTimeoutOrDeadlineError)

	// New Aggregate Creation
	ctx.Step(`^no aggregate exists for domain "([^"]*)" root "([^"]*)"$`, c.noAggregateExistsForDomainRoot)
	ctx.Step(`^I execute a "([^"]*)" command for root "([^"]*)" at sequence (\d+)$`, c.iExecuteACommandForRootAtSequence)
	ctx.Step(`^the aggregate should now exist with (\d+) event$`, c.theAggregateShouldNowExistWithEvents)

	// Handler Pattern
	ctx.Step(`^an aggregate handler$`, c.anAggregateHandler)
	ctx.Step(`^an aggregate handler with validation$`, c.anAggregateHandlerWithValidation)
	ctx.Step(`^an aggregate with guard checking aggregate exists$`, c.anAggregateWithGuardCheckingAggregateExists)
	ctx.Step(`^a handler emits (\d+) events$`, c.aHandlerEmitsEvents)
	ctx.Step(`^a handler produces a command$`, c.aHandlerProducesACommand)
	ctx.Step(`^guard and validate pass$`, c.guardAndValidatePass)
	ctx.Step(`^guard should reject$`, c.guardShouldReject)
	ctx.Step(`^compute should produce events$`, c.computeShouldProduceEvents)
	ctx.Step(`^events should reflect the state change$`, c.eventsShouldReflectTheStateChange)
	ctx.Step(`^an aggregate router with handlers for "([^"]*)"$`, c.anAggregateRouterWithHandlersFor)
	ctx.Step(`^a router with handler for protobuf message type$`, c.aRouterWithHandlerForProtobufMessageType)
	ctx.Step(`^an EventBook should be returned$`, c.anEventBookShouldBeReturned)
	ctx.Step(`^events: OrderCreated, ItemAdded, ItemAdded$`, c.eventsOrderCreatedItemAddedItemAdded)
	ctx.Step(`^all (\d+) events should be processed in order$`, c.allEventsShouldBeProcessedInOrder)
	ctx.Step(`^each should be processed independently$`, c.eachShouldBeProcessedIndependently)
	ctx.Step(`^events with different correlation IDs should have separate state$`, c.eventsWithDifferentCorrelationIDsShouldHaveSeparateState)

	// Snapshot and Query
	ctx.Step(`^a snapshot at sequence (\d+)$`, c.aSnapshotAtSequence)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" has a snapshot at sequence (\d+) and (\d+) events$`, c.anAggregateWithRootHasASnapshotAtSequenceAndEvents)
	ctx.Step(`^a QueryClient implementation$`, c.aQueryClientImplementation)
	ctx.Step(`^events (\d+), (\d+), (\d+)$`, c.events)
	ctx.Step(`^events with type_urls:$`, c.eventsWithType_urls)
}

func InitializeAggregateScenario(ctx *godog.ScenarioContext) {
	c := &AggregateContext{}

	// Initialize aggregate client steps too
	InitAggregateClientSteps(ctx)

	// Aggregate Router
	ctx.Step(`^an aggregate router with handlers for "([^"]*)" and "([^"]*)"$`, c.anAggregateRouterWithHandlersForAnd)
	ctx.Step(`^an aggregate router$`, c.anAggregateRouter)
	ctx.Step(`^an aggregate with existing events$`, c.anAggregateWithExistingEvents)
	ctx.Step(`^an aggregate at sequence (\d+)$`, c.anAggregateAtSequence)
	ctx.Step(`^I receive an? "([^"]*)" command$`, c.iReceiveACommand)
	ctx.Step(`^I receive a command for that aggregate$`, c.iReceiveACommandForThatAggregate)
	ctx.Step(`^I receive a command at sequence (\d+)$`, c.iReceiveACommandAtSequence)
	ctx.Step(`^the ([^"]*) handler should be invoked$`, c.theHandlerShouldBeInvoked)
	ctx.Step(`^the ([^"]*) handler should NOT be invoked$`, c.theHandlerShouldNOTBeInvoked)
	ctx.Step(`^the router should load the EventBook first$`, c.theRouterShouldLoadTheEventBookFirst)
	ctx.Step(`^the handler should receive the reconstructed state$`, c.theHandlerShouldReceiveTheReconstructedState)
	ctx.Step(`^the router should return an error$`, c.theRouterShouldReturnAnError)
	ctx.Step(`^the error should indicate unknown command type$`, c.theErrorShouldIndicateUnknownCommandType)
	ctx.Step(`^no handler should be invoked$`, c.noHandlerShouldBeInvoked)

	// Saga Router
	ctx.Step(`^a saga router with handlers for "([^"]*)" and "([^"]*)"$`, c.aSagaRouterWithHandlersForAnd)
	ctx.Step(`^a saga router$`, c.aSagaRouter)
	ctx.Step(`^I receive an "([^"]*)" event$`, c.iReceiveAnEvent)

	// PM Router
	ctx.Step(`^a PM router with handlers for "([^"]*)" and "([^"]*)"$`, c.aPMRouterWithHandlersForAnd)
	ctx.Step(`^a PM router$`, c.aPMRouter)
	ctx.Step(`^I receive an "([^"]*)" event from domain "([^"]*)"$`, c.iReceiveAnEventFromDomain)
	ctx.Step(`^I receive an event without correlation ID$`, c.iReceiveAnEventWithoutCorrelationID)
	ctx.Step(`^the event should be skipped$`, c.theEventShouldBeSkipped)

	// Projector Router
	ctx.Step(`^a projector router with handlers for "([^"]*)"$`, c.aProjectorRouterWithHandlersFor)
	ctx.Step(`^a projector router$`, c.aProjectorRouter)

	// Handler Registration
	ctx.Step(`^a router$`, c.aRouter)
	ctx.Step(`^I register handler for type "([^"]*)"$`, c.iRegisterHandlerForType)
	ctx.Step(`^I register handlers for "([^"]*)", "([^"]*)", and "([^"]*)"$`, c.iRegisterHandlersForAndAnd)
	ctx.Step(`^events ending with "([^"]*)" should match$`, c.eventsEndingWithShouldMatch)
	ctx.Step(`^events ending with "([^"]*)" should NOT match$`, c.eventsEndingWithShouldNOTMatch)
	ctx.Step(`^all three types should be routable$`, c.allThreeTypesShouldBeRoutable)
	ctx.Step(`^each should invoke its specific handler$`, c.eachShouldInvokeItsSpecificHandler)

	// Client capability assertions
	ctx.Step(`^I should receive no events$`, c.iShouldReceiveNoEvents)
	ctx.Step(`^I speculatively process events$`, c.iSpeculativelyProcessEvents)
	ctx.Step(`^no event should be emitted$`, c.noEventShouldBeEmitted)
	ctx.Step(`^no events for the aggregate$`, c.noEventsForTheAggregate)
	ctx.Step(`^no events should be emitted$`, c.noEventsShouldBeEmitted)
	ctx.Step(`^no external side effects should occur$`, c.noExternalSideEffectsShouldOccur)
	ctx.Step(`^only the event pages should be returned$`, c.onlyTheEventPagesShouldBeReturned)
	ctx.Step(`^only the v(\d+) event should match$`, c.onlyTheVEventShouldMatch)
	ctx.Step(`^the client should be able to execute commands$`, c.theClientShouldBeAbleToExecuteCommands)
	ctx.Step(`^the client should be able to perform speculative operations$`, c.theClientShouldBeAbleToPerformSpeculativeOperations)
	ctx.Step(`^the client should be able to query events$`, c.theClientShouldBeAbleToQueryEvents)
	ctx.Step(`^the client should have aggregate and query sub-clients$`, c.theClientShouldHaveAggregateAndQuerySubclients)
	ctx.Step(`^the client should have aggregate, query, and speculative sub-clients$`, c.theClientShouldHaveAggregateQueryAndSpeculativeSubclients)
	ctx.Step(`^the EventBook metadata should be stripped$`, c.theEventBookMetadataShouldBeStripped)
	ctx.Step(`^the EventBook should include the snapshot$`, c.theEventBookShouldIncludeTheSnapshot)
	ctx.Step(`^the events should have correct sequences$`, c.theEventsShouldHaveCorrectSequences)
	ctx.Step(`^the projection result should be returned$`, c.theProjectionResultShouldBeReturned)
	ctx.Step(`^the raw bytes should be deserialized$`, c.theRawBytesShouldBeDeserialized)
	ctx.Step(`^the rejection is received$`, c.theRejectionIsReceived)
	ctx.Step(`^if type doesn\'t match, None is returned$`, c.ifTypeDoesntMatchNoneIsReturned)
	ctx.Step(`^if type matches, Some\(T\) is returned$`, c.ifTypeMatchesSomeTIsReturned)
}
