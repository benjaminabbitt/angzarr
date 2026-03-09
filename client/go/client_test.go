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

type mockCHCoordinatorServiceClient struct {
	handleCommandFn         func(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error)
	handleEventFn           func(ctx context.Context, in *pb.EventRequest, opts ...grpc.CallOption) (*pb.FactInjectionResponse, error)
	handleSyncSpeculativeFn func(ctx context.Context, in *pb.SpeculateCommandHandlerRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error)
	handleCompensationFn    func(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.BusinessResponse, error)
}

func (m *mockCHCoordinatorServiceClient) HandleCommand(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
	if m.handleCommandFn != nil {
		return m.handleCommandFn(ctx, in, opts...)
	}
	return &pb.CommandResponse{}, nil
}

func (m *mockCHCoordinatorServiceClient) HandleEvent(ctx context.Context, in *pb.EventRequest, opts ...grpc.CallOption) (*pb.FactInjectionResponse, error) {
	if m.handleEventFn != nil {
		return m.handleEventFn(ctx, in, opts...)
	}
	return &pb.FactInjectionResponse{}, nil
}

func (m *mockCHCoordinatorServiceClient) HandleSyncSpeculative(ctx context.Context, in *pb.SpeculateCommandHandlerRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
	if m.handleSyncSpeculativeFn != nil {
		return m.handleSyncSpeculativeFn(ctx, in, opts...)
	}
	return &pb.CommandResponse{}, nil
}

func (m *mockCHCoordinatorServiceClient) HandleCompensation(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.BusinessResponse, error) {
	if m.handleCompensationFn != nil {
		return m.handleCompensationFn(ctx, in, opts...)
	}
	return &pb.BusinessResponse{}, nil
}

type mockSagaCoordinatorServiceClient struct {
	executeSpeculativeFn func(ctx context.Context, in *pb.SpeculateSagaRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error)
}

func (m *mockSagaCoordinatorServiceClient) Execute(ctx context.Context, in *pb.SagaHandleRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error) {
	return &pb.SagaResponse{}, nil
}

func (m *mockSagaCoordinatorServiceClient) ExecuteSpeculative(ctx context.Context, in *pb.SpeculateSagaRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error) {
	if m.executeSpeculativeFn != nil {
		return m.executeSpeculativeFn(ctx, in, opts...)
	}
	return &pb.SagaResponse{}, nil
}

type mockProjectorCoordinatorServiceClient struct {
	handleSpeculativeFn func(ctx context.Context, in *pb.SpeculateProjectorRequest, opts ...grpc.CallOption) (*pb.Projection, error)
}

func (m *mockProjectorCoordinatorServiceClient) HandleSync(ctx context.Context, in *pb.EventRequest, opts ...grpc.CallOption) (*pb.Projection, error) {
	return &pb.Projection{}, nil
}

func (m *mockProjectorCoordinatorServiceClient) Handle(ctx context.Context, in *pb.EventBook, opts ...grpc.CallOption) (*emptypb.Empty, error) {
	return &emptypb.Empty{}, nil
}

func (m *mockProjectorCoordinatorServiceClient) HandleSpeculative(ctx context.Context, in *pb.SpeculateProjectorRequest, opts ...grpc.CallOption) (*pb.Projection, error) {
	if m.handleSpeculativeFn != nil {
		return m.handleSpeculativeFn(ctx, in, opts...)
	}
	return &pb.Projection{}, nil
}

type mockProcessManagerCoordinatorServiceClient struct {
	handleSpeculativeFn func(ctx context.Context, in *pb.SpeculatePmRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error)
}

func (m *mockProcessManagerCoordinatorServiceClient) Prepare(ctx context.Context, in *pb.ProcessManagerPrepareRequest, opts ...grpc.CallOption) (*pb.ProcessManagerPrepareResponse, error) {
	return &pb.ProcessManagerPrepareResponse{}, nil
}

func (m *mockProcessManagerCoordinatorServiceClient) Handle(ctx context.Context, in *pb.ProcessManagerHandleRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error) {
	return &pb.ProcessManagerHandleResponse{}, nil
}

func (m *mockProcessManagerCoordinatorServiceClient) HandleSpeculative(ctx context.Context, in *pb.SpeculatePmRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error) {
	if m.handleSpeculativeFn != nil {
		return m.handleSpeculativeFn(ctx, in, opts...)
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

// CommandHandlerClient tests

func TestCommandHandlerClient_Handle(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		expected := &pb.CommandResponse{Events: &pb.EventBook{NextSequence: 10}}
		mock := &mockCHCoordinatorServiceClient{
			handleCommandFn: func(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return expected, nil
			},
		}
		client := &CommandHandlerClient{inner: mock}

		result, err := client.Handle(context.Background(), &pb.CommandBook{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if result.Events.NextSequence != 10 {
			t.Errorf("got NextSequence %d, want 10", result.Events.NextSequence)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockCHCoordinatorServiceClient{
			handleCommandFn: func(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return nil, status.Error(codes.FailedPrecondition, "sequence mismatch")
			},
		}
		client := &CommandHandlerClient{inner: mock}

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

func TestCommandHandlerClient_HandleCommand(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockCHCoordinatorServiceClient{
			handleCommandFn: func(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return &pb.CommandResponse{}, nil
			},
		}
		client := &CommandHandlerClient{inner: mock}

		_, err := client.HandleCommand(context.Background(), &pb.CommandRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockCHCoordinatorServiceClient{
			handleCommandFn: func(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return nil, status.Error(codes.Internal, "internal error")
			},
		}
		client := &CommandHandlerClient{inner: mock}

		_, err := client.HandleCommand(context.Background(), &pb.CommandRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestCommandHandlerClient_HandleSyncSpeculative(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockCHCoordinatorServiceClient{
			handleSyncSpeculativeFn: func(ctx context.Context, in *pb.SpeculateCommandHandlerRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return &pb.CommandResponse{}, nil
			},
		}
		client := &CommandHandlerClient{inner: mock}

		_, err := client.HandleSyncSpeculative(context.Background(), &pb.SpeculateCommandHandlerRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockCHCoordinatorServiceClient{
			handleSyncSpeculativeFn: func(ctx context.Context, in *pb.SpeculateCommandHandlerRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return nil, status.Error(codes.InvalidArgument, "invalid")
			},
		}
		client := &CommandHandlerClient{inner: mock}

		_, err := client.HandleSyncSpeculative(context.Background(), &pb.SpeculateCommandHandlerRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestCommandHandlerClient_Close(t *testing.T) {
	t.Run("nil connection", func(t *testing.T) {
		client := &CommandHandlerClient{conn: nil}
		err := client.Close()
		if err != nil {
			t.Errorf("unexpected error: %v", err)
		}
	})
}

func TestCommandHandlerClientFromConn(t *testing.T) {
	client := CommandHandlerClientFromConn(nil)
	if client == nil {
		t.Error("expected non-nil client")
	}
}

// SpeculativeClient tests

func TestSpeculativeClient_CommandHandler(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockCHCoordinatorServiceClient{
			handleSyncSpeculativeFn: func(ctx context.Context, in *pb.SpeculateCommandHandlerRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return &pb.CommandResponse{}, nil
			},
		}
		client := &SpeculativeClient{chStub: mock}

		_, err := client.CommandHandler(context.Background(), &pb.SpeculateCommandHandlerRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockCHCoordinatorServiceClient{
			handleSyncSpeculativeFn: func(ctx context.Context, in *pb.SpeculateCommandHandlerRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
				return nil, status.Error(codes.Internal, "error")
			},
		}
		client := &SpeculativeClient{chStub: mock}

		_, err := client.CommandHandler(context.Background(), &pb.SpeculateCommandHandlerRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestSpeculativeClient_Projector(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockProjectorCoordinatorServiceClient{
			handleSpeculativeFn: func(ctx context.Context, in *pb.SpeculateProjectorRequest, opts ...grpc.CallOption) (*pb.Projection, error) {
				return &pb.Projection{}, nil
			},
		}
		client := &SpeculativeClient{projectorStub: mock}

		_, err := client.Projector(context.Background(), &pb.SpeculateProjectorRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockProjectorCoordinatorServiceClient{
			handleSpeculativeFn: func(ctx context.Context, in *pb.SpeculateProjectorRequest, opts ...grpc.CallOption) (*pb.Projection, error) {
				return nil, status.Error(codes.Internal, "error")
			},
		}
		client := &SpeculativeClient{projectorStub: mock}

		_, err := client.Projector(context.Background(), &pb.SpeculateProjectorRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestSpeculativeClient_Saga(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockSagaCoordinatorServiceClient{
			executeSpeculativeFn: func(ctx context.Context, in *pb.SpeculateSagaRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error) {
				return &pb.SagaResponse{}, nil
			},
		}
		client := &SpeculativeClient{sagaStub: mock}

		_, err := client.Saga(context.Background(), &pb.SpeculateSagaRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockSagaCoordinatorServiceClient{
			executeSpeculativeFn: func(ctx context.Context, in *pb.SpeculateSagaRequest, opts ...grpc.CallOption) (*pb.SagaResponse, error) {
				return nil, status.Error(codes.Internal, "error")
			},
		}
		client := &SpeculativeClient{sagaStub: mock}

		_, err := client.Saga(context.Background(), &pb.SpeculateSagaRequest{})
		if err == nil {
			t.Fatal("expected error")
		}
	})
}

func TestSpeculativeClient_ProcessManager(t *testing.T) {
	t.Run("successful response", func(t *testing.T) {
		mock := &mockProcessManagerCoordinatorServiceClient{
			handleSpeculativeFn: func(ctx context.Context, in *pb.SpeculatePmRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error) {
				return &pb.ProcessManagerHandleResponse{}, nil
			},
		}
		client := &SpeculativeClient{pmStub: mock}

		_, err := client.ProcessManager(context.Background(), &pb.SpeculatePmRequest{})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
	})

	t.Run("grpc error", func(t *testing.T) {
		mock := &mockProcessManagerCoordinatorServiceClient{
			handleSpeculativeFn: func(ctx context.Context, in *pb.SpeculatePmRequest, opts ...grpc.CallOption) (*pb.ProcessManagerHandleResponse, error) {
				return nil, status.Error(codes.Internal, "error")
			},
		}
		client := &SpeculativeClient{pmStub: mock}

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
	mock := &mockCHCoordinatorServiceClient{
		handleCommandFn: func(ctx context.Context, in *pb.CommandRequest, opts ...grpc.CallOption) (*pb.CommandResponse, error) {
			return expected, nil
		},
	}
	client := &DomainClient{
		CommandHandler: &CommandHandlerClient{inner: mock},
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
	if client.CommandHandler == nil {
		t.Error("expected non-nil CommandHandler")
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
	if client.CommandHandler == nil {
		t.Error("expected non-nil CommandHandler")
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

func TestCommandHandlerClientFromEnv(t *testing.T) {
	t.Run("uses env var when set", func(t *testing.T) {
		os.Setenv("TEST_CH_ENDPOINT_12345", "localhost:99999")
		defer os.Unsetenv("TEST_CH_ENDPOINT_12345")

		_, err := CommandHandlerClientFromEnv("TEST_CH_ENDPOINT_12345", "default:8000")
		_ = err
	})

	t.Run("uses default when env not set", func(t *testing.T) {
		os.Unsetenv("NONEXISTENT_VAR_12345")

		_, err := CommandHandlerClientFromEnv("NONEXISTENT_VAR_12345", "localhost:99999")
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
