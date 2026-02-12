package angzarr

import (
	"context"
	"os"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// formatEndpoint converts an endpoint to gRPC target format.
// Supports both TCP (host:port) and Unix Domain Sockets (file paths).
// UDS paths are detected by leading '/' or './' and converted to unix:// URIs.
func formatEndpoint(endpoint string) string {
	if strings.HasPrefix(endpoint, "/") || strings.HasPrefix(endpoint, "./") {
		return "unix://" + endpoint
	}
	if strings.HasPrefix(endpoint, "unix://") {
		return endpoint
	}
	return endpoint
}

// QueryClient wraps the EventQueryService for event retrieval.
type QueryClient struct {
	inner pb.EventQueryServiceClient
	conn  *grpc.ClientConn
}

// NewQueryClient connects to an event query service at the given endpoint.
func NewQueryClient(endpoint string) (*QueryClient, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &QueryClient{
		inner: pb.NewEventQueryServiceClient(conn),
		conn:  conn,
	}, nil
}

// QueryClientFromEnv connects using an environment variable with fallback.
func QueryClientFromEnv(envVar, defaultEndpoint string) (*QueryClient, error) {
	endpoint := os.Getenv(envVar)
	if endpoint == "" {
		endpoint = defaultEndpoint
	}
	return NewQueryClient(endpoint)
}

// QueryClientFromConn creates a client from an existing connection.
func QueryClientFromConn(conn *grpc.ClientConn) *QueryClient {
	return &QueryClient{
		inner: pb.NewEventQueryServiceClient(conn),
		conn:  conn,
	}
}

// GetEventBook retrieves a single EventBook for the query.
func (c *QueryClient) GetEventBook(ctx context.Context, query *pb.Query) (*pb.EventBook, error) {
	resp, err := c.inner.GetEventBook(ctx, query)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// GetEvents retrieves all EventBooks matching the query.
func (c *QueryClient) GetEvents(ctx context.Context, query *pb.Query) ([]*pb.EventBook, error) {
	stream, err := c.inner.GetEvents(ctx, query)
	if err != nil {
		return nil, GRPCError(err)
	}

	var events []*pb.EventBook
	for {
		event, err := stream.Recv()
		if err != nil {
			break
		}
		events = append(events, event)
	}
	return events, nil
}

// Close closes the underlying connection.
func (c *QueryClient) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// AggregateClient wraps the AggregateCoordinatorService for command execution.
type AggregateClient struct {
	inner pb.AggregateCoordinatorServiceClient
	conn  *grpc.ClientConn
}

// NewAggregateClient connects to an aggregate coordinator at the given endpoint.
func NewAggregateClient(endpoint string) (*AggregateClient, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &AggregateClient{
		inner: pb.NewAggregateCoordinatorServiceClient(conn),
		conn:  conn,
	}, nil
}

// AggregateClientFromEnv connects using an environment variable with fallback.
func AggregateClientFromEnv(envVar, defaultEndpoint string) (*AggregateClient, error) {
	endpoint := os.Getenv(envVar)
	if endpoint == "" {
		endpoint = defaultEndpoint
	}
	return NewAggregateClient(endpoint)
}

// AggregateClientFromConn creates a client from an existing connection.
func AggregateClientFromConn(conn *grpc.ClientConn) *AggregateClient {
	return &AggregateClient{
		inner: pb.NewAggregateCoordinatorServiceClient(conn),
		conn:  conn,
	}
}

// Handle executes a command asynchronously.
func (c *AggregateClient) Handle(ctx context.Context, cmd *pb.CommandBook) (*pb.CommandResponse, error) {
	resp, err := c.inner.Handle(ctx, cmd)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// HandleSync executes a command synchronously with the specified sync mode.
func (c *AggregateClient) HandleSync(ctx context.Context, cmd *pb.SyncCommandBook) (*pb.CommandResponse, error) {
	resp, err := c.inner.HandleSync(ctx, cmd)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// DryRunHandle executes a command in dry-run mode (no persistence).
func (c *AggregateClient) DryRunHandle(ctx context.Context, req *pb.DryRunRequest) (*pb.CommandResponse, error) {
	resp, err := c.inner.DryRunHandle(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// Close closes the underlying connection.
func (c *AggregateClient) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// SpeculativeClient wraps the SpeculativeService for what-if scenarios.
type SpeculativeClient struct {
	inner pb.SpeculativeServiceClient
	conn  *grpc.ClientConn
}

// NewSpeculativeClient connects to a speculative service at the given endpoint.
func NewSpeculativeClient(endpoint string) (*SpeculativeClient, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &SpeculativeClient{
		inner: pb.NewSpeculativeServiceClient(conn),
		conn:  conn,
	}, nil
}

// SpeculativeClientFromEnv connects using an environment variable with fallback.
func SpeculativeClientFromEnv(envVar, defaultEndpoint string) (*SpeculativeClient, error) {
	endpoint := os.Getenv(envVar)
	if endpoint == "" {
		endpoint = defaultEndpoint
	}
	return NewSpeculativeClient(endpoint)
}

// SpeculativeClientFromConn creates a client from an existing connection.
func SpeculativeClientFromConn(conn *grpc.ClientConn) *SpeculativeClient {
	return &SpeculativeClient{
		inner: pb.NewSpeculativeServiceClient(conn),
		conn:  conn,
	}
}

// DryRun executes a command without persistence.
func (c *SpeculativeClient) DryRun(ctx context.Context, req *pb.DryRunRequest) (*pb.CommandResponse, error) {
	resp, err := c.inner.DryRunCommand(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// Projector speculatively executes a projector against events.
func (c *SpeculativeClient) Projector(ctx context.Context, req *pb.SpeculateProjectorRequest) (*pb.Projection, error) {
	resp, err := c.inner.SpeculateProjector(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// Saga speculatively executes a saga against events.
func (c *SpeculativeClient) Saga(ctx context.Context, req *pb.SpeculateSagaRequest) (*pb.SagaResponse, error) {
	resp, err := c.inner.SpeculateSaga(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// ProcessManager speculatively executes a process manager.
func (c *SpeculativeClient) ProcessManager(ctx context.Context, req *pb.SpeculatePmRequest) (*pb.ProcessManagerHandleResponse, error) {
	resp, err := c.inner.SpeculateProcessManager(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// Close closes the underlying connection.
func (c *SpeculativeClient) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// DomainClient combines aggregate and query clients for a single domain.
type DomainClient struct {
	Aggregate *AggregateClient
	Query     *QueryClient
	conn      *grpc.ClientConn
}

// NewDomainClient connects to a domain's coordinator at the given endpoint.
func NewDomainClient(endpoint string) (*DomainClient, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &DomainClient{
		Aggregate: AggregateClientFromConn(conn),
		Query:     QueryClientFromConn(conn),
		conn:      conn,
	}, nil
}

// DomainClientFromEnv connects using an environment variable with fallback.
func DomainClientFromEnv(envVar, defaultEndpoint string) (*DomainClient, error) {
	endpoint := os.Getenv(envVar)
	if endpoint == "" {
		endpoint = defaultEndpoint
	}
	return NewDomainClient(endpoint)
}

// DomainClientFromConn creates a client from an existing connection.
func DomainClientFromConn(conn *grpc.ClientConn) *DomainClient {
	return &DomainClient{
		Aggregate: AggregateClientFromConn(conn),
		Query:     QueryClientFromConn(conn),
		conn:      conn,
	}
}

// Execute is a convenience method that delegates to Aggregate.Handle.
func (c *DomainClient) Execute(ctx context.Context, cmd *pb.CommandBook) (*pb.CommandResponse, error) {
	return c.Aggregate.Handle(ctx, cmd)
}

// Close closes the underlying connection.
func (c *DomainClient) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// Client combines aggregate, query, and speculative clients.
type Client struct {
	Aggregate   *AggregateClient
	Query       *QueryClient
	Speculative *SpeculativeClient
	conn        *grpc.ClientConn
}

// NewClient connects to a server providing all services.
func NewClient(endpoint string) (*Client, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &Client{
		Aggregate:   AggregateClientFromConn(conn),
		Query:       QueryClientFromConn(conn),
		Speculative: SpeculativeClientFromConn(conn),
		conn:        conn,
	}, nil
}

// ClientFromEnv connects using an environment variable with fallback.
func ClientFromEnv(envVar, defaultEndpoint string) (*Client, error) {
	endpoint := os.Getenv(envVar)
	if endpoint == "" {
		endpoint = defaultEndpoint
	}
	return NewClient(endpoint)
}

// ClientFromConn creates a client from an existing connection.
func ClientFromConn(conn *grpc.ClientConn) *Client {
	return &Client{
		Aggregate:   AggregateClientFromConn(conn),
		Query:       QueryClientFromConn(conn),
		Speculative: SpeculativeClientFromConn(conn),
		conn:        conn,
	}
}

// Close closes the underlying connection.
func (c *Client) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}
