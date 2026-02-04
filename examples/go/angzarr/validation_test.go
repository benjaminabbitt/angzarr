package angzarr

import "testing"

func TestRequireExists_PassesWhenNonEmpty(t *testing.T) {
	if err := RequireExists("value", "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
}

func TestRequireExists_FailsWhenEmpty(t *testing.T) {
	err := RequireExists("", "entity not found")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if err.Code != StatusFailedPrecondition {
		t.Errorf("expected FailedPrecondition, got %v", err.Code)
	}
	if err.Message != "entity not found" {
		t.Errorf("expected 'entity not found', got %q", err.Message)
	}
}

func TestRequireNotExists_PassesWhenEmpty(t *testing.T) {
	if err := RequireNotExists("", "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
}

func TestRequireNotExists_FailsWhenNonEmpty(t *testing.T) {
	err := RequireNotExists("value", "already exists")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if err.Message != "already exists" {
		t.Errorf("expected 'already exists', got %q", err.Message)
	}
}

func TestRequirePositive_Passes(t *testing.T) {
	if err := RequirePositive(1, "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
	if err := RequirePositive(100, "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
}

func TestRequirePositive_FailsOnZero(t *testing.T) {
	err := RequirePositive(0, "must be positive")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if err.Message != "must be positive" {
		t.Errorf("expected 'must be positive', got %q", err.Message)
	}
}

func TestRequirePositive_FailsOnNegative(t *testing.T) {
	if err := RequirePositive(-1, "error"); err == nil {
		t.Fatal("expected error, got nil")
	}
}

func TestRequireNonNegative_Passes(t *testing.T) {
	if err := RequireNonNegative(0, "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
	if err := RequireNonNegative(1, "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
}

func TestRequireNonNegative_Fails(t *testing.T) {
	if err := RequireNonNegative(-1, "error"); err == nil {
		t.Fatal("expected error, got nil")
	}
}

func TestRequireNotEmpty_Passes(t *testing.T) {
	if err := RequireNotEmpty([]int{1, 2, 3}, "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
}

func TestRequireNotEmpty_Fails(t *testing.T) {
	err := RequireNotEmpty([]int{}, "items required")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if err.Message != "items required" {
		t.Errorf("expected 'items required', got %q", err.Message)
	}
}

func TestRequireStatus_Passes(t *testing.T) {
	if err := RequireStatus("active", "active", "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
}

func TestRequireStatus_Fails(t *testing.T) {
	err := RequireStatus("pending", "active", "wrong status")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if err.Message != "wrong status" {
		t.Errorf("expected 'wrong status', got %q", err.Message)
	}
}

func TestRequireStatusNot_Passes(t *testing.T) {
	if err := RequireStatusNot("active", "checked_out", "error"); err != nil {
		t.Errorf("expected nil, got %v", err)
	}
}

func TestRequireStatusNot_Fails(t *testing.T) {
	err := RequireStatusNot("checked_out", "checked_out", "already checked out")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if err.Message != "already checked out" {
		t.Errorf("expected 'already checked out', got %q", err.Message)
	}
}
