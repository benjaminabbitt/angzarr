package angzarr

import "fmt"

// StatusCode represents the category of a command rejection.
type StatusCode int

const (
	StatusInvalidArgument    StatusCode = iota
	StatusFailedPrecondition
)

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

// CommandError is returned when a command is rejected by business logic.
type CommandError struct {
	Code    StatusCode
	Message string
}

func (e *CommandError) Error() string {
	return e.Message
}

// NewInvalidArgument creates a CommandError for invalid input.
func NewInvalidArgument(message string) *CommandError {
	return &CommandError{Code: StatusInvalidArgument, Message: message}
}

// NewFailedPrecondition creates a CommandError for violated preconditions.
func NewFailedPrecondition(message string) *CommandError {
	return &CommandError{Code: StatusFailedPrecondition, Message: message}
}

// NewFailedPreconditionf creates a CommandError with a formatted message.
func NewFailedPreconditionf(format string, args ...interface{}) *CommandError {
	return &CommandError{Code: StatusFailedPrecondition, Message: fmt.Sprintf(format, args...)}
}
