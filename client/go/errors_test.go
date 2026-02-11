package angzarr

import (
	"errors"
	"fmt"
	"testing"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

func TestErrorKindConstants(t *testing.T) {
	// Verify error kind constants are distinct
	kinds := []ErrorKind{ErrConnection, ErrTransport, ErrGRPC, ErrInvalidArgument, ErrInvalidTimestamp}
	seen := make(map[ErrorKind]bool)
	for _, k := range kinds {
		if seen[k] {
			t.Errorf("duplicate error kind: %v", k)
		}
		seen[k] = true
	}
}

func TestClientError_Error_WithCause(t *testing.T) {
	cause := errors.New("underlying error")
	err := &ClientError{Kind: ErrTransport, Message: "transport failed", Cause: cause}

	got := err.Error()
	want := "transport failed: underlying error"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestClientError_Error_NoCause(t *testing.T) {
	err := &ClientError{Kind: ErrConnection, Message: "connection refused"}

	got := err.Error()
	want := "connection refused"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestClientError_Unwrap(t *testing.T) {
	cause := errors.New("root cause")
	err := &ClientError{Kind: ErrTransport, Message: "transport", Cause: cause}

	if err.Unwrap() != cause {
		t.Error("Unwrap should return the cause")
	}
}

func TestClientError_Unwrap_Nil(t *testing.T) {
	err := &ClientError{Kind: ErrConnection, Message: "no cause"}

	if err.Unwrap() != nil {
		t.Error("Unwrap should return nil when no cause")
	}
}

func TestClientError_Code_GRPCError(t *testing.T) {
	grpcErr := status.Error(codes.NotFound, "not found")
	err := &ClientError{Kind: ErrGRPC, Message: "grpc error", Cause: grpcErr}

	if err.Code() != codes.NotFound {
		t.Errorf("got %v, want %v", err.Code(), codes.NotFound)
	}
}

func TestClientError_Code_NonGRPCKind(t *testing.T) {
	err := &ClientError{Kind: ErrConnection, Message: "connection error"}

	if err.Code() != codes.Unknown {
		t.Errorf("got %v, want %v", err.Code(), codes.Unknown)
	}
}

func TestClientError_Code_NilCause(t *testing.T) {
	err := &ClientError{Kind: ErrGRPC, Message: "grpc error", Cause: nil}

	if err.Code() != codes.Unknown {
		t.Errorf("got %v, want %v", err.Code(), codes.Unknown)
	}
}

func TestClientError_Code_NonStatusCause(t *testing.T) {
	err := &ClientError{Kind: ErrGRPC, Message: "grpc error", Cause: errors.New("not a status")}

	if err.Code() != codes.Unknown {
		t.Errorf("got %v, want %v", err.Code(), codes.Unknown)
	}
}

func TestClientError_Status_GRPCError(t *testing.T) {
	grpcErr := status.Error(codes.PermissionDenied, "access denied")
	err := &ClientError{Kind: ErrGRPC, Message: "grpc error", Cause: grpcErr}

	s := err.Status()
	if s == nil {
		t.Fatal("expected non-nil status")
	}
	if s.Code() != codes.PermissionDenied {
		t.Errorf("got %v, want %v", s.Code(), codes.PermissionDenied)
	}
	if s.Message() != "access denied" {
		t.Errorf("got %q, want %q", s.Message(), "access denied")
	}
}

func TestClientError_Status_NonGRPCKind(t *testing.T) {
	err := &ClientError{Kind: ErrTransport, Message: "transport error"}

	if err.Status() != nil {
		t.Error("expected nil status for non-gRPC error")
	}
}

func TestClientError_Status_NilCause(t *testing.T) {
	err := &ClientError{Kind: ErrGRPC, Message: "grpc error", Cause: nil}

	if err.Status() != nil {
		t.Error("expected nil status when cause is nil")
	}
}

func TestClientError_IsNotFound(t *testing.T) {
	tests := []struct {
		name     string
		err      *ClientError
		expected bool
	}{
		{
			name:     "not found error",
			err:      &ClientError{Kind: ErrGRPC, Cause: status.Error(codes.NotFound, "")},
			expected: true,
		},
		{
			name:     "other grpc error",
			err:      &ClientError{Kind: ErrGRPC, Cause: status.Error(codes.Internal, "")},
			expected: false,
		},
		{
			name:     "non-grpc error",
			err:      &ClientError{Kind: ErrConnection, Message: "conn"},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.err.IsNotFound(); got != tt.expected {
				t.Errorf("got %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestClientError_IsPreconditionFailed(t *testing.T) {
	tests := []struct {
		name     string
		err      *ClientError
		expected bool
	}{
		{
			name:     "precondition failed",
			err:      &ClientError{Kind: ErrGRPC, Cause: status.Error(codes.FailedPrecondition, "")},
			expected: true,
		},
		{
			name:     "other grpc error",
			err:      &ClientError{Kind: ErrGRPC, Cause: status.Error(codes.NotFound, "")},
			expected: false,
		},
		{
			name:     "non-grpc error",
			err:      &ClientError{Kind: ErrTransport},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.err.IsPreconditionFailed(); got != tt.expected {
				t.Errorf("got %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestClientError_IsInvalidArgument(t *testing.T) {
	tests := []struct {
		name     string
		err      *ClientError
		expected bool
	}{
		{
			name:     "invalid argument kind",
			err:      &ClientError{Kind: ErrInvalidArgument, Message: "bad arg"},
			expected: true,
		},
		{
			name:     "grpc invalid argument",
			err:      &ClientError{Kind: ErrGRPC, Cause: status.Error(codes.InvalidArgument, "")},
			expected: true,
		},
		{
			name:     "other error",
			err:      &ClientError{Kind: ErrConnection},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.err.IsInvalidArgument(); got != tt.expected {
				t.Errorf("got %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestClientError_IsConnectionError(t *testing.T) {
	tests := []struct {
		name     string
		err      *ClientError
		expected bool
	}{
		{
			name:     "connection error",
			err:      &ClientError{Kind: ErrConnection},
			expected: true,
		},
		{
			name:     "transport error",
			err:      &ClientError{Kind: ErrTransport},
			expected: true,
		},
		{
			name:     "grpc error",
			err:      &ClientError{Kind: ErrGRPC},
			expected: false,
		},
		{
			name:     "invalid argument",
			err:      &ClientError{Kind: ErrInvalidArgument},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.err.IsConnectionError(); got != tt.expected {
				t.Errorf("got %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestConnectionError(t *testing.T) {
	err := ConnectionError("connection refused")

	if err.Kind != ErrConnection {
		t.Errorf("got kind %v, want %v", err.Kind, ErrConnection)
	}
	if err.Message != "connection refused" {
		t.Errorf("got message %q, want %q", err.Message, "connection refused")
	}
	if err.Cause != nil {
		t.Error("expected nil cause")
	}
}

func TestTransportError(t *testing.T) {
	cause := errors.New("network unreachable")
	err := TransportError(cause)

	if err.Kind != ErrTransport {
		t.Errorf("got kind %v, want %v", err.Kind, ErrTransport)
	}
	if err.Message != "transport error" {
		t.Errorf("got message %q, want %q", err.Message, "transport error")
	}
	if err.Cause != cause {
		t.Error("cause mismatch")
	}
}

func TestGRPCError(t *testing.T) {
	cause := status.Error(codes.Internal, "server error")
	err := GRPCError(cause)

	if err.Kind != ErrGRPC {
		t.Errorf("got kind %v, want %v", err.Kind, ErrGRPC)
	}
	if err.Message != "grpc error" {
		t.Errorf("got message %q, want %q", err.Message, "grpc error")
	}
	if err.Cause != cause {
		t.Error("cause mismatch")
	}
}

func TestInvalidArgumentError(t *testing.T) {
	err := InvalidArgumentError("missing field")

	if err.Kind != ErrInvalidArgument {
		t.Errorf("got kind %v, want %v", err.Kind, ErrInvalidArgument)
	}
	if err.Message != "missing field" {
		t.Errorf("got message %q, want %q", err.Message, "missing field")
	}
}

func TestInvalidTimestampError(t *testing.T) {
	err := InvalidTimestampError("bad format")

	if err.Kind != ErrInvalidTimestamp {
		t.Errorf("got kind %v, want %v", err.Kind, ErrInvalidTimestamp)
	}
	if err.Message != "bad format" {
		t.Errorf("got message %q, want %q", err.Message, "bad format")
	}
}

func TestIsClientError(t *testing.T) {
	tests := []struct {
		name     string
		err      error
		expected bool
	}{
		{
			name:     "client error",
			err:      ConnectionError("test"),
			expected: true,
		},
		{
			name:     "wrapped client error",
			err:      fmt.Errorf("wrapped: %w", ConnectionError("test")),
			expected: true,
		},
		{
			name:     "standard error",
			err:      errors.New("regular error"),
			expected: false,
		},
		{
			name:     "nil error",
			err:      nil,
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := IsClientError(tt.err); got != tt.expected {
				t.Errorf("got %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestAsClientError(t *testing.T) {
	t.Run("client error", func(t *testing.T) {
		original := ConnectionError("test")
		result := AsClientError(original)
		if result != original {
			t.Error("expected same error")
		}
	})

	t.Run("wrapped client error", func(t *testing.T) {
		original := ConnectionError("test")
		wrapped := fmt.Errorf("wrapped: %w", original)
		result := AsClientError(wrapped)
		if result != original {
			t.Error("expected unwrapped error")
		}
	})

	t.Run("standard error", func(t *testing.T) {
		result := AsClientError(errors.New("regular"))
		if result != nil {
			t.Error("expected nil for non-client error")
		}
	})

	t.Run("nil error", func(t *testing.T) {
		result := AsClientError(nil)
		if result != nil {
			t.Error("expected nil for nil input")
		}
	})
}
