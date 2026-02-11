package angzarr

import (
	"context"

	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// CommandBuilder provides fluent construction and execution of commands.
type CommandBuilder struct {
	client        *AggregateClient
	domain        string
	root          *uuid.UUID
	correlationID string
	sequence      uint32
	typeURL       string
	payload       []byte
	err           error
}

// NewCommandBuilder creates a command builder for an existing aggregate.
func NewCommandBuilder(client *AggregateClient, domain string, root uuid.UUID) *CommandBuilder {
	return &CommandBuilder{
		client: client,
		domain: domain,
		root:   &root,
	}
}

// NewCommandBuilderNew creates a command builder for a new aggregate (no root yet).
func NewCommandBuilderNew(client *AggregateClient, domain string) *CommandBuilder {
	return &CommandBuilder{
		client: client,
		domain: domain,
	}
}

// WithCorrelationID sets the correlation ID for request tracing.
func (b *CommandBuilder) WithCorrelationID(id string) *CommandBuilder {
	b.correlationID = id
	return b
}

// WithSequence sets the expected sequence number for optimistic locking.
func (b *CommandBuilder) WithSequence(seq uint32) *CommandBuilder {
	b.sequence = seq
	return b
}

// WithCommand sets the command type URL and message.
func (b *CommandBuilder) WithCommand(typeURL string, msg proto.Message) *CommandBuilder {
	payload, err := proto.Marshal(msg)
	if err != nil {
		b.err = InvalidArgumentError("failed to marshal command: " + err.Error())
		return b
	}
	b.typeURL = typeURL
	b.payload = payload
	return b
}

// Build constructs the CommandBook without executing.
func (b *CommandBuilder) Build() (*pb.CommandBook, error) {
	if b.err != nil {
		return nil, b.err
	}
	if b.typeURL == "" {
		return nil, InvalidArgumentError("command type_url not set")
	}
	if b.payload == nil {
		return nil, InvalidArgumentError("command payload not set")
	}

	correlationID := b.correlationID
	if correlationID == "" {
		correlationID = uuid.New().String()
	}

	cover := &pb.Cover{
		Domain:        b.domain,
		CorrelationId: correlationID,
	}
	if b.root != nil {
		cover.Root = UUIDToProto(*b.root)
	}

	return &pb.CommandBook{
		Cover: cover,
		Pages: []*pb.CommandPage{{
			Sequence: b.sequence,
			Command:  &anypb.Any{TypeUrl: b.typeURL, Value: b.payload},
		}},
	}, nil
}

// Execute builds and executes the command.
func (b *CommandBuilder) Execute(ctx context.Context) (*pb.CommandResponse, error) {
	cmd, err := b.Build()
	if err != nil {
		return nil, err
	}
	return b.client.Handle(ctx, cmd)
}

// QueryBuilder provides fluent construction and execution of queries.
type QueryBuilder struct {
	client        *QueryClient
	domain        string
	root          *uuid.UUID
	correlationID string
	rangeSelect   *pb.SequenceRange
	temporal      *pb.TemporalQuery
	edition       string
	err           error
}

// NewQueryBuilder creates a query builder for a specific aggregate.
func NewQueryBuilder(client *QueryClient, domain string, root uuid.UUID) *QueryBuilder {
	return &QueryBuilder{
		client: client,
		domain: domain,
		root:   &root,
	}
}

// NewQueryBuilderDomain creates a query builder by domain only (use with ByCorrelationID).
func NewQueryBuilderDomain(client *QueryClient, domain string) *QueryBuilder {
	return &QueryBuilder{
		client: client,
		domain: domain,
	}
}

// ByCorrelationID queries by correlation ID instead of root.
func (b *QueryBuilder) ByCorrelationID(id string) *QueryBuilder {
	b.correlationID = id
	b.root = nil
	return b
}

// WithEdition queries events from a specific edition.
func (b *QueryBuilder) WithEdition(edition string) *QueryBuilder {
	b.edition = edition
	return b
}

// Range queries a range of sequences from lower (inclusive).
func (b *QueryBuilder) Range(lower uint32) *QueryBuilder {
	b.rangeSelect = &pb.SequenceRange{Lower: lower}
	return b
}

// RangeTo queries a range of sequences with upper bound (inclusive).
func (b *QueryBuilder) RangeTo(lower, upper uint32) *QueryBuilder {
	b.rangeSelect = &pb.SequenceRange{Lower: lower, Upper: &upper}
	return b
}

// AsOfSequence queries state as of a specific sequence number.
func (b *QueryBuilder) AsOfSequence(seq uint32) *QueryBuilder {
	b.temporal = &pb.TemporalQuery{
		PointInTime: &pb.TemporalQuery_AsOfSequence{AsOfSequence: seq},
	}
	return b
}

// AsOfTime queries state as of a specific timestamp (RFC3339 format).
func (b *QueryBuilder) AsOfTime(rfc3339 string) *QueryBuilder {
	ts, err := ParseTimestamp(rfc3339)
	if err != nil {
		b.err = err
		return b
	}
	b.temporal = &pb.TemporalQuery{
		PointInTime: &pb.TemporalQuery_AsOfTime{AsOfTime: ts},
	}
	return b
}

// Build constructs the Query without executing.
func (b *QueryBuilder) Build() (*pb.Query, error) {
	if b.err != nil {
		return nil, b.err
	}

	cover := &pb.Cover{
		Domain:        b.domain,
		CorrelationId: b.correlationID,
	}
	if b.root != nil {
		cover.Root = UUIDToProto(*b.root)
	}
	if b.edition != "" {
		cover.Edition = ImplicitEdition(b.edition)
	}

	q := &pb.Query{Cover: cover}
	if b.rangeSelect != nil {
		q.Selection = &pb.Query_Range{Range: b.rangeSelect}
	} else if b.temporal != nil {
		q.Selection = &pb.Query_Temporal{Temporal: b.temporal}
	}
	return q, nil
}

// GetEventBook executes the query and returns a single EventBook.
func (b *QueryBuilder) GetEventBook(ctx context.Context) (*pb.EventBook, error) {
	query, err := b.Build()
	if err != nil {
		return nil, err
	}
	return b.client.GetEventBook(ctx, query)
}

// GetEvents executes the query and returns all matching EventBooks.
func (b *QueryBuilder) GetEvents(ctx context.Context) ([]*pb.EventBook, error) {
	query, err := b.Build()
	if err != nil {
		return nil, err
	}
	return b.client.GetEvents(ctx, query)
}

// GetPages executes the query and returns just the event pages.
func (b *QueryBuilder) GetPages(ctx context.Context) ([]*pb.EventPage, error) {
	book, err := b.GetEventBook(ctx)
	if err != nil {
		return nil, err
	}
	return book.Pages, nil
}

// Convenience methods on clients for builder creation

// Command starts building a command for the given domain and root.
func (c *AggregateClient) Command(domain string, root uuid.UUID) *CommandBuilder {
	return NewCommandBuilder(c, domain, root)
}

// CommandNew starts building a command for a new aggregate.
func (c *AggregateClient) CommandNew(domain string) *CommandBuilder {
	return NewCommandBuilderNew(c, domain)
}

// Query starts building a query for the given domain and root.
func (c *QueryClient) Query(domain string, root uuid.UUID) *QueryBuilder {
	return NewQueryBuilder(c, domain, root)
}

// QueryDomain starts building a query by domain only.
func (c *QueryClient) QueryDomain(domain string) *QueryBuilder {
	return NewQueryBuilderDomain(c, domain)
}

// DomainClient convenience methods

// Command starts building a command via the domain client's aggregate.
func (c *DomainClient) Command(domain string, root uuid.UUID) *CommandBuilder {
	return c.Aggregate.Command(domain, root)
}

// CommandNew starts building a command for a new aggregate.
func (c *DomainClient) CommandNew(domain string) *CommandBuilder {
	return c.Aggregate.CommandNew(domain)
}

// NewQuery starts building a query via the domain client's query client.
func (c *DomainClient) NewQuery(domain string, root uuid.UUID) *QueryBuilder {
	return c.Query.Query(domain, root)
}

// NewQueryDomain starts building a query by domain only.
func (c *DomainClient) NewQueryDomain(domain string) *QueryBuilder {
	return c.Query.QueryDomain(domain)
}
