// Package angzarr provides a client library for Angzarr gRPC services.
package angzarr

import (
	"errors"
	"fmt"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// ClientError represents errors from client operations.
type ClientError struct {
	Kind    ErrorKind
	Message string
	Cause   error
}

// ErrorKind categorizes client errors.
type ErrorKind int

const (
	// ErrConnection indicates a connection failure.
	ErrConnection ErrorKind = iota
	// ErrTransport indicates a transport-level error.
	ErrTransport
	// ErrGRPC indicates a gRPC error from the server.
	ErrGRPC
	// ErrInvalidArgument indicates an invalid argument from the caller.
	ErrInvalidArgument
	// ErrInvalidTimestamp indicates a timestamp parsing failure.
	ErrInvalidTimestamp
)

func (e *ClientError) Error() string {
	if e.Cause != nil {
		return fmt.Sprintf("%s: %v", e.Message, e.Cause)
	}
	return e.Message
}

func (e *ClientError) Unwrap() error {
	return e.Cause
}

// Code returns the gRPC status code if this is a gRPC error.
func (e *ClientError) Code() codes.Code {
	if e.Kind != ErrGRPC || e.Cause == nil {
		return codes.Unknown
	}
	if s, ok := status.FromError(e.Cause); ok {
		return s.Code()
	}
	return codes.Unknown
}

// Status returns the gRPC Status if this is a gRPC error.
func (e *ClientError) Status() *status.Status {
	if e.Kind != ErrGRPC || e.Cause == nil {
		return nil
	}
	s, _ := status.FromError(e.Cause)
	return s
}

// IsNotFound returns true if this is a "not found" error.
func (e *ClientError) IsNotFound() bool {
	return e.Code() == codes.NotFound
}

// IsPreconditionFailed returns true if this is a "precondition failed" error.
func (e *ClientError) IsPreconditionFailed() bool {
	return e.Code() == codes.FailedPrecondition
}

// IsInvalidArgument returns true if this is an "invalid argument" error.
func (e *ClientError) IsInvalidArgument() bool {
	return e.Kind == ErrInvalidArgument || e.Code() == codes.InvalidArgument
}

// IsConnectionError returns true if this is a connection or transport error.
func (e *ClientError) IsConnectionError() bool {
	return e.Kind == ErrConnection || e.Kind == ErrTransport
}

// Error constructors

// ConnectionError creates a connection error.
func ConnectionError(msg string) *ClientError {
	return &ClientError{Kind: ErrConnection, Message: msg}
}

// TransportError wraps a transport error.
func TransportError(err error) *ClientError {
	return &ClientError{Kind: ErrTransport, Message: "transport error", Cause: err}
}

// GRPCError wraps a gRPC error.
func GRPCError(err error) *ClientError {
	return &ClientError{Kind: ErrGRPC, Message: "grpc error", Cause: err}
}

// InvalidArgumentError creates an invalid argument error.
func InvalidArgumentError(msg string) *ClientError {
	return &ClientError{Kind: ErrInvalidArgument, Message: msg}
}

// InvalidTimestampError creates a timestamp parsing error.
func InvalidTimestampError(msg string) *ClientError {
	return &ClientError{Kind: ErrInvalidTimestamp, Message: msg}
}

// IsClientError checks if an error is a ClientError.
func IsClientError(err error) bool {
	var clientErr *ClientError
	return errors.As(err, &clientErr)
}

// AsClientError extracts a ClientError from an error chain.
func AsClientError(err error) *ClientError {
	var clientErr *ClientError
	if errors.As(err, &clientErr) {
		return clientErr
	}
	return nil
}
