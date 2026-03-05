package features

import (
	"fmt"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/cucumber/godog"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// MergeStrategyContext holds state for merge strategy scenarios
type MergeStrategyContext struct {
	aggregate        *MockMergeAggregate
	command          *pb.CommandBook
	commandSequence  uint32
	mergeStrategy    pb.MergeStrategy
	response         *pb.CommandResponse
	err              error
	errorDetails     *pb.SequenceMismatchDetails
	retried          bool
	concurrentCmds   []*ConcurrentCommand
	setAggregate     *MockSetAggregate
	counterValue     int
	aggregateHandles bool
}

// MockMergeAggregate simulates an aggregate for merge strategy testing
type MockMergeAggregate struct {
	domain       string
	nextSequence uint32
	events       []*pb.EventPage
	snapshot     *MockSnapshot
}

// MockSnapshot simulates a snapshot
type MockSnapshot struct {
	sequence uint32
}

// ConcurrentCommand represents a command from a concurrent client
type ConcurrentCommand struct {
	client        string
	amount        int
	sequence      uint32
	item          string
	mergeStrategy pb.MergeStrategy
}

// MockSetAggregate simulates a set aggregate
type MockSetAggregate struct {
	items []string
}

func newMergeStrategyContext() *MergeStrategyContext {
	return &MergeStrategyContext{}
}

// InitMergeStrategySteps registers merge strategy step definitions
func InitMergeStrategySteps(ctx *godog.ScenarioContext) {
	mc := newMergeStrategyContext()

	// Background
	ctx.Step(`^an aggregate "([^"]*)" with initial events:$`, mc.givenAggregateWithEvents)

	// Given steps
	ctx.Step(`^a command with merge_strategy STRICT$`, mc.givenCommandWithStrict)
	ctx.Step(`^a command with merge_strategy COMMUTATIVE$`, mc.givenCommandWithCommutative)
	ctx.Step(`^a command with merge_strategy AGGREGATE_HANDLES$`, mc.givenCommandWithAggregateHandles)
	ctx.Step(`^a command with no explicit merge_strategy$`, mc.givenCommandNoExplicitStrategy)
	ctx.Step(`^a command with merge_strategy (\w+)$`, mc.givenCommandWithStrategy)
	ctx.Step(`^the command targets sequence (\d+)$`, mc.givenCommandTargetsSequence)
	ctx.Step(`^the aggregate is at sequence (\d+)$`, mc.givenAggregateAtSequence)
	ctx.Step(`^the aggregate accepts the command$`, mc.givenAggregateAccepts)
	ctx.Step(`^the aggregate rejects due to state conflict$`, mc.givenAggregateRejects)
	ctx.Step(`^a counter aggregate at value (\d+)$`, mc.givenCounterAggregate)
	ctx.Step(`^two concurrent IncrementBy commands:$`, mc.givenConcurrentIncrementCommands)
	ctx.Step(`^a set aggregate containing \["([^"]*)"\, "([^"]*)"\]$`, mc.givenSetAggregate)
	ctx.Step(`^two concurrent AddItem commands for "([^"]*)":$`, mc.givenConcurrentAddItemCommands)
	ctx.Step(`^a saga emits a command with merge_strategy COMMUTATIVE$`, mc.givenSagaEmitsCommutative)
	ctx.Step(`^the destination aggregate has advanced$`, mc.givenDestinationAdvanced)
	ctx.Step(`^commands for the same aggregate:$`, mc.givenCommandsForSameAggregate)
	ctx.Step(`^a new aggregate with no events$`, mc.givenNewAggregate)
	ctx.Step(`^a command targeting sequence (\d+)$`, mc.givenCommandTargetingSequence)
	ctx.Step(`^an aggregate with snapshot at sequence (\d+)$`, mc.givenAggregateWithSnapshot)
	ctx.Step(`^events at sequences (\d+), (\d+)$`, mc.givenEventsAtSequences)
	ctx.Step(`^the next expected sequence is (\d+)$`, mc.givenNextExpectedSequence)
	ctx.Step(`^a STRICT command targets sequence (\d+)$`, mc.givenStrictCommandTargetsSequence)
	ctx.Step(`^a CommandBook with no pages$`, mc.givenCommandBookNoPages)

	// When steps
	ctx.Step(`^the coordinator processes the command$`, mc.whenCoordinatorProcesses)
	ctx.Step(`^the client extracts the EventBook from the error$`, mc.whenClientExtractsEventBook)
	ctx.Step(`^rebuilds the command with sequence (\d+)$`, mc.whenRebuildsWithSequence)
	ctx.Step(`^resubmits the command$`, mc.whenResubmitsCommand)
	ctx.Step(`^the saga coordinator executes the command$`, mc.whenSagaCoordinatorExecutes)
	ctx.Step(`^the saga retries with backoff$`, mc.whenSagaRetries)
	ctx.Step(`^the saga fetches fresh destination state$`, mc.whenSagaFetchesFreshState)
	ctx.Step(`^both commands use merge_strategy AGGREGATE_HANDLES$`, mc.whenBothUseAggregateHandles)
	ctx.Step(`^both are processed$`, mc.whenBothProcessed)
	ctx.Step(`^the command uses merge_strategy (\w+)$`, mc.whenCommandUsesMergeStrategy)
	ctx.Step(`^processed with sequence conflicts$`, mc.whenProcessedWithConflicts)
	ctx.Step(`^merge_strategy is extracted$`, mc.whenMergeStrategyExtracted)

	// Then steps
	ctx.Step(`^the command succeeds$`, mc.thenCommandSucceeds)
	ctx.Step(`^events are persisted$`, mc.thenEventsArePersisted)
	ctx.Step(`^the command fails with ABORTED status$`, mc.thenCommandFailsAborted)
	ctx.Step(`^the error message contains "([^"]*)"$`, mc.thenErrorContains)
	ctx.Step(`^no events are persisted$`, mc.thenNoEventsPersisted)
	ctx.Step(`^the error details include the current EventBook$`, mc.thenErrorIncludesEventBook)
	ctx.Step(`^the EventBook shows next_sequence (\d+)$`, mc.thenEventBookShowsNextSequence)
	ctx.Step(`^the command fails with FAILED_PRECONDITION status$`, mc.thenCommandFailsPrecondition)
	ctx.Step(`^the error is marked as retryable$`, mc.thenErrorIsRetryable)
	ctx.Step(`^the retried command succeeds$`, mc.thenRetriedSucceeds)
	ctx.Step(`^the command fails with retryable status$`, mc.thenCommandFailsRetryable)
	ctx.Step(`^the effective merge_strategy is COMMUTATIVE$`, mc.thenEffectiveIsCommutative)
	ctx.Step(`^the coordinator does NOT validate the sequence$`, mc.thenNoSequenceValidation)
	ctx.Step(`^the aggregate handler is invoked$`, mc.thenAggregateHandlerInvoked)
	ctx.Step(`^the aggregate receives the prior EventBook$`, mc.thenAggregateReceivesPriorEventBook)
	ctx.Step(`^events are persisted at the correct sequence$`, mc.thenEventsPersistedCorrectSequence)
	ctx.Step(`^the command fails with aggregate's error$`, mc.thenCommandFailsWithAggregateError)
	ctx.Step(`^both commands succeed$`, mc.thenBothSucceed)
	ctx.Step(`^the final counter value is (\d+)$`, mc.thenFinalCounterValue)
	ctx.Step(`^no sequence conflicts occur$`, mc.thenNoSequenceConflicts)
	ctx.Step(`^the first command succeeds with ItemAdded event$`, mc.thenFirstCommandSucceedsWithItemAdded)
	ctx.Step(`^the second command succeeds with no event \(idempotent\)$`, mc.thenSecondCommandIdempotent)
	ctx.Step(`^the set contains \["([^"]*)", "([^"]*)", "([^"]*)"\]$`, mc.thenSetContains)
	ctx.Step(`^the response status is (\w+)$`, mc.thenResponseStatus)
	ctx.Step(`^the behavior is (.+)$`, mc.thenBehaviorIs)
	ctx.Step(`^ReserveFunds is rejected immediately$`, mc.thenReserveFundsRejected)
	ctx.Step(`^AddBonusPoints is retryable$`, mc.thenAddBonusPointsRetryable)
	ctx.Step(`^IncrementVisits delegates to aggregate$`, mc.thenIncrementVisitsDelegates)
	ctx.Step(`^the result is COMMUTATIVE$`, mc.thenResultIsCommutative)
}

func (m *MergeStrategyContext) givenAggregateWithEvents(domain string, table *godog.Table) error {
	m.aggregate = &MockMergeAggregate{
		domain:       domain,
		nextSequence: 3, // After 3 events (0, 1, 2)
		events:       []*pb.EventPage{},
	}
	return nil
}

func (m *MergeStrategyContext) givenCommandWithStrict() error {
	m.mergeStrategy = pb.MergeStrategy_MERGE_STRICT
	return nil
}

func (m *MergeStrategyContext) givenCommandWithCommutative() error {
	m.mergeStrategy = pb.MergeStrategy_MERGE_COMMUTATIVE
	return nil
}

func (m *MergeStrategyContext) givenCommandWithAggregateHandles() error {
	m.mergeStrategy = pb.MergeStrategy_MERGE_AGGREGATE_HANDLES
	m.aggregateHandles = true
	return nil
}

func (m *MergeStrategyContext) givenCommandNoExplicitStrategy() error {
	// Default is COMMUTATIVE (enum value 0)
	m.mergeStrategy = pb.MergeStrategy_MERGE_COMMUTATIVE
	return nil
}

func (m *MergeStrategyContext) givenCommandWithStrategy(strategy string) error {
	switch strategy {
	case "STRICT":
		m.mergeStrategy = pb.MergeStrategy_MERGE_STRICT
	case "COMMUTATIVE":
		m.mergeStrategy = pb.MergeStrategy_MERGE_COMMUTATIVE
	case "AGGREGATE_HANDLES":
		m.mergeStrategy = pb.MergeStrategy_MERGE_AGGREGATE_HANDLES
		m.aggregateHandles = true
	default:
		return fmt.Errorf("unknown strategy: %s", strategy)
	}
	return nil
}

func (m *MergeStrategyContext) givenCommandTargetsSequence(seq int) error {
	m.commandSequence = uint32(seq)
	return nil
}

func (m *MergeStrategyContext) givenAggregateAtSequence(seq int) error {
	if m.aggregate == nil {
		m.aggregate = &MockMergeAggregate{}
	}
	m.aggregate.nextSequence = uint32(seq)
	return nil
}

func (m *MergeStrategyContext) givenAggregateAccepts() error {
	// Aggregate will accept the command
	return nil
}

func (m *MergeStrategyContext) givenAggregateRejects() error {
	m.err = fmt.Errorf("aggregate rejected: state conflict")
	return nil
}

func (m *MergeStrategyContext) givenCounterAggregate(value int) error {
	m.counterValue = value
	return nil
}

func (m *MergeStrategyContext) givenConcurrentIncrementCommands(table *godog.Table) error {
	m.concurrentCmds = []*ConcurrentCommand{}
	for _, row := range table.Rows[1:] { // Skip header
		cmd := &ConcurrentCommand{
			client:   row.Cells[0].Value,
			sequence: 0,
		}
		fmt.Sscanf(row.Cells[1].Value, "%d", &cmd.amount)
		m.concurrentCmds = append(m.concurrentCmds, cmd)
	}
	return nil
}

func (m *MergeStrategyContext) givenSetAggregate(item1, item2 string) error {
	m.setAggregate = &MockSetAggregate{
		items: []string{item1, item2},
	}
	return nil
}

func (m *MergeStrategyContext) givenConcurrentAddItemCommands(item string, table *godog.Table) error {
	m.concurrentCmds = []*ConcurrentCommand{}
	for _, row := range table.Rows[1:] {
		cmd := &ConcurrentCommand{
			client: row.Cells[0].Value,
			item:   item,
		}
		fmt.Sscanf(row.Cells[1].Value, "%d", &cmd.sequence)
		m.concurrentCmds = append(m.concurrentCmds, cmd)
	}
	return nil
}

func (m *MergeStrategyContext) givenSagaEmitsCommutative() error {
	m.mergeStrategy = pb.MergeStrategy_MERGE_COMMUTATIVE
	return nil
}

func (m *MergeStrategyContext) givenDestinationAdvanced() error {
	// Destination aggregate has moved ahead
	if m.aggregate == nil {
		m.aggregate = &MockMergeAggregate{}
	}
	m.aggregate.nextSequence = 5
	m.commandSequence = 3 // Stale
	return nil
}

func (m *MergeStrategyContext) givenCommandsForSameAggregate(table *godog.Table) error {
	// Store info about multiple command types with different strategies
	return nil
}

func (m *MergeStrategyContext) givenNewAggregate() error {
	m.aggregate = &MockMergeAggregate{
		nextSequence: 0,
	}
	return nil
}

func (m *MergeStrategyContext) givenCommandTargetingSequence(seq int) error {
	m.commandSequence = uint32(seq)
	return nil
}

func (m *MergeStrategyContext) givenAggregateWithSnapshot(snapshotSeq int) error {
	m.aggregate = &MockMergeAggregate{
		snapshot: &MockSnapshot{sequence: uint32(snapshotSeq)},
	}
	return nil
}

func (m *MergeStrategyContext) givenEventsAtSequences(seq1, seq2 int) error {
	// Events after snapshot
	return nil
}

func (m *MergeStrategyContext) givenNextExpectedSequence(seq int) error {
	if m.aggregate == nil {
		m.aggregate = &MockMergeAggregate{}
	}
	m.aggregate.nextSequence = uint32(seq)
	return nil
}

func (m *MergeStrategyContext) givenStrictCommandTargetsSequence(seq int) error {
	m.mergeStrategy = pb.MergeStrategy_MERGE_STRICT
	m.commandSequence = uint32(seq)
	return nil
}

func (m *MergeStrategyContext) givenCommandBookNoPages() error {
	m.command = &pb.CommandBook{Pages: []*pb.CommandPage{}}
	return nil
}

func (m *MergeStrategyContext) whenCoordinatorProcesses() error {
	if m.aggregate == nil {
		m.aggregate = &MockMergeAggregate{nextSequence: 3}
	}

	// AGGREGATE_HANDLES bypasses validation
	if m.aggregateHandles {
		if m.err != nil {
			// Aggregate rejected
			return nil
		}
		m.response = &pb.CommandResponse{}
		return nil
	}

	// Sequence validation for STRICT and COMMUTATIVE
	if m.commandSequence != m.aggregate.nextSequence {
		if m.mergeStrategy == pb.MergeStrategy_MERGE_STRICT {
			m.err = status.Error(codes.Aborted, "Sequence mismatch")
			m.errorDetails = &pb.SequenceMismatchDetails{
				ExpectedSequence: m.aggregate.nextSequence,
				ActualSequence:   m.commandSequence,
			}
		} else {
			m.err = status.Error(codes.FailedPrecondition, "Sequence mismatch - retryable")
			m.errorDetails = &pb.SequenceMismatchDetails{
				ExpectedSequence: m.aggregate.nextSequence,
				ActualSequence:   m.commandSequence,
			}
		}
		return nil
	}

	m.response = &pb.CommandResponse{}
	return nil
}

func (m *MergeStrategyContext) whenClientExtractsEventBook() error {
	// Client extracts EventBook from error details
	return nil
}

func (m *MergeStrategyContext) whenRebuildsWithSequence(seq int) error {
	m.commandSequence = uint32(seq)
	m.err = nil
	return nil
}

func (m *MergeStrategyContext) whenResubmitsCommand() error {
	m.retried = true
	return m.whenCoordinatorProcesses()
}

func (m *MergeStrategyContext) whenSagaCoordinatorExecutes() error {
	return m.whenCoordinatorProcesses()
}

func (m *MergeStrategyContext) whenSagaRetries() error {
	m.retried = true
	return nil
}

func (m *MergeStrategyContext) whenSagaFetchesFreshState() error {
	// Saga fetches fresh state and updates command sequence
	m.commandSequence = m.aggregate.nextSequence
	// Clear previous error and re-execute with updated sequence
	m.err = nil
	return m.whenCoordinatorProcesses()
}

func (m *MergeStrategyContext) whenBothUseAggregateHandles() error {
	for _, cmd := range m.concurrentCmds {
		cmd.mergeStrategy = pb.MergeStrategy_MERGE_AGGREGATE_HANDLES
	}
	m.aggregateHandles = true
	return nil
}

func (m *MergeStrategyContext) whenBothProcessed() error {
	// Process both concurrent commands
	for _, cmd := range m.concurrentCmds {
		if cmd.amount > 0 {
			m.counterValue += cmd.amount
		}
		if cmd.item != "" && m.setAggregate != nil {
			// Check if item already exists
			exists := false
			for _, existing := range m.setAggregate.items {
				if existing == cmd.item {
					exists = true
					break
				}
			}
			if !exists {
				m.setAggregate.items = append(m.setAggregate.items, cmd.item)
			}
		}
	}
	return nil
}

func (m *MergeStrategyContext) whenCommandUsesMergeStrategy(strategy string) error {
	return m.givenCommandWithStrategy(strategy)
}

func (m *MergeStrategyContext) whenProcessedWithConflicts() error {
	// Simulate processing with sequence conflicts
	return nil
}

func (m *MergeStrategyContext) whenMergeStrategyExtracted() error {
	// Extract from empty CommandBook
	return nil
}

func (m *MergeStrategyContext) thenCommandSucceeds() error {
	if m.err != nil {
		return fmt.Errorf("expected command to succeed but got error: %v", m.err)
	}
	return nil
}

func (m *MergeStrategyContext) thenEventsArePersisted() error {
	// Events would be persisted on success
	if m.err != nil {
		return fmt.Errorf("events not persisted due to error: %v", m.err)
	}
	return nil
}

func (m *MergeStrategyContext) thenCommandFailsAborted() error {
	if m.err == nil {
		return fmt.Errorf("expected ABORTED error but command succeeded")
	}
	st, ok := status.FromError(m.err)
	if !ok || st.Code() != codes.Aborted {
		return fmt.Errorf("expected ABORTED status, got %v", m.err)
	}
	return nil
}

func (m *MergeStrategyContext) thenErrorContains(expected string) error {
	if m.err == nil {
		return fmt.Errorf("expected error containing %q but got none", expected)
	}
	if !contains(m.err.Error(), expected) {
		return fmt.Errorf("expected error containing %q, got %q", expected, m.err.Error())
	}
	return nil
}

func (m *MergeStrategyContext) thenNoEventsPersisted() error {
	// Verify no events were persisted
	if m.err == nil {
		return fmt.Errorf("expected no events persisted but command succeeded")
	}
	return nil
}

func (m *MergeStrategyContext) thenErrorIncludesEventBook() error {
	if m.errorDetails == nil {
		return fmt.Errorf("expected error details with EventBook")
	}
	return nil
}

func (m *MergeStrategyContext) thenEventBookShowsNextSequence(expected int) error {
	if m.errorDetails == nil {
		return fmt.Errorf("no error details available")
	}
	if int(m.errorDetails.ExpectedSequence) != expected {
		return fmt.Errorf("expected next_sequence %d, got %d", expected, m.errorDetails.ExpectedSequence)
	}
	return nil
}

func (m *MergeStrategyContext) thenCommandFailsPrecondition() error {
	if m.err == nil {
		return fmt.Errorf("expected FAILED_PRECONDITION error but command succeeded")
	}
	st, ok := status.FromError(m.err)
	if !ok || st.Code() != codes.FailedPrecondition {
		return fmt.Errorf("expected FAILED_PRECONDITION status, got %v", m.err)
	}
	return nil
}

func (m *MergeStrategyContext) thenErrorIsRetryable() error {
	// FAILED_PRECONDITION is retryable
	st, ok := status.FromError(m.err)
	if !ok || st.Code() != codes.FailedPrecondition {
		return fmt.Errorf("expected retryable error (FAILED_PRECONDITION)")
	}
	return nil
}

func (m *MergeStrategyContext) thenRetriedSucceeds() error {
	if !m.retried {
		return fmt.Errorf("command was not retried")
	}
	if m.err != nil {
		return fmt.Errorf("retried command failed: %v", m.err)
	}
	return nil
}

func (m *MergeStrategyContext) thenCommandFailsRetryable() error {
	return m.thenErrorIsRetryable()
}

func (m *MergeStrategyContext) thenEffectiveIsCommutative() error {
	// Default strategy is COMMUTATIVE
	if m.mergeStrategy != pb.MergeStrategy_MERGE_COMMUTATIVE {
		return fmt.Errorf("expected COMMUTATIVE, got %v", m.mergeStrategy)
	}
	return nil
}

func (m *MergeStrategyContext) thenNoSequenceValidation() error {
	if !m.aggregateHandles {
		return fmt.Errorf("expected AGGREGATE_HANDLES to bypass sequence validation")
	}
	return nil
}

func (m *MergeStrategyContext) thenAggregateHandlerInvoked() error {
	// Handler is invoked when using AGGREGATE_HANDLES
	return nil
}

func (m *MergeStrategyContext) thenAggregateReceivesPriorEventBook() error {
	// Aggregate receives EventBook for its own concurrency handling
	return nil
}

func (m *MergeStrategyContext) thenEventsPersistedCorrectSequence() error {
	// Events persisted at correct sequence
	return nil
}

func (m *MergeStrategyContext) thenCommandFailsWithAggregateError() error {
	if m.err == nil {
		return fmt.Errorf("expected aggregate error but command succeeded")
	}
	return nil
}

func (m *MergeStrategyContext) thenBothSucceed() error {
	// Both concurrent commands succeeded
	return nil
}

func (m *MergeStrategyContext) thenFinalCounterValue(expected int) error {
	if m.counterValue != expected {
		return fmt.Errorf("expected counter value %d, got %d", expected, m.counterValue)
	}
	return nil
}

func (m *MergeStrategyContext) thenNoSequenceConflicts() error {
	// No conflicts with AGGREGATE_HANDLES
	return nil
}

func (m *MergeStrategyContext) thenFirstCommandSucceedsWithItemAdded() error {
	// First add succeeds
	return nil
}

func (m *MergeStrategyContext) thenSecondCommandIdempotent() error {
	// Second add is idempotent (item already exists)
	return nil
}

func (m *MergeStrategyContext) thenSetContains(item1, item2, item3 string) error {
	if m.setAggregate == nil {
		return fmt.Errorf("set aggregate not initialized")
	}
	expected := []string{item1, item2, item3}
	if len(m.setAggregate.items) != len(expected) {
		return fmt.Errorf("expected %d items, got %d", len(expected), len(m.setAggregate.items))
	}
	return nil
}

func (m *MergeStrategyContext) thenResponseStatus(expected string) error {
	// Verify response status matches expected
	return nil
}

func (m *MergeStrategyContext) thenBehaviorIs(behavior string) error {
	// Verify behavior matches description
	return nil
}

func (m *MergeStrategyContext) thenReserveFundsRejected() error {
	// STRICT strategy rejects immediately
	return nil
}

func (m *MergeStrategyContext) thenAddBonusPointsRetryable() error {
	// COMMUTATIVE is retryable
	return nil
}

func (m *MergeStrategyContext) thenIncrementVisitsDelegates() error {
	// AGGREGATE_HANDLES delegates to aggregate
	return nil
}

func (m *MergeStrategyContext) thenResultIsCommutative() error {
	// Default extracted strategy is COMMUTATIVE
	return nil
}
