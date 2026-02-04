package angzarr

import (
	"testing"
)

func TestNewInvalidArgument_setsCodeAndMessage(t *testing.T) {
	err := NewInvalidArgument("bad input")
	if err.Code != StatusInvalidArgument {
		t.Errorf("expected StatusInvalidArgument, got %v", err.Code)
	}
	if err.Message != "bad input" {
		t.Errorf("expected 'bad input', got %q", err.Message)
	}
}

func TestNewFailedPrecondition_setsCodeAndMessage(t *testing.T) {
	err := NewFailedPrecondition("not ready")
	if err.Code != StatusFailedPrecondition {
		t.Errorf("expected StatusFailedPrecondition, got %v", err.Code)
	}
	if err.Message != "not ready" {
		t.Errorf("expected 'not ready', got %q", err.Message)
	}
}

func TestNewFailedPreconditionf_formatsMessage(t *testing.T) {
	err := NewFailedPreconditionf("item %s not found", "abc")
	if err.Code != StatusFailedPrecondition {
		t.Errorf("expected StatusFailedPrecondition, got %v", err.Code)
	}
	if err.Message != "item abc not found" {
		t.Errorf("expected 'item abc not found', got %q", err.Message)
	}
}

func TestCommandError_Error_returnsMessage(t *testing.T) {
	err := NewInvalidArgument("test message")
	if err.Error() != "test message" {
		t.Errorf("expected 'test message', got %q", err.Error())
	}
}

func TestStatusCode_String_returnsLabel(t *testing.T) {
	tests := []struct {
		code StatusCode
		want string
	}{
		{StatusInvalidArgument, "INVALID_ARGUMENT"},
		{StatusFailedPrecondition, "FAILED_PRECONDITION"},
		{StatusCode(99), "UNKNOWN"},
	}
	for _, tt := range tests {
		if got := tt.code.String(); got != tt.want {
			t.Errorf("StatusCode(%d).String() = %q, want %q", tt.code, got, tt.want)
		}
	}
}
