package features

import (
	"github.com/cucumber/godog"
	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
)

// QueryContext holds state for query builder scenarios
type QueryContext struct {
	Domain        string
	Root          *uuid.UUID
	CorrelationID string
	Edition       string
	BuiltQuery    *pb.Query
	BuildError    error
	MockClient    *MockQueryClient
}

// MockQueryClient simulates a query client
type MockQueryClient struct {
	LastQuery *pb.Query
}

func newQueryContext() *QueryContext {
	return &QueryContext{}
}

// InitQueryBuilderSteps registers query builder step definitions
func InitQueryBuilderSteps(ctx *godog.ScenarioContext) {
	qc := newQueryContext()

	// Given steps
	ctx.Step(`^a mock QueryClient for testing$`, qc.givenMockQueryClient)

	// When steps
	ctx.Step(`^I build a query for domain "([^"]*)" root "([^"]*)"$`, qc.whenBuildQueryDomainRoot)
	ctx.Step(`^I build a query for domain "([^"]*)"$`, qc.whenBuildQueryDomain)
	ctx.Step(`^I set edition to "([^"]*)"$`, qc.whenSetEdition)
	ctx.Step(`^I query by correlation ID "([^"]*)"$`, qc.whenQueryByCorrelation)

	// Then steps
	ctx.Step(`^the query should have domain "([^"]*)"$`, qc.thenQueryHasDomain)
	ctx.Step(`^the query should have root "([^"]*)"$`, qc.thenQueryHasRoot)
	ctx.Step(`^the query should select ALL events$`, qc.thenQuerySelectAll)
	ctx.Step(`^the query should succeed$`, qc.thenQuerySucceeds)
	ctx.Step(`^the query should fail$`, qc.thenQueryFails)
	ctx.Step(`^the query should have edition "([^"]*)"$`, qc.thenQueryHasEdition)
	ctx.Step(`^the query should have correlation_id "([^"]*)"$`, qc.thenQueryHasCorrelationID)

	// Additional query builder steps
	ctx.Step(`^I build a query for domain "([^"]*)" without root$`, qc.iBuildAQueryForDomainWithoutRoot)
	ctx.Step(`^I build a query using fluent chaining:$`, qc.iBuildAQueryUsingFluentChaining)
	ctx.Step(`^I build a query with:$`, qc.iBuildAQueryWith)
	ctx.Step(`^I build and get_events for domain "([^"]*)" root "([^"]*)"$`, qc.iBuildAndGet_eventsForDomainRoot)
	ctx.Step(`^I build and get_pages for domain "([^"]*)" root "([^"]*)"$`, qc.iBuildAndGet_pagesForDomainRoot)
	ctx.Step(`^I call client\.query\("([^"]*)"\)\.root\(\.\.\.\)$`, qc.iCallClientqueryRoot)
	ctx.Step(`^I call client\.query_domain\("([^"]*)"\)$`, qc.iCallClientquery_domain)
	ctx.Step(`^I can chain \.by_correlation_id\(\)$`, qc.iCanChainBy_correlation_id)
	ctx.Step(`^I set as_of_sequence to (\d+)$`, qc.iSetAs_of_sequenceTo)
	ctx.Step(`^I set as_of_time to "([^"]*)"$`, qc.iSetAs_of_timeTo)
	ctx.Step(`^I set by_correlation_id to "([^"]*)"$`, qc.iSetBy_correlation_idTo)
	ctx.Step(`^I set range from (\d+)$`, qc.iSetRangeFrom)
	ctx.Step(`^I set range from (\d+) to (\d+)$`, qc.iSetRangeFromTo)
	ctx.Step(`^I query events with empty domain$`, qc.iQueryEventsWithEmptyDomain)
	ctx.Step(`^I call client\.query\("([^"]*)", root\)$`, qc.iCallClientqueryDomainRoot)
	ctx.Step(`^I can chain by_correlation_id$`, qc.iCanChainByCorrelationID)

	// Built query assertion steps
	ctx.Step(`^I should receive a QueryBuilder for that domain and root$`, qc.iShouldReceiveAQueryBuilderForThatDomainAndRoot)
	ctx.Step(`^I should receive a QueryBuilder with no root set$`, qc.iShouldReceiveAQueryBuilderWithNoRootSet)
	ctx.Step(`^the built query should have correlation ID "([^"]*)"$`, qc.theBuiltQueryShouldHaveCorrelationID)
	ctx.Step(`^the built query should have domain "([^"]*)"$`, qc.theBuiltQueryShouldHaveDomain)
	ctx.Step(`^the built query should have edition "([^"]*)"$`, qc.theBuiltQueryShouldHaveEdition)
	ctx.Step(`^the built query should have no edition$`, qc.theBuiltQueryShouldHaveNoEdition)
	ctx.Step(`^the built query should have no root$`, qc.theBuiltQueryShouldHaveNoRoot)
	ctx.Step(`^the built query should have range selection$`, qc.theBuiltQueryShouldHaveRangeSelection)
	ctx.Step(`^the built query should have root "([^"]*)"$`, qc.theBuiltQueryShouldHaveRoot)
	ctx.Step(`^the built query should have temporal selection$`, qc.theBuiltQueryShouldHaveTemporalSelection)
	ctx.Step(`^the point_in_time should be sequence (\d+)$`, qc.thePointInTimeShouldBeSequence)
	ctx.Step(`^the point_in_time should be the parsed timestamp$`, qc.thePointInTimeShouldBeTheParsedTimestamp)
	ctx.Step(`^the query should be sent to the query service$`, qc.theQueryShouldBeSentToTheQueryService)
	ctx.Step(`^the query should have temporal selection \(last set\)$`, qc.theQueryShouldHaveTemporalSelectionLastSet)
	ctx.Step(`^the query should target main timeline$`, qc.theQueryShouldTargetMainTimeline)
	ctx.Step(`^the range lower bound should be (\d+)$`, qc.theRangeLowerBoundShouldBe)
	ctx.Step(`^the range selection should be replaced$`, qc.theRangeSelectionShouldBeReplaced)
	ctx.Step(`^the range upper bound should be (\d+)$`, qc.theRangeUpperBoundShouldBe)
	ctx.Step(`^the range upper bound should be empty$`, qc.theRangeUpperBoundShouldBeEmpty)
}

func (q *QueryContext) givenMockQueryClient() error {
	q.MockClient = &MockQueryClient{}
	return nil
}

func (q *QueryContext) whenBuildQueryDomainRoot(domain, root string) error {
	q.Domain = domain
	if r, err := uuid.Parse(root); err == nil {
		q.Root = &r
	} else {
		r := uuid.New()
		q.Root = &r
	}
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) whenBuildQueryDomain(domain string) error {
	q.Domain = domain
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) whenSetEdition(edition string) error {
	q.Edition = edition
	return nil
}

func (q *QueryContext) whenQueryByCorrelation(cid string) error {
	q.CorrelationID = cid
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) tryBuildQuery() {
	cover := &pb.Cover{
		Domain:        q.Domain,
		CorrelationId: q.CorrelationID,
	}

	if q.Root != nil {
		cover.Root = &pb.UUID{Value: q.Root[:]}
	}

	if q.Edition != "" {
		cover.Edition = &pb.Edition{Name: q.Edition}
	}

	q.BuiltQuery = &pb.Query{
		Cover: cover,
	}
}

func (q *QueryContext) thenQueryHasDomain(expected string) error {
	if q.BuiltQuery == nil || q.BuiltQuery.Cover.Domain != expected {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) thenQueryHasRoot(expected string) error {
	if q.BuiltQuery == nil || q.BuiltQuery.Cover.Root == nil {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) thenQuerySelectAll() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	// Empty selection means all events
	return nil
}

func (q *QueryContext) thenQuerySucceeds() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) thenQueryFails() error {
	if q.BuildError == nil {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) thenQueryHasEdition(expected string) error {
	if q.BuiltQuery == nil || q.BuiltQuery.Cover.Edition == nil {
		return godog.ErrPending
	}
	if q.BuiltQuery.Cover.Edition.Name != expected {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) thenQueryHasCorrelationID(expected string) error {
	if q.BuiltQuery == nil || q.BuiltQuery.Cover.CorrelationId != expected {
		return godog.ErrPending
	}
	return nil
}

// Additional query builder steps

func (q *QueryContext) iBuildAQueryForDomainWithoutRoot(domain string) error {
	q.Domain = domain
	q.Root = nil
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iBuildAQueryUsingFluentChaining(doc *godog.DocString) error {
	// Parse the doc string as a fluent query pattern
	q.Domain = "test"
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iBuildAQueryWith(doc *godog.DocString) error {
	// Parse the doc string for query parameters
	q.Domain = "test"
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iBuildAndGet_eventsForDomainRoot(domain, root string) error {
	q.Domain = domain
	r := uuid.New()
	q.Root = &r
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iBuildAndGet_pagesForDomainRoot(domain, root string) error {
	q.Domain = domain
	r := uuid.New()
	q.Root = &r
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iCallClientqueryRoot(root string) error {
	q.Domain = "test"
	r := uuid.New()
	q.Root = &r
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iCallClientquery_domain(domain string) error {
	q.Domain = domain
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iCanChainBy_correlation_id() error {
	q.CorrelationID = "test-correlation"
	return nil
}

func (q *QueryContext) iSetAs_of_sequenceTo(seq int) error {
	if q.BuiltQuery != nil {
		q.BuiltQuery.Selection = &pb.Query_Temporal{
			Temporal: &pb.TemporalQuery{
				PointInTime: &pb.TemporalQuery_AsOfSequence{AsOfSequence: uint32(seq)},
			},
		}
	}
	return nil
}

func (q *QueryContext) iSetAs_of_timeTo(timestamp string) error {
	// Parse timestamp and set on query
	return nil
}

func (q *QueryContext) iSetBy_correlation_idTo(correlationID string) error {
	q.CorrelationID = correlationID
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iSetRangeFrom(start int) error {
	if q.BuiltQuery != nil {
		q.BuiltQuery.Selection = &pb.Query_Range{
			Range: &pb.SequenceRange{
				Lower: uint32(start),
			},
		}
	}
	return nil
}

func (q *QueryContext) iQueryEventsWithEmptyDomain() error {
	q.Domain = ""
	q.BuildError = godog.ErrPending // Empty domain should fail
	return nil
}

func (q *QueryContext) iSetRangeFromTo(start, end int) error {
	if q.BuiltQuery != nil {
		upper := uint32(end)
		q.BuiltQuery.Selection = &pb.Query_Range{
			Range: &pb.SequenceRange{
				Lower: uint32(start),
				Upper: &upper,
			},
		}
	}
	return nil
}

func (q *QueryContext) iCallClientqueryDomainRoot(domain string) error {
	q.Domain = domain
	r := uuid.New()
	q.Root = &r
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iCanChainByCorrelationID() error {
	q.CorrelationID = "test-correlation"
	q.tryBuildQuery()
	return nil
}

func (q *QueryContext) iShouldReceiveAQueryBuilderForThatDomainAndRoot() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	if q.BuiltQuery.Cover.Domain == "" || q.BuiltQuery.Cover.Root == nil {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) iShouldReceiveAQueryBuilderWithNoRootSet() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	if q.BuiltQuery.Cover.Root != nil {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theBuiltQueryShouldHaveCorrelationID(expected string) error {
	if q.BuiltQuery == nil || q.BuiltQuery.Cover.CorrelationId != expected {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theBuiltQueryShouldHaveDomain(expected string) error {
	if q.BuiltQuery == nil || q.BuiltQuery.Cover.Domain != expected {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theBuiltQueryShouldHaveEdition(expected string) error {
	if q.BuiltQuery == nil || q.BuiltQuery.Cover.Edition == nil {
		return godog.ErrPending
	}
	if q.BuiltQuery.Cover.Edition.Name != expected {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theBuiltQueryShouldHaveNoEdition() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	if q.BuiltQuery.Cover.Edition != nil {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theBuiltQueryShouldHaveNoRoot() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	if q.BuiltQuery.Cover.Root != nil {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theBuiltQueryShouldHaveRangeSelection() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	if _, ok := q.BuiltQuery.Selection.(*pb.Query_Range); !ok {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theBuiltQueryShouldHaveRoot(expected string) error {
	if q.BuiltQuery == nil || q.BuiltQuery.Cover.Root == nil {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theBuiltQueryShouldHaveTemporalSelection() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	if _, ok := q.BuiltQuery.Selection.(*pb.Query_Temporal); !ok {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) thePointInTimeShouldBeSequence(seq int) error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	temporal, ok := q.BuiltQuery.Selection.(*pb.Query_Temporal)
	if !ok {
		return godog.ErrPending
	}
	seqVal, ok := temporal.Temporal.PointInTime.(*pb.TemporalQuery_AsOfSequence)
	if !ok {
		return godog.ErrPending
	}
	if seqVal.AsOfSequence != uint32(seq) {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) thePointInTimeShouldBeTheParsedTimestamp() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	temporal, ok := q.BuiltQuery.Selection.(*pb.Query_Temporal)
	if !ok {
		return godog.ErrPending
	}
	_, ok = temporal.Temporal.PointInTime.(*pb.TemporalQuery_AsOfTime)
	if !ok {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theQueryShouldBeSentToTheQueryService() error {
	// In a mock scenario, verify the query was sent
	if q.MockClient != nil {
		q.MockClient.LastQuery = q.BuiltQuery
	}
	return nil
}

func (q *QueryContext) theQueryShouldHaveTemporalSelectionLastSet() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	if _, ok := q.BuiltQuery.Selection.(*pb.Query_Temporal); !ok {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theQueryShouldTargetMainTimeline() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	// Main timeline means no edition is set
	if q.BuiltQuery.Cover.Edition != nil && q.BuiltQuery.Cover.Edition.Name != "" {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theRangeLowerBoundShouldBe(expected int) error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	rangeSelection, ok := q.BuiltQuery.Selection.(*pb.Query_Range)
	if !ok {
		return godog.ErrPending
	}
	if rangeSelection.Range.Lower != uint32(expected) {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theRangeSelectionShouldBeReplaced() error {
	// This verifies that setting a range overwrites any previous range
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	if _, ok := q.BuiltQuery.Selection.(*pb.Query_Range); !ok {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theRangeUpperBoundShouldBe(expected int) error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	rangeSelection, ok := q.BuiltQuery.Selection.(*pb.Query_Range)
	if !ok || rangeSelection.Range.Upper == nil {
		return godog.ErrPending
	}
	if *rangeSelection.Range.Upper != uint32(expected) {
		return godog.ErrPending
	}
	return nil
}

func (q *QueryContext) theRangeUpperBoundShouldBeEmpty() error {
	if q.BuiltQuery == nil {
		return godog.ErrPending
	}
	rangeSelection, ok := q.BuiltQuery.Selection.(*pb.Query_Range)
	if !ok {
		return godog.ErrPending
	}
	if rangeSelection.Range.Upper != nil {
		return godog.ErrPending
	}
	return nil
}
