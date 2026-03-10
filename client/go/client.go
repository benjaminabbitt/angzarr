package angzarr

import (
	"context"
	"fmt"
	"os"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// TransportMode specifies how to connect to angzarr services.
type TransportMode string

const (
	// TransportStandalone uses Unix Domain Sockets for local process communication.
	TransportStandalone TransportMode = "standalone"
	// TransportDistributed uses TCP via Kubernetes DNS for cluster communication.
	TransportDistributed TransportMode = "distributed"
)

// Default configuration values.
const (
	DefaultUDSBase       = "/tmp/angzarr"
	DefaultNamespace     = "angzarr"
	DefaultCHPort        = 1310
	DefaultTransportMode = TransportDistributed
)

// Environment variable names.
const (
	EnvMode      = "ANGZARR_MODE"
	EnvUDSBase   = "ANGZARR_UDS_BASE"
	EnvNamespace = "ANGZARR_NAMESPACE"
	EnvCHPort    = "ANGZARR_CH_PORT"
)

// ResolveCHEndpoint resolves a domain name to a command handler endpoint.
//
// In standalone mode, returns a Unix Domain Socket path.
// In distributed mode, returns a Kubernetes DNS name with port.
//
// Both modes use the ch-{domain} naming convention for consistency.
//
// The mode is detected from ANGZARR_MODE env var if not specified.
// Other env vars: ANGZARR_UDS_BASE, ANGZARR_NAMESPACE, ANGZARR_CH_PORT.
func ResolveCHEndpoint(domain string, mode TransportMode) string {
	if mode == "" {
		modeStr := os.Getenv(EnvMode)
		if modeStr == "" {
			mode = DefaultTransportMode
		} else {
			mode = TransportMode(modeStr)
		}
	}

	if mode == TransportStandalone {
		base := os.Getenv(EnvUDSBase)
		if base == "" {
			base = DefaultUDSBase
		}
		return fmt.Sprintf("%s/ch-%s.sock", base, domain)
	}

	// Distributed mode - K8s DNS
	namespace := os.Getenv(EnvNamespace)
	if namespace == "" {
		namespace = DefaultNamespace
	}
	portStr := os.Getenv(EnvCHPort)
	port := DefaultCHPort
	if portStr != "" {
		if p, err := fmt.Sscanf(portStr, "%d", &port); p != 1 || err != nil {
			port = DefaultCHPort
		}
	}
	return fmt.Sprintf("ch-%s.%s.svc:%d", domain, namespace, port)
}

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

// CommandHandlerClient wraps the CommandHandlerCoordinatorService for command execution.
type CommandHandlerClient struct {
	inner pb.CommandHandlerCoordinatorServiceClient
	conn  *grpc.ClientConn
}

// NewCommandHandlerClient connects to a command handler at the given endpoint.
func NewCommandHandlerClient(endpoint string) (*CommandHandlerClient, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &CommandHandlerClient{
		inner: pb.NewCommandHandlerCoordinatorServiceClient(conn),
		conn:  conn,
	}, nil
}

// CommandHandlerClientFromEnv connects using an environment variable with fallback.
func CommandHandlerClientFromEnv(envVar, defaultEndpoint string) (*CommandHandlerClient, error) {
	endpoint := os.Getenv(envVar)
	if endpoint == "" {
		endpoint = defaultEndpoint
	}
	return NewCommandHandlerClient(endpoint)
}

// CommandHandlerClientFromConn creates a client from an existing connection.
func CommandHandlerClientFromConn(conn *grpc.ClientConn) *CommandHandlerClient {
	return &CommandHandlerClient{
		inner: pb.NewCommandHandlerCoordinatorServiceClient(conn),
		conn:  conn,
	}
}

// Handle executes a command asynchronously (fire-and-forget).
// Convenience method that wraps CommandBook in CommandRequest with default sync mode.
func (c *CommandHandlerClient) Handle(ctx context.Context, cmd *pb.CommandBook) (*pb.CommandResponse, error) {
	request := &pb.CommandRequest{
		Command:  cmd,
		SyncMode: pb.SyncMode_SYNC_MODE_ASYNC,
	}
	return c.HandleCommand(ctx, request)
}

// HandleCommand executes a command with the specified sync mode.
func (c *CommandHandlerClient) HandleCommand(ctx context.Context, request *pb.CommandRequest) (*pb.CommandResponse, error) {
	resp, err := c.inner.HandleCommand(ctx, request)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// HandleSyncSpeculative executes a command speculatively against temporal state (no persistence).
func (c *CommandHandlerClient) HandleSyncSpeculative(ctx context.Context, req *pb.SpeculateCommandHandlerRequest) (*pb.CommandResponse, error) {
	resp, err := c.inner.HandleSyncSpeculative(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// Close closes the underlying connection.
func (c *CommandHandlerClient) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// SpeculativeClient wraps coordinator services for speculative execution.
// Speculative execution runs commands/events against temporal state without persistence.
type SpeculativeClient struct {
	chStub        pb.CommandHandlerCoordinatorServiceClient
	sagaStub      pb.SagaCoordinatorServiceClient
	projectorStub pb.ProjectorCoordinatorServiceClient
	pmStub        pb.ProcessManagerCoordinatorServiceClient
	conn          *grpc.ClientConn
}

// NewSpeculativeClient connects to coordinator services at the given endpoint.
func NewSpeculativeClient(endpoint string) (*SpeculativeClient, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &SpeculativeClient{
		chStub:        pb.NewCommandHandlerCoordinatorServiceClient(conn),
		sagaStub:      pb.NewSagaCoordinatorServiceClient(conn),
		projectorStub: pb.NewProjectorCoordinatorServiceClient(conn),
		pmStub:        pb.NewProcessManagerCoordinatorServiceClient(conn),
		conn:          conn,
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
		chStub:        pb.NewCommandHandlerCoordinatorServiceClient(conn),
		sagaStub:      pb.NewSagaCoordinatorServiceClient(conn),
		projectorStub: pb.NewProjectorCoordinatorServiceClient(conn),
		pmStub:        pb.NewProcessManagerCoordinatorServiceClient(conn),
		conn:          conn,
	}
}

// CommandHandler executes a command speculatively against temporal state.
func (c *SpeculativeClient) CommandHandler(ctx context.Context, req *pb.SpeculateCommandHandlerRequest) (*pb.CommandResponse, error) {
	resp, err := c.chStub.HandleSyncSpeculative(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// Projector speculatively executes a projector against events.
func (c *SpeculativeClient) Projector(ctx context.Context, req *pb.SpeculateProjectorRequest) (*pb.Projection, error) {
	resp, err := c.projectorStub.HandleSpeculative(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// Saga speculatively executes a saga against events.
func (c *SpeculativeClient) Saga(ctx context.Context, req *pb.SpeculateSagaRequest) (*pb.SagaResponse, error) {
	resp, err := c.sagaStub.ExecuteSpeculative(ctx, req)
	if err != nil {
		return nil, GRPCError(err)
	}
	return resp, nil
}

// ProcessManager speculatively executes a process manager.
func (c *SpeculativeClient) ProcessManager(ctx context.Context, req *pb.SpeculatePmRequest) (*pb.ProcessManagerHandleResponse, error) {
	resp, err := c.pmStub.HandleSpeculative(ctx, req)
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

// DomainClient combines command handler and query clients for a single domain.
type DomainClient struct {
	CommandHandler *CommandHandlerClient
	Query          *QueryClient
	conn           *grpc.ClientConn
}

// NewDomainClient connects to a domain's coordinator at the given endpoint.
func NewDomainClient(endpoint string) (*DomainClient, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &DomainClient{
		CommandHandler: CommandHandlerClientFromConn(conn),
		Query:          QueryClientFromConn(conn),
		conn:           conn,
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
		CommandHandler: CommandHandlerClientFromConn(conn),
		Query:          QueryClientFromConn(conn),
		conn:           conn,
	}
}

// DomainClientForDomain connects to a domain's command handler.
//
// Resolves the domain name to the appropriate endpoint based on transport mode.
// Pass an empty string for mode to auto-detect from ANGZARR_MODE env var.
//
// Examples:
//
//	// Auto-detect mode from ANGZARR_MODE env var
//	player, err := DomainClientForDomain("player", "")
//
//	// Explicitly use standalone mode (Unix Domain Sockets)
//	player, err := DomainClientForDomain("player", TransportStandalone)
//
//	// Explicitly use distributed mode (K8s DNS)
//	player, err := DomainClientForDomain("player", TransportDistributed)
func DomainClientForDomain(domain string, mode TransportMode) (*DomainClient, error) {
	endpoint := ResolveCHEndpoint(domain, mode)
	return NewDomainClient(endpoint)
}

// Execute executes a command asynchronously (fire-and-forget).
// Use ExecuteWithMode to specify a different sync mode.
func (c *DomainClient) Execute(ctx context.Context, cmd *pb.CommandBook) (*pb.CommandResponse, error) {
	return c.CommandHandler.Handle(ctx, cmd)
}

// ExecuteWithMode executes a command with the specified sync mode.
//
// Use pb.SyncMode_SYNC_MODE_ASYNC for fire-and-forget (default).
// Use pb.SyncMode_SYNC_MODE_SIMPLE to wait for sync projectors.
// Use pb.SyncMode_SYNC_MODE_CASCADE for full sync including saga cascade.
func (c *DomainClient) ExecuteWithMode(ctx context.Context, cmd *pb.CommandBook, syncMode pb.SyncMode) (*pb.CommandResponse, error) {
	request := &pb.CommandRequest{
		Command:  cmd,
		SyncMode: syncMode,
	}
	return c.CommandHandler.HandleCommand(ctx, request)
}

// Close closes the underlying connection.
func (c *DomainClient) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// Client combines command handler, query, and speculative clients.
type Client struct {
	CommandHandler *CommandHandlerClient
	Query          *QueryClient
	Speculative    *SpeculativeClient
	conn           *grpc.ClientConn
}

// NewClient connects to a server providing all services.
func NewClient(endpoint string) (*Client, error) {
	conn, err := grpc.NewClient(formatEndpoint(endpoint), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return nil, TransportError(err)
	}
	return &Client{
		CommandHandler: CommandHandlerClientFromConn(conn),
		Query:          QueryClientFromConn(conn),
		Speculative:    SpeculativeClientFromConn(conn),
		conn:           conn,
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
		CommandHandler: CommandHandlerClientFromConn(conn),
		Query:          QueryClientFromConn(conn),
		Speculative:    SpeculativeClientFromConn(conn),
		conn:           conn,
	}
}

// Close closes the underlying connection.
func (c *Client) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}
