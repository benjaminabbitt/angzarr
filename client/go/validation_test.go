package angzarr

import (
	"errors"
	"testing"
)

func TestRequireExists(t *testing.T) {
	t.Run("passes when exists", func(t *testing.T) {
		err := RequireExists(true, "should exist")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("fails when not exists", func(t *testing.T) {
		err := RequireExists(false, "Player does not exist")
		if err == nil {
			t.Error("expected error, got nil")
		}
		var rejected CommandRejectedError
		if !errors.As(err, &rejected) {
			t.Errorf("expected CommandRejectedError, got %T", err)
		}
		if rejected.Message != "Player does not exist" {
			t.Errorf("expected message 'Player does not exist', got '%s'", rejected.Message)
		}
	})
}

func TestRequireNotExists(t *testing.T) {
	t.Run("passes when not exists", func(t *testing.T) {
		err := RequireNotExists(false, "should not exist")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("fails when exists", func(t *testing.T) {
		err := RequireNotExists(true, "Player already exists")
		if err == nil {
			t.Error("expected error, got nil")
		}
	})
}

func TestRequirePositive(t *testing.T) {
	t.Run("passes for positive int", func(t *testing.T) {
		err := RequirePositive(1, "amount")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("passes for positive int64", func(t *testing.T) {
		err := RequirePositive(int64(100), "value")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("fails for zero", func(t *testing.T) {
		err := RequirePositive(0, "amount")
		if err == nil {
			t.Error("expected error, got nil")
		}
		var rejected CommandRejectedError
		if errors.As(err, &rejected) && rejected.Message != "amount must be positive" {
			t.Errorf("expected 'amount must be positive', got '%s'", rejected.Message)
		}
	})

	t.Run("fails for negative", func(t *testing.T) {
		err := RequirePositive(-5, "amount")
		if err == nil {
			t.Error("expected error, got nil")
		}
	})
}

func TestRequireNonNegative(t *testing.T) {
	t.Run("passes for zero", func(t *testing.T) {
		err := RequireNonNegative(0, "balance")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("passes for positive", func(t *testing.T) {
		err := RequireNonNegative(100, "balance")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("fails for negative", func(t *testing.T) {
		err := RequireNonNegative(-1, "balance")
		if err == nil {
			t.Error("expected error, got nil")
		}
		var rejected CommandRejectedError
		if errors.As(err, &rejected) && rejected.Message != "balance must be non-negative" {
			t.Errorf("expected 'balance must be non-negative', got '%s'", rejected.Message)
		}
	})
}

func TestRequireNotEmptyString(t *testing.T) {
	t.Run("passes for non-empty string", func(t *testing.T) {
		err := RequireNotEmptyString("hello", "name")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("fails for empty string", func(t *testing.T) {
		err := RequireNotEmptyString("", "name")
		if err == nil {
			t.Error("expected error, got nil")
		}
		var rejected CommandRejectedError
		if errors.As(err, &rejected) && rejected.Message != "name must not be empty" {
			t.Errorf("expected 'name must not be empty', got '%s'", rejected.Message)
		}
	})
}

func TestRequireNotEmpty(t *testing.T) {
	t.Run("passes for non-empty slice", func(t *testing.T) {
		err := RequireNotEmpty([]int{1, 2, 3}, "items")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("fails for empty slice", func(t *testing.T) {
		err := RequireNotEmpty([]int{}, "items")
		if err == nil {
			t.Error("expected error, got nil")
		}
		var rejected CommandRejectedError
		if errors.As(err, &rejected) && rejected.Message != "items must not be empty" {
			t.Errorf("expected 'items must not be empty', got '%s'", rejected.Message)
		}
	})
}

func TestRequireStatus(t *testing.T) {
	t.Run("passes when status matches", func(t *testing.T) {
		err := RequireStatus("active", "active", "must be active")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("fails when status differs", func(t *testing.T) {
		err := RequireStatus("pending", "active", "must be active")
		if err == nil {
			t.Error("expected error, got nil")
		}
		var rejected CommandRejectedError
		if errors.As(err, &rejected) && rejected.Message != "must be active" {
			t.Errorf("expected 'must be active', got '%s'", rejected.Message)
		}
	})
}

func TestRequireStatusNot(t *testing.T) {
	t.Run("passes when status differs", func(t *testing.T) {
		err := RequireStatusNot("active", "deleted", "cannot be deleted")
		if err != nil {
			t.Errorf("expected nil, got %v", err)
		}
	})

	t.Run("fails when status matches forbidden", func(t *testing.T) {
		err := RequireStatusNot("deleted", "deleted", "cannot be deleted")
		if err == nil {
			t.Error("expected error, got nil")
		}
		var rejected CommandRejectedError
		if errors.As(err, &rejected) && rejected.Message != "cannot be deleted" {
			t.Errorf("expected 'cannot be deleted', got '%s'", rejected.Message)
		}
	})
}
