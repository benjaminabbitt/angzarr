package logic

import "fmt"

type StatusCode int

const (
	StatusInvalidArgument StatusCode = iota
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

type CommandError struct {
	Code    StatusCode
	Message string
}

func (e *CommandError) Error() string {
	return e.Message
}

func NewInvalidArgument(message string) *CommandError {
	return &CommandError{Code: StatusInvalidArgument, Message: message}
}

func NewFailedPrecondition(message string) *CommandError {
	return &CommandError{Code: StatusFailedPrecondition, Message: message}
}

func NewFailedPreconditionf(format string, args ...interface{}) *CommandError {
	return &CommandError{Code: StatusFailedPrecondition, Message: fmt.Sprintf(format, args...)}
}
