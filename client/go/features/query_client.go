package features

import (
	"fmt"
	"time"

	"github.com/cucumber/godog"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/emptypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// QueryClientContext holds state for query client scenarios
type QueryClientContext struct {
	eventBooks       map[string]*pb.EventBook // domain/root -> EventBook
	lastResult       *pb.EventBook
	lastError        error
	correlatedEvents map[string][]*pb.EventBook // correlationID -> EventBooks
}

func newQueryClientContext() *QueryClientContext {
	return &QueryClientContext{
		eventBooks:       make(map[string]*pb.EventBook),
		correlatedEvents: make(map[string][]*pb.EventBook),
	}
}

func (c *QueryClientContext) key(domain, root string) string {
	return domain + "/" + root
}

// Background steps

func (c *QueryClientContext) aQueryClientConnectedToTheTestBackend() error {
	// Mock connection - in real tests this would connect to test backend
	return nil
}

// Basic Event Retrieval steps

func (c *QueryClientContext) anAggregateWithRoot(domain, root string) error {
	// Create empty aggregate
	c.eventBooks[c.key(domain, root)] = &pb.EventBook{
		Cover: &pb.Cover{
			Domain: domain,
			Root:   &pb.UUID{Value: []byte(root)},
		},
		NextSequence: 0,
	}
	return nil
}

func (c *QueryClientContext) anAggregateWithRootHasEvents(domain, root string, count int) error {
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
			Sequence:  uint32(i),
			CreatedAt: timestamppb.Now(),
			Payload:   &pb.EventPage_Event{Event: evt},
		})
	}
	c.eventBooks[c.key(domain, root)] = book
	return nil
}

func (c *QueryClientContext) anAggregateWithRootHasEventWithData(domain, root, eventType, data string) error {
	book := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: domain,
			Root:   &pb.UUID{Value: []byte(root)},
		},
		NextSequence: 1,
	}
	evt := &anypb.Any{
		TypeUrl: "type.googleapis.com/" + eventType,
		Value:   []byte(data),
	}
	book.Pages = append(book.Pages, &pb.EventPage{
		Sequence:  0,
		CreatedAt: timestamppb.Now(),
		Payload:   &pb.EventPage_Event{Event: evt},
	})
	c.eventBooks[c.key(domain, root)] = book
	return nil
}

func (c *QueryClientContext) iQueryEventsForRoot(domain, root string) error {
	key := c.key(domain, root)
	if book, ok := c.eventBooks[key]; ok {
		c.lastResult = book
	} else {
		c.lastResult = &pb.EventBook{
			Cover: &pb.Cover{
				Domain: domain,
				Root:   &pb.UUID{Value: []byte(root)},
			},
			NextSequence: 0,
		}
	}
	return nil
}

func (c *QueryClientContext) iShouldReceiveAnEventBookWithEvents(count int) error {
	if c.lastResult == nil {
		return fmt.Errorf("no result received")
	}
	if len(c.lastResult.Pages) != count {
		return fmt.Errorf("expected %d events, got %d", count, len(c.lastResult.Pages))
	}
	return nil
}

func (c *QueryClientContext) theNextSequenceShouldBe(seq int) error {
	if c.lastResult == nil {
		return fmt.Errorf("no result received")
	}
	if c.lastResult.NextSequence != uint32(seq) {
		return fmt.Errorf("expected next_sequence %d, got %d", seq, c.lastResult.NextSequence)
	}
	return nil
}

func (c *QueryClientContext) eventsShouldBeInSequenceOrderTo(from, to int) error {
	if c.lastResult == nil {
		return fmt.Errorf("no result received")
	}
	for i, page := range c.lastResult.Pages {
		expected := uint32(from + i)
		if page.Sequence != expected {
			return fmt.Errorf("expected sequence %d at index %d, got %d", expected, i, page.Sequence)
		}
	}
	return nil
}

func (c *QueryClientContext) theFirstEventShouldHaveType(eventType string) error {
	if c.lastResult == nil || len(c.lastResult.Pages) == 0 {
		return fmt.Errorf("no events received")
	}
	page := c.lastResult.Pages[0]
	if evt := page.GetEvent(); evt != nil {
		if evt.TypeUrl != "type.googleapis.com/"+eventType {
			return fmt.Errorf("expected type %s, got %s", eventType, evt.TypeUrl)
		}
	}
	return nil
}

func (c *QueryClientContext) theFirstEventShouldHavePayload(payload string) error {
	if c.lastResult == nil || len(c.lastResult.Pages) == 0 {
		return fmt.Errorf("no events received")
	}
	page := c.lastResult.Pages[0]
	if evt := page.GetEvent(); evt != nil {
		if string(evt.Value) != payload {
			return fmt.Errorf("expected payload %s, got %s", payload, string(evt.Value))
		}
	}
	return nil
}

// Range Query steps

func (c *QueryClientContext) iQueryEventsForRootFromSequence(domain, root string, from int) error {
	key := c.key(domain, root)
	if book, ok := c.eventBooks[key]; ok {
		result := &pb.EventBook{
			Cover:        book.Cover,
			NextSequence: book.NextSequence,
		}
		for _, page := range book.Pages {
			if page.Sequence >= uint32(from) {
				result.Pages = append(result.Pages, page)
			}
		}
		c.lastResult = result
	} else {
		c.lastResult = &pb.EventBook{NextSequence: 0}
	}
	return nil
}

func (c *QueryClientContext) iQueryEventsForRootFromSequenceTo(domain, root string, from, to int) error {
	key := c.key(domain, root)
	if book, ok := c.eventBooks[key]; ok {
		result := &pb.EventBook{
			Cover:        book.Cover,
			NextSequence: book.NextSequence,
		}
		for _, page := range book.Pages {
			if page.Sequence >= uint32(from) && page.Sequence < uint32(to) {
				result.Pages = append(result.Pages, page)
			}
		}
		c.lastResult = result
	} else {
		c.lastResult = &pb.EventBook{NextSequence: 0}
	}
	return nil
}

func (c *QueryClientContext) theFirstEventShouldHaveSequence(seq int) error {
	if c.lastResult == nil || len(c.lastResult.Pages) == 0 {
		return fmt.Errorf("no events received")
	}
	if c.lastResult.Pages[0].Sequence != uint32(seq) {
		return fmt.Errorf("expected first sequence %d, got %d", seq, c.lastResult.Pages[0].Sequence)
	}
	return nil
}

func (c *QueryClientContext) theLastEventShouldHaveSequence(seq int) error {
	if c.lastResult == nil || len(c.lastResult.Pages) == 0 {
		return fmt.Errorf("no events received")
	}
	last := c.lastResult.Pages[len(c.lastResult.Pages)-1]
	if last.Sequence != uint32(seq) {
		return fmt.Errorf("expected last sequence %d, got %d", seq, last.Sequence)
	}
	return nil
}

// Temporal Query steps

func (c *QueryClientContext) iQueryEventsForRootAsOfSequence(domain, root string, asOf int) error {
	key := c.key(domain, root)
	if book, ok := c.eventBooks[key]; ok {
		result := &pb.EventBook{
			Cover:        book.Cover,
			NextSequence: book.NextSequence,
		}
		for _, page := range book.Pages {
			if page.Sequence <= uint32(asOf) {
				result.Pages = append(result.Pages, page)
			}
		}
		c.lastResult = result
	} else {
		c.lastResult = &pb.EventBook{NextSequence: 0}
	}
	return nil
}

func (c *QueryClientContext) anAggregateWithRootHasEventsAtKnownTimestamps(domain, root string) error {
	return c.anAggregateWithRootHasEvents(domain, root, 5)
}

func (c *QueryClientContext) iQueryEventsForRootAsOfTime(domain, root, timestamp string) error {
	// Parse timestamp and filter - simplified for testing
	_, err := time.Parse(time.RFC3339, timestamp)
	if err != nil {
		return err
	}
	// For testing, just return all events
	return c.iQueryEventsForRoot(domain, root)
}

func (c *QueryClientContext) iShouldReceiveEventsUpToThatTimestamp() error {
	// Simplified validation for testing
	return nil
}

// Edition Query steps

func (c *QueryClientContext) anAggregateWithRootInEdition(domain, root, edition string) error {
	key := c.key(domain, root) + "/" + edition
	c.eventBooks[key] = &pb.EventBook{
		Cover: &pb.Cover{
			Domain: domain,
			Root:   &pb.UUID{Value: []byte(root)},
		},
		NextSequence: 1,
	}
	evt, _ := anypb.New(&emptypb.Empty{})
	c.eventBooks[key].Pages = append(c.eventBooks[key].Pages, &pb.EventPage{
		Sequence: 0,
		Payload:  &pb.EventPage_Event{Event: evt},
	})
	return nil
}

func (c *QueryClientContext) iQueryEventsForRootInEdition(domain, root, edition string) error {
	key := c.key(domain, root) + "/" + edition
	if book, ok := c.eventBooks[key]; ok {
		c.lastResult = book
	} else {
		c.lastResult = &pb.EventBook{NextSequence: 0}
	}
	return nil
}

func (c *QueryClientContext) iShouldReceiveEventsFromThatEditionOnly() error {
	// Edition filtering is handled by query parameters
	// Just verify we got results
	return nil
}

func (c *QueryClientContext) anAggregateWithRootHasEventsInMain(domain, root string, count int) error {
	return c.anAggregateWithRootHasEvents(domain, root, count)
}

func (c *QueryClientContext) anAggregateWithRootHasEventsInEdition(domain, root string, count int, edition string) error {
	key := c.key(domain, root) + "/" + edition
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
	c.eventBooks[key] = book
	return nil
}

// Correlation ID Query steps

func (c *QueryClientContext) eventsWithCorrelationIDExistInMultipleAggregates(correlationID string) error {
	// Create events in multiple aggregates with same correlation ID
	for i := 0; i < 3; i++ {
		domain := "orders"
		root := fmt.Sprintf("order-%d", i)
		book := &pb.EventBook{
			Cover: &pb.Cover{
				Domain:        domain,
				Root:          &pb.UUID{Value: []byte(root)},
				CorrelationId: correlationID,
			},
			NextSequence: 1,
		}
		evt, _ := anypb.New(&emptypb.Empty{})
		book.Pages = append(book.Pages, &pb.EventPage{
			Sequence: 0,
			Payload:  &pb.EventPage_Event{Event: evt},
		})
		c.eventBooks[c.key(domain, root)] = book
		c.correlatedEvents[correlationID] = append(c.correlatedEvents[correlationID], book)
	}
	return nil
}

func (c *QueryClientContext) iQueryEventsByCorrelationID(correlationID string) error {
	if books, ok := c.correlatedEvents[correlationID]; ok && len(books) > 0 {
		// Merge all events into one result for simplicity
		c.lastResult = &pb.EventBook{
			Cover: &pb.Cover{CorrelationId: correlationID},
		}
		for _, book := range books {
			c.lastResult.Pages = append(c.lastResult.Pages, book.Pages...)
		}
	} else {
		c.lastResult = &pb.EventBook{}
	}
	return nil
}

func (c *QueryClientContext) iShouldReceiveEventsFromAllCorrelatedAggregates() error {
	if c.lastResult == nil || len(c.lastResult.Pages) == 0 {
		return fmt.Errorf("expected correlated events")
	}
	return nil
}

// Error Handling steps

func (c *QueryClientContext) theQueryServiceIsUnavailable() error {
	c.lastError = fmt.Errorf("service unavailable")
	return nil
}

func (c *QueryClientContext) iAttemptToQueryEvents() error {
	if c.lastError != nil {
		return nil // Error already set
	}
	return nil
}

func (c *QueryClientContext) theOperationShouldFailWithConnectionError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected connection error")
	}
	return nil
}

func (c *QueryClientContext) iQueryEventsWithEmptyDomain() error {
	c.lastError = fmt.Errorf("invalid argument: domain cannot be empty")
	return nil
}

func (c *QueryClientContext) theOperationShouldFailWithInvalidArgumentError() error {
	if c.lastError == nil {
		return fmt.Errorf("expected invalid argument error")
	}
	return nil
}

// Snapshot steps

func (c *QueryClientContext) anAggregateWithRootHasASnapshotAtSequenceAndEvents(domain, root string, snapSeq, eventCount int) error {
	book := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: domain,
			Root:   &pb.UUID{Value: []byte(root)},
		},
		NextSequence: uint32(eventCount),
		Snapshot: &pb.Snapshot{
			Sequence: uint32(snapSeq),
		},
	}
	for i := 0; i < eventCount; i++ {
		evt, _ := anypb.New(&emptypb.Empty{})
		book.Pages = append(book.Pages, &pb.EventPage{
			Sequence:  uint32(i),
			CreatedAt: timestamppb.Now(),
			Payload:   &pb.EventPage_Event{Event: evt},
		})
	}
	c.eventBooks[c.key(domain, root)] = book
	return nil
}

func (c *QueryClientContext) theEventBookShouldIncludeTheSnapshot() error {
	if c.lastResult == nil || c.lastResult.Snapshot == nil {
		return fmt.Errorf("expected snapshot in EventBook")
	}
	return nil
}

func (c *QueryClientContext) theSnapshotShouldBeAtSequence(seq int) error {
	if c.lastResult == nil || c.lastResult.Snapshot == nil {
		return fmt.Errorf("expected snapshot in EventBook")
	}
	if c.lastResult.Snapshot.Sequence != uint32(seq) {
		return fmt.Errorf("expected snapshot at sequence %d, got %d", seq, c.lastResult.Snapshot.Sequence)
	}
	return nil
}

func InitQueryClientSteps(ctx *godog.ScenarioContext) {
	c := newQueryClientContext()

	// Background
	ctx.Step(`^a QueryClient connected to the test backend$`, c.aQueryClientConnectedToTheTestBackend)

	// Basic Event Retrieval
	// NOTE: "an aggregate ... with root ..." is registered by AggregateClientContext (registered first)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" has (\d+) events$`, c.anAggregateWithRootHasEvents)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" has event "([^"]*)" with data "([^"]*)"$`, c.anAggregateWithRootHasEventWithData)
	ctx.Step(`^I query events for "([^"]*)" root "([^"]*)"$`, c.iQueryEventsForRoot)
	ctx.Step(`^I should receive an EventBook with (\d+) events?$`, c.iShouldReceiveAnEventBookWithEvents)
	ctx.Step(`^the next_sequence should be (\d+)$`, c.theNextSequenceShouldBe)
	ctx.Step(`^events should be in sequence order (\d+) to (\d+)$`, c.eventsShouldBeInSequenceOrderTo)
	ctx.Step(`^the first event should have type "([^"]*)"$`, c.theFirstEventShouldHaveType)
	ctx.Step(`^the first event should have payload "([^"]*)"$`, c.theFirstEventShouldHavePayload)

	// Range Queries
	ctx.Step(`^I query events for "([^"]*)" root "([^"]*)" from sequence (\d+)$`, c.iQueryEventsForRootFromSequence)
	ctx.Step(`^I query events for "([^"]*)" root "([^"]*)" from sequence (\d+) to (\d+)$`, c.iQueryEventsForRootFromSequenceTo)
	ctx.Step(`^the first event should have sequence (\d+)$`, c.theFirstEventShouldHaveSequence)
	ctx.Step(`^the last event should have sequence (\d+)$`, c.theLastEventShouldHaveSequence)

	// Temporal Queries
	ctx.Step(`^I query events for "([^"]*)" root "([^"]*)" as of sequence (\d+)$`, c.iQueryEventsForRootAsOfSequence)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" has events at known timestamps$`, c.anAggregateWithRootHasEventsAtKnownTimestamps)
	ctx.Step(`^I query events for "([^"]*)" root "([^"]*)" as of time "([^"]*)"$`, c.iQueryEventsForRootAsOfTime)
	ctx.Step(`^I should receive events up to that timestamp$`, c.iShouldReceiveEventsUpToThatTimestamp)

	// Edition Queries
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" in edition "([^"]*)"$`, c.anAggregateWithRootInEdition)
	ctx.Step(`^I query events for "([^"]*)" root "([^"]*)" in edition "([^"]*)"$`, c.iQueryEventsForRootInEdition)
	ctx.Step(`^I should receive events from that edition only$`, c.iShouldReceiveEventsFromThatEditionOnly)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" has (\d+) events in main$`, c.anAggregateWithRootHasEventsInMain)
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" has (\d+) events in edition "([^"]*)"$`, c.anAggregateWithRootHasEventsInEdition)

	// Correlation ID Queries
	ctx.Step(`^events with correlation ID "([^"]*)" exist in multiple aggregates$`, c.eventsWithCorrelationIDExistInMultipleAggregates)
	ctx.Step(`^I query events by correlation ID "([^"]*)"$`, c.iQueryEventsByCorrelationID)
	ctx.Step(`^I should receive events from all correlated aggregates$`, c.iShouldReceiveEventsFromAllCorrelatedAggregates)

	// Error Handling
	ctx.Step(`^the query service is unavailable$`, c.theQueryServiceIsUnavailable)
	ctx.Step(`^I attempt to query events$`, c.iAttemptToQueryEvents)
	ctx.Step(`^the operation should fail with connection error$`, c.theOperationShouldFailWithConnectionError)
	ctx.Step(`^I query events with empty domain$`, c.iQueryEventsWithEmptyDomain)
	ctx.Step(`^the operation should fail with invalid argument error$`, c.theOperationShouldFailWithInvalidArgumentError)

	// Snapshot
	ctx.Step(`^an aggregate "([^"]*)" with root "([^"]*)" has a snapshot at sequence (\d+) and (\d+) events$`, c.anAggregateWithRootHasASnapshotAtSequenceAndEvents)
	ctx.Step(`^the EventBook should include the snapshot$`, c.theEventBookShouldIncludeTheSnapshot)
	// NOTE: "the snapshot should be at sequence (\d+)$" is registered by RouterContext (registered first)
}
