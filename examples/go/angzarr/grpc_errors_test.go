package angzarr

import (
	"errors"
	"testing"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

func TestMapCommandError_invalidArgument_mapsToGRPCInvalidArgument(t *testing.T) {
	err := MapCommandError(NewInvalidArgument("bad field"))
	st, ok := status.FromError(err)
	if !ok {
		t.Fatal("expected gRPC status error")
	}
	if st.Code() != codes.InvalidArgument {
		t.Errorf("expected InvalidArgument, got %v", st.Code())
	}
	if st.Message() != "bad field" {
		t.Errorf("expected 'bad field', got %q", st.Message())
	}
}

func TestMapCommandError_failedPrecondition_mapsToGRPCFailedPrecondition(t *testing.T) {
	err := MapCommandError(NewFailedPrecondition("not ready"))
	st, ok := status.FromError(err)
	if !ok {
		t.Fatal("expected gRPC status error")
	}
	if st.Code() != codes.FailedPrecondition {
		t.Errorf("expected FailedPrecondition, got %v", st.Code())
	}
	if st.Message() != "not ready" {
		t.Errorf("expected 'not ready', got %q", st.Message())
	}
}

func TestMapCommandError_nonCommandError_mapsToInternal(t *testing.T) {
	err := MapCommandError(errors.New("something broke"))
	st, ok := status.FromError(err)
	if !ok {
		t.Fatal("expected gRPC status error")
	}
	if st.Code() != codes.Internal {
		t.Errorf("expected Internal, got %v", st.Code())
	}
}
