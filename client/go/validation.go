// Package angzarr provides validation helpers for command handler precondition checks.
package angzarr

// RequireExists checks that an aggregate exists (has prior events).
func RequireExists(exists bool, message string) error {
	if !exists {
		return NewCommandRejectedError(message)
	}
	return nil
}

// RequireNotExists checks that an aggregate does not exist.
func RequireNotExists(exists bool, message string) error {
	if exists {
		return NewCommandRejectedError(message)
	}
	return nil
}

// RequirePositive checks that a value is positive (greater than zero).
func RequirePositive[T ~int | ~int32 | ~int64 | ~float32 | ~float64](value T, fieldName string) error {
	if value <= 0 {
		return NewCommandRejectedError(fieldName + " must be positive")
	}
	return nil
}

// RequireNonNegative checks that a value is non-negative (zero or greater).
func RequireNonNegative[T ~int | ~int32 | ~int64 | ~float32 | ~float64](value T, fieldName string) error {
	if value < 0 {
		return NewCommandRejectedError(fieldName + " must be non-negative")
	}
	return nil
}

// RequireNotEmptyString checks that a string is not empty.
func RequireNotEmptyString(value string, fieldName string) error {
	if value == "" {
		return NewCommandRejectedError(fieldName + " must not be empty")
	}
	return nil
}

// RequireNotEmpty checks that a slice is not empty.
func RequireNotEmpty[T any](items []T, fieldName string) error {
	if len(items) == 0 {
		return NewCommandRejectedError(fieldName + " must not be empty")
	}
	return nil
}

// RequireStatus checks that a status matches an expected value.
func RequireStatus[T comparable](actual, expected T, message string) error {
	if actual != expected {
		return NewCommandRejectedError(message)
	}
	return nil
}

// RequireStatusNot checks that a status does not match a forbidden value.
func RequireStatusNot[T comparable](actual, forbidden T, message string) error {
	if actual == forbidden {
		return NewCommandRejectedError(message)
	}
	return nil
}
