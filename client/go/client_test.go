package angzarr

import (
	"context"
	"os"
	"testing"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/types/known/emptypb"
)

// Mock implementations for testing

type mockEventQueryServiceClient struct {
	getEventBookFn func(ctx context.Context, in *pb.Query, opts ...grpc.CallOption) (*pb.EventBook, error)
	getEventsFn    func(ctx context.Context, in *pb.Query, opts ...grpc.CallOption) (grpc.ServerStreamingClient[pb.EventBook], error)
}

func (m *mockEventQueryServiceClient) GetEventBook(ctx context.Context, in *pb.Query, opts ...grpc.CallOption) (*pb.EventBook, error) {
	if m.getEventBookFn != nil {
		return m.getEventBookFn(ctx, in, opts...)
	}
	return &pb.EventBook{}, nil
}

func (m *mockEventQueryServiceClient) GetEvents(ctx context.Context, in *pb.Query, opts ...grpc.CallOption) (grpc.ServerStreamingClient[pb.EventBook], error) {
	if m.getEventsFn != nil {
		return m.getEventsFn(ctx, in, opts...)
	}
	return nil, nil
}

func (m *mockEventQueryServiceClient) Synchronize(ctx context.Context, opts ...grpc.CallOption) (grpc.BidiStreamingClient[pb.Query, pb.EventBook], error) {
	return nil, nil
}

func (m *mockEventQueryServiceClient) GetAggregateRoots(ctx context.Context, in *emptypb.Empty, opts ...grpc.CallOption) (grpc.ServerStreamingClient[pb.AggregateRoot], error) {
	return nil, nil
}

type mockAggregateCoordinatorServiceClient struct {
	handleFn       func(ctx context.Context, in *pb.CommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error)
	handleSyncFn   func(ctx context.Context, in *pb.SyncCommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error)
	dryRunHandleFn func(ctx context.Context, in *pb.DryRunRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error)
}

func (m *mockAggregateCoordinatorServiceClient) Handle(ctx context.Context, in *pb.CommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
	if m.handleFn != nil {
		return m.handleFn(ctx, in, opts...)
	}
	return &pb.CommandResponse{}, nil
}

func (m *mockAggregateCoordinatorServiceClient) HandleSync(ctx context.Context, in *pb.SyncCommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
	if m.handleSyncFn != nil {
		return m.handleSyncFn(ctx, in, opts...)
	}
	return &pb.CommandResponse{}, nil
}

func (m *mockAggregateCoordinatorServiceClient) DryRunHandle(ctx context.Context, in *pb.DryRunRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
	if m.dryRunHandleFn != nil {
		return m.dryRunHandleFn(ctx, in, opts...)
	}
	return &pb.CommandResponse{}, nil
}

type mockSpeculativeServiceClient struct {
	dryRunCommandFn          func(ctx context.Context, in *pb.DryRunRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error)
	speculateProjectorFn     func(ctx context.Context, in *pb.SpeculateProjectorRequest, opts ...grpc.CallOption) (*pb.Projection, error)
	speculateSagaFn          func(ctx context.Context, in *pb.SpeculateSagaRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error)
	speculateProcessManagerFn func(ctx context.Context, in *pb.SpeculatePmRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error)
}

func (m *mockSpeculativeServiceClient) DryRunCommand(ctx context.Context, in *pb.DryRunRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
	if m.dryRunCommandFn != nil {
		return m.dryRunCommandFn(ctx, in, opts...)
	}
	return &pb.CommandResponse{}, nil
}

func (m *mockSpeculativeServiceClient) SpeculateProjector(ctx context.Context, in *pb.SpeculateProjectorRequest, opts ...grpc.CallOption) (*pb.Projection, error) {
	if m.speculateProjectorFn != nil {
		return m.speculateProjectorFn(ctx, in, opts...)
	}
	return &pb.Projection{}, nil
}

func (m *mockSpeculativeServiceClient) SpeculateSaga(ctx context.Context, in *pb.SpeculateSagaRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error) {
	if m.speculateSagaFn != nil {
		return m.speculateSagaFn(ctx, in, opts...)
	}
	return &pb.SagaResponse{}, nil
}

func (m *mockSpeculativeServiceClient) SpeculateProcessManager(ctx context.Context, in *pb.SpeculatePmRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error) {
	if m.speculateProcessManagerFn != nil {
		return m.speculateProcessManagerFn(ctx, in, opts...)
	}
	return &pb.ProcessManagerHandleResponse{}, nil
}

// QueryClient tests

func TestQueryClient_GetEventBook(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		expected := &pb.EventBook{NextSequence: 5}
		mock := &mockEventQueryServiceClient{
			getEventBookFn: func(ctx context.Context, in *pb.Query, opts ...grpc.CallOption) (*pb.EventBook, error) {
				return expected, nil
			},
		}
		client := &QueryClient{inner: mock}

		result, err := client.GetEventBook(context.Background(), &pb.Query{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if result.NextSequence != 5 {
			t.Errorf("got NextSequence %d, want 5", result.NextSequence)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockEventQueryServiceClient{
			getEventBookFn: func(ctx context.Context, in *pb.Query, opts ...grpc.CallOption) (*pb.EventBook, error) {
				return nil, status.Error(codes.NotFound, "not found")
			},
		}
		client := &QueryClient{inner: mock}

		_, err := client.GetEventBook(context.Background(), &pb.Query{})
		if err == nil {
			t.Fatal("expected error")
		}
		clientErr := AsClientError(err)
		if clientErr == nil {
			t.Fatal("expected ClientError")
		}
		if clientErr.Kind != ErrGRPC {
			t.Errorf("got kind %v, want ErrGRPC", clientErr.Kind)
		}
	})
}

func TestQueryClient_GetEvents(t *testing.T) {
	t.Run("grpc error on stream creation", func(t *testing.T) {
		mock := &mockEventQueryServiceClient{
			getEventsFn: func(ctx context.Context, in *pb.Query, opts ...grpc.CallOption) (pb.EventQueryService_GetEventsClient, error) {
				return nil, status.Error(codes.Internal, "internal error")
			},
		}
		client := &QueryClient{inner: mock}

		_, err := client.GetEvents(context.Background(), &pb.Query{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestQueryClient_Close(t *testing.T) {
	t.Run("nil connection", func(t *testing.T) {
		client := &QueryClient{conn: nil}
		err := client.Close()
		if err != nil {
			t.Errorf("unexpected error: %v", err)
		}
	})
}

func TestQueryClientFromConn(t *testing.T) {
	// Can't create a real connection without a server, but we can test the function structure
	client := QueryClientFromConn(nil)
	if client == nil {
		t.Error("expected non-nil client")
	}
}

// AggregateClient tests

func TestAggregateClient_Handle(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		expected := &pb.CommandResponse{Events: &pb.EventBook{NextSequence: 10}}
		mock := &mockAggregateCoordinatorServiceClient{
			handleFn: func(ctx context.Context, in *pb.CommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return expected, nil
			},
		}
		client := &AggregateClient{inner: mock}

		result, err := client.Handle(context.Background(), &pb.CommandBook{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if result.Events.NextSequence != 10 {
			t.Errorf("got NextSequence %d, want 10", result.Events.NextSequence)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockAggregateCoordinatorServiceClient{
			handleFn: func(ctx context.Context, in *pb.CommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return nil, status.Error(codes.FailedPrecondition, "sequence mismatch")
			},
		}
		client := &AggregateClient{inner: mock}

		_, err := client.Handle(context.Background(), &pb.CommandBook{})
		if err == nil {
			t.Fatal("expected error")
		}
		clientErr := AsClientError(err)
		if clientErr == nil || !clientErr.IsPreconditionFailed() {
			t.Error("expected precondition failed error")
		}
	})
}

func TestAggregateClient_HandleSync(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockAggregateCoordinatorServiceClient{
			handleSyncFn: func(ctx context.Context, in *pb.SyncCommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return &pb.CommandResponse{}, nil
			},
		}
		client := &AggregateClient{inner: mock}

		_, err := client.HandleSync(context.Background(), &pb.SyncCommandBook{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockAggregateCoordinatorServiceClient{
			handleSyncFn: func(ctx context.Context, in *pb.SyncCommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return nil, status.Error(codes.Internal, "internal error")
			},
		}
		client := &AggregateClient{inner: mock}

		_, err := client.HandleSync(context.Background(), &pb.SyncCommandBook{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestAggregateClient_DryRunHandle(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockAggregateCoordinatorServiceClient{
			dryRunHandleFn: func(ctx context.Context, in *pb.DryRunRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return &pb.CommandResponse{}, nil
			},
		}
		client := &AggregateClient{inner: mock}

		_, err := client.DryRunHandle(context.Background(), &pb.DryRunRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockAggregateCoordinatorServiceClient{
			dryRunHandleFn: func(ctx context.Context, in *pb.DryRunRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return nil, status.Error(codes.InvalidArgument, "invalid")
			},
		}
		client := &AggregateClient{inner: mock}

		_, err := client.DryRunHandle(context.Background(), &pb.DryRunRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestAggregateClient_Close(t *testing.T) {
	t.Run("nil connection", func(t *testing.T) {
		client := &AggregateClient{conn: nil}
		err := client.Close()
		if err != nil {
			t.Errorf("unexpected error: %v", err)
		}
	})
}

func TestAggregateClientFromConn(t *testing.T) {
	client := AggregateClientFromConn(nil)
	if client == nil {
		t.Error("expected non-nil client")
	}
}

// SpeculativeClient tests

func TestSpeculativeClient_DryRun(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockSpeculativeServiceClient{
			dryRunCommandFn: func(ctx context.Context, in *pb.DryRunRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return &pb.CommandResponse{}, nil
			},
		}
		client := &SpeculativeClient{inner: mock}

		_, err := client.DryRun(context.Background(), &pb.DryRunRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockSpeculativeServiceClient{
			dryRunCommandFn: func(ctx context.Context, in *pb.DryRunRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return nil, status.Error(codes.Internal, "error")
			},
		}
		client := &SpeculativeClient{inner: mock}

		_, err := client.DryRun(context.Background(), &pb.DryRunRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestSpeculativeClient_Projector(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockSpeculativeServiceClient{
			speculateProjectorFn: func(ctx context.Context, in *pb.SpeculateProjectorRequest, opts ...grpc.CallOption) (*pb.Projection, error) {
				return &pb.Projection{}, nil
			},
		}
		client := &SpeculativeClient{inner: mock}

		_, err := client.Projector(context.Background(), &pb.SpeculateProjectorRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockSpeculativeServiceClient{
			speculateProjectorFn: func(ctx context.Context, in *pb.SpeculateProjectorRequest, opts ...grpc.CallOption) (*pb.Projection, error) {
				return nil, status.Error(codes.Internal, "error")
			},
		}
		client := &SpeculativeClient{inner: mock}

		_, err := client.Projector(context.Background(), &pb.SpeculateProjectorRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestSpeculativeClient_Saga(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockSpeculativeServiceClient{
			speculateSagaFn: func(ctx context.Context, in *pb.SpeculateSagaRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error) {
				return &pb.SagaResponse{}, nil
			},
		}
		client := &SpeculativeClient{inner: mock}

		_, err := client.Saga(context.Background(), &pb.SpeculateSagaRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockSpeculativeServiceClient{
			speculateSagaFn: func(ctx context.Context, in *pb.SpeculateSagaRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error) {
				return nil, status.Error(codes.Internal, "error")
			},
		}
		client := &SpeculativeClient{inner: mock}

		_, err := client.Saga(context.Background(), &pb.SpeculateSagaRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestSpeculativeClient_ProcessManager(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockSpeculativeServiceClient{
			speculateProcessManagerFn: func(ctx context.Context, in *pb.SpeculatePmRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error) {
				return &pb.ProcessManagerHandleResponse{}, nil
			},
		}
		client := &SpeculativeClient{inner: mock}

		_, err := client.ProcessManager(context.Background(), &pb.SpeculatePmRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockSpeculativeServiceClient{
			speculateProcessManagerFn: func(ctx context.Context, in *pb.SpeculatePmRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error) {
				return nil, status.Error(codes.Internal, "error")
			},
		}
		client := &SpeculativeClient{inner: mock}

		_, err := client.ProcessManager(context.Background(), &pb.SpeculatePmRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestSpeculativeClient_Close(t *testing.T) {
	t.Run("nil connection", func(t *testing.T) {
		client := &SpeculativeClient{conn: nil}
		err := client.Close()
		if err != nil {
			t.Errorf("unexpected error: %v", err)
		}
	})
}

func TestSpeculativeClientFromConn(t *testing.T) {
	client := SpeculativeClientFromConn(nil)
	if client == nil {
		t.Error("expected non-nil client")
	}
}

// DomainClient tests

func TestDomainClient_Execute(t *testing.T) {
	expected := &pb.CommandResponse{}
	mock := &mockAggregateCoordinatorServiceClient{
		handleFn: func(ctx context.Context, in *pb.CommandBook, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
			return expected, nil
		},
	}
	client := &DomainClient{
		Aggregate: &AggregateClient{inner: mock},
	}

	result, err := client.Execute(context.Background(), &pb.CommandBook{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != expected {
		t.Error("expected same response")
	}
}

func TestDomainClient_Close(t *testing.T) {
	t.Run("nil connection", func(t *testing.T) {
		client := &DomainClient{conn: nil}
		err := client.Close()
		if err != nil {
			t.Errorf("unexpected error: %v", err)
		}
	})
}

func TestDomainClientFromConn(t *testing.T) {
	client := DomainClientFromConn(nil)
	if client == nil {
		t.Error("expected non-nil client")
	}
	if client.Aggregate == nil {
		t.Error("expected non-nil Aggregate")
	}
	if client.Query == nil {
		t.Error("expected non-nil Query")
	}
}

// Client tests

func TestClient_Close(t *testing.T) {
	t.Run("nil connection", func(t *testing.T) {
		client := &Client{conn: nil}
		err := client.Close()
		if err != nil {
			t.Errorf("unexpected error: %v", err)
		}
	})
}

func TestClientFromConn(t *testing.T) {
	client := ClientFromConn(nil)
	if client == nil {
		t.Error("expected non-nil client")
	}
	if client.Aggregate == nil {
		t.Error("expected non-nil Aggregate")
	}
	if client.Query == nil {
		t.Error("expected non-nil Query")
	}
	if client.Speculative == nil {
		t.Error("expected non-nil Speculative")
	}
}

// FromEnv tests

func TestQueryClientFromEnv(t *testing.T) {
	t.Run("uses env var when set", func(t *testing.T) {
		// This test would need a real server, so we just verify it doesn't panic
		// with a nonexistent endpoint
		os.Setenv("TEST_QUERY_ENDPOINT_12345", "localhost:99999")
		defer os.Unsetenv("TEST_QUERY_ENDPOINT_12345")

		// Will fail to connect, but shouldn't panic
		_, err := QueryClientFromEnv("TEST_QUERY_ENDPOINT_12345", "default:8000")
		// Connection may fail, but that's expected without a real server
		_ = err
	})

	t.Run("uses default when env not set", func(t *testing.T) {
		os.Unsetenv("NONEXISTENT_VAR_12345")

		// Will fail to connect, but tests the default path
		_, err := QueryClientFromEnv("NONEXISTENT_VAR_12345", "localhost:99999")
		_ = err
	})
}

func TestAggregateClientFromEnv(t *testing.T) {
	t.Run("uses env var when set", func(t *testing.T) {
		os.Setenv("TEST_AGG_ENDPOINT_12345", "localhost:99999")
		defer os.Unsetenv("TEST_AGG_ENDPOINT_12345")

		_, err := AggregateClientFromEnv("TEST_AGG_ENDPOINT_12345", "default:8000")
		_ = err
	})

	t.Run("uses default when env not set", func(t *testing.T) {
		os.Unsetenv("NONEXISTENT_VAR_12345")

		_, err := AggregateClientFromEnv("NONEXISTENT_VAR_12345", "localhost:99999")
		_ = err
	})
}

func TestSpeculativeClientFromEnv(t *testing.T) {
	t.Run("uses env var when set", func(t *testing.T) {
		os.Setenv("TEST_SPEC_ENDPOINT_12345", "localhost:99999")
		defer os.Unsetenv("TEST_SPEC_ENDPOINT_12345")

		_, err := SpeculativeClientFromEnv("TEST_SPEC_ENDPOINT_12345", "default:8000")
		_ = err
	})
}

func TestDomainClientFromEnv(t *testing.T) {
	t.Run("uses env var when set", func(t *testing.T) {
		os.Setenv("TEST_DOMAIN_ENDPOINT_12345", "localhost:99999")
		defer os.Unsetenv("TEST_DOMAIN_ENDPOINT_12345")

		_, err := DomainClientFromEnv("TEST_DOMAIN_ENDPOINT_12345", "default:8000")
		_ = err
	})
}

func TestClientFromEnv(t *testing.T) {
	t.Run("uses env var when set", func(t *testing.T) {
		os.Setenv("TEST_CLIENT_ENDPOINT_12345", "localhost:99999")
		defer os.Unsetenv("TEST_CLIENT_ENDPOINT_12345")

		_, err := ClientFromEnv("TEST_CLIENT_ENDPOINT_12345", "default:8000")
		_ = err
	})
}
