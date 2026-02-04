package angzarr

import (
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// MapCommandError converts a CommandError to a gRPC status error.
// Non-CommandError values are wrapped as Internal.
func MapCommandError(err error) error {
	if cmdErr, ok := err.(*CommandError); ok {
		switch cmdErr.Code {
		case StatusInvalidArgument:
			return status.Error(codes.InvalidArgument, cmdErr.Message)
		case StatusFailedPrecondition:
			return status.Error(codes.FailedPrecondition, cmdErr.Message)
		}
	}
	return status.Errorf(codes.Internal, "internal error: %v", err)
}
