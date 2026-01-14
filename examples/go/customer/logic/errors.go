package logic

import "fmt"

// StatusCode represents command validation error codes.
type StatusCode int

const (
	StatusInvalidArgument StatusCode = iota
	StatusFailedPrecondition
)

// String returns the string representation of the status code.
func (s StatusCode) String() string {
	switch s {
	case StatusInvalidArgument:
		return "INVALID_ARGUMENT"
	case StatusFailedPrecondition:
		return "FAILED_PRECONDITION"
	default:
		return "UNKNOWN"
	}
}

// CommandError represents a validation error from command handling.
type CommandError struct {
	Code    StatusCode
	Message string
}

func (e *CommandError) Error() string {
	return e.Message
}

// NewInvalidArgument creates an INVALID_ARGUMENT error.
func NewInvalidArgument(message string) *CommandError {
	return &CommandError{Code: StatusInvalidArgument, Message: message}
}

// NewFailedPrecondition creates a FAILED_PRECONDITION error.
func NewFailedPrecondition(message string) *CommandError {
	return &CommandError{Code: StatusFailedPrecondition, Message: message}
}

// NewFailedPreconditionf creates a FAILED_PRECONDITION error with formatting.
func NewFailedPreconditionf(format string, args ...interface{}) *CommandError {
	return &CommandError{Code: StatusFailedPrecondition, Message: fmt.Sprintf(format, args...)}
}
