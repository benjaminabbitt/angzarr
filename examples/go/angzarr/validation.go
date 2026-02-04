package angzarr

// RequireExists checks that a field is non-empty (entity exists).
func RequireExists(field, errMsg string) *CommandError {
	if field == "" {
		return NewFailedPrecondition(errMsg)
	}
	return nil
}

// RequireNotExists checks that a field is empty (entity does not yet exist).
func RequireNotExists(field, errMsg string) *CommandError {
	if field != "" {
		return NewFailedPrecondition(errMsg)
	}
	return nil
}

// RequirePositive checks that a value is greater than zero.
func RequirePositive(value int32, errMsg string) *CommandError {
	if value <= 0 {
		return NewFailedPrecondition(errMsg)
	}
	return nil
}

// RequireNonNegative checks that a value is zero or greater.
func RequireNonNegative(value int32, errMsg string) *CommandError {
	if value < 0 {
		return NewFailedPrecondition(errMsg)
	}
	return nil
}

// RequireNotEmpty checks that a slice has at least one element.
func RequireNotEmpty[T any](items []T, errMsg string) *CommandError {
	if len(items) == 0 {
		return NewFailedPrecondition(errMsg)
	}
	return nil
}

// RequireStatus checks that the current status matches the expected value.
func RequireStatus(actual, expected, errMsg string) *CommandError {
	if actual != expected {
		return NewFailedPrecondition(errMsg)
	}
	return nil
}

// RequireStatusNot checks that the current status is NOT the forbidden value.
func RequireStatusNot(actual, forbidden, errMsg string) *CommandError {
	if actual == forbidden {
		return NewFailedPrecondition(errMsg)
	}
	return nil
}
