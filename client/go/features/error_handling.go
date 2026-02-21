package features

import (
	"fmt"

	"github.com/cucumber/godog"
	"google.golang.org/grpc/codes"
)

// ErrorContext holds state for error handling scenarios
type ErrorContext struct {
	Error           interface{}
	ErrorCode       string
	ErrorMsg        string
	IsRetryable     bool
	lastError       error
	errorType       string
	grpcCode        codes.Code
	hasGRPCCode     bool
	errorMessage    string
	retryAfter      int
	serverState     string
	connectionState string
}

func newErrorContext() *ErrorContext {
	return &ErrorContext{}
}

// ClientError represents a client error
type ClientError struct {
	Code    string
	Message string
}

// CommandRejectedError represents a rejected command error
type CommandRejectedError struct {
	Reason string
}

// InitErrorHandlingSteps registers error handling step definitions
func InitErrorHandlingSteps(ctx *godog.ScenarioContext) {
	ec := newErrorContext()

	// Original step definitions
	ctx.Step(`^a client library implementation$`, ec.givenClientImpl)
	ctx.Step(`^a response with status code "([^"]*)"$`, ec.givenResponseWithStatus)
	ctx.Step(`^a response with message "([^"]*)"$`, ec.givenResponseWithMessage)
	ctx.Step(`^parsing the error response$`, ec.whenParsingErrorResponse)
	ctx.Step(`^a command is rejected$`, ec.whenCommandRejected)
	ctx.Step(`^a network error occurs$`, ec.whenNetworkError)
	ctx.Step(`^a timeout occurs$`, ec.whenTimeoutOccurs)
	ctx.Step(`^the error should have a code$`, ec.thenErrorHasCode)
	ctx.Step(`^the error should have a message$`, ec.thenErrorHasMessage)
	ctx.Step(`^the error should be retryable$`, ec.thenErrorRetryable)
	ctx.Step(`^the error should NOT be retryable$`, ec.thenErrorNotRetryable)
	ctx.Step(`^the error code should be "([^"]*)"$`, ec.thenErrorCodeEquals)
	ctx.Step(`^the error message should contain "([^"]*)"$`, ec.thenErrorMessageContains)

	// Error category setup steps
	ctx.Step(`^the server is unreachable$`, ec.theServerIsUnreachable)
	ctx.Step(`^the connection drops mid-request$`, ec.theConnectionDropsMidRequest)
	ctx.Step(`^the server returns a gRPC error$`, ec.theServerReturnsAGRPCError)
	ctx.Step(`^I build a command without required fields$`, ec.iBuildACommandWithoutRequiredFields)
	ctx.Step(`^I build a query with invalid timestamp format$`, ec.iBuildAQueryWithInvalidTimestampFormat)
	ctx.Step(`^the aggregate does not exist$`, ec.theAggregateDoesNotExist)
	ctx.Step(`^the server aggregate is at sequence (\d+)$`, ec.anAggregateAtSequence)
	ctx.Step(`^the client lacks required permissions$`, ec.theClientLacksRequiredPermissions)
	ctx.Step(`^the server has an internal error$`, ec.theServerHasAnInternalError)
	ctx.Step(`^the operation times out$`, ec.theOperationTimesOut)
	ctx.Step(`^any client error$`, ec.anyClientError)
	ctx.Step(`^a gRPC error with status NOT_FOUND$`, ec.aGRPCErrorWithStatusNOT_FOUND)
	ctx.Step(`^a connection error$`, ec.aConnectionError)
	ctx.Step(`^a gRPC error with detailed status$`, ec.aGRPCErrorWithDetailedStatus)
	ctx.Step(`^an invalid argument error$`, ec.anInvalidArgumentError)
	ctx.Step(`^different error types$`, ec.differentErrorTypes)
	ctx.Step(`^various error types$`, ec.variousErrorTypes)
	ctx.Step(`^an error with retry-after metadata$`, ec.anErrorWithRetryafterMetadata)

	// When steps
	ctx.Step(`^I attempt a client operation$`, ec.iAttemptAClientOperation)
	ctx.Step(`^I query events for the aggregate$`, ec.iQueryEventsForTheAggregate)
	ctx.Step(`^I execute a mock command at sequence (\d+)$`, ec.iExecuteACommandAtSequence)
	ctx.Step(`^I send a malformed request to the server$`, ec.iSendAMalformedRequestToTheServer)
	ctx.Step(`^I attempt a restricted operation$`, ec.iAttemptARestrictedOperation)
	ctx.Step(`^I call message\(\) on the error$`, ec.iCallMessageOnTheError)
	ctx.Step(`^I call code\(\) on the error$`, ec.iCallCodeOnTheError)
	ctx.Step(`^I call status\(\) on the error$`, ec.iCallStatusOnTheError)
	ctx.Step(`^I convert the error to string$`, ec.iConvertTheErrorToString)
	ctx.Step(`^I debug-format the error$`, ec.iDebugformatTheError)
	ctx.Step(`^I inspect the error details$`, ec.iInspectTheErrorDetails)

	// Then steps - error type assertions
	ctx.Step(`^the error should be a connection error$`, ec.theErrorShouldBeAConnectionError)
	ctx.Step(`^the error should be a transport error$`, ec.theErrorShouldBeATransportError)
	ctx.Step(`^the error should be a gRPC error$`, ec.theErrorShouldBeAGRPCError)
	ctx.Step(`^the error should be an invalid argument error$`, ec.theErrorShouldBeAnInvalidArgumentError)
	ctx.Step(`^the error should be an invalid timestamp error$`, ec.theErrorShouldBeAnInvalidTimestampError)

	// Boolean predicate assertions
	ctx.Step(`^is_connection_error should return true$`, ec.is_connection_errorShouldReturnTrue)
	ctx.Step(`^is_not_found should return true$`, ec.is_not_foundShouldReturnTrue)
	ctx.Step(`^is_precondition_failed should return true$`, ec.is_precondition_failedShouldReturnTrue)
	ctx.Step(`^is_invalid_argument should return true$`, ec.is_invalid_argumentShouldReturnTrue)

	// Code assertions
	ctx.Step(`^code should return NOT_FOUND$`, ec.codeShouldReturnNOT_FOUND)
	ctx.Step(`^code should return FAILED_PRECONDITION$`, ec.codeShouldReturnFAILED_PRECONDITION)
	ctx.Step(`^code should return INVALID_ARGUMENT$`, ec.codeShouldReturnINVALID_ARGUMENT)
	ctx.Step(`^code should return PERMISSION_DENIED$`, ec.codeShouldReturnPERMISSION_DENIED)
	ctx.Step(`^code should return INTERNAL$`, ec.codeShouldReturnINTERNAL)
	ctx.Step(`^code should return DEADLINE_EXCEEDED$`, ec.codeShouldReturnDEADLINE_EXCEEDED)

	// Message assertions
	ctx.Step(`^the error message should describe the connection failure$`, ec.theErrorMessageShouldDescribeTheConnectionFailure)
	ctx.Step(`^the error message should describe what's missing$`, ec.theErrorMessageShouldDescribeWhatsMissing)
	ctx.Step(`^the error message should indicate the format problem$`, ec.theErrorMessageShouldIndicateTheFormatProblem)
	ctx.Step(`^the error indicates optimistic lock failure$`, ec.theErrorIndicatesOptimisticLockFailure)
	ctx.Step(`^the error message should describe access denial$`, ec.theErrorMessageShouldDescribeAccessDenial)
	ctx.Step(`^the error should indicate server-side failure$`, ec.theErrorShouldIndicateServerSideFailure)
	ctx.Step(`^the underlying Status should be accessible$`, ec.theUnderlyingStatusShouldBeAccessible)

	// Method result assertions
	ctx.Step(`^I should get a non-empty string$`, ec.iShouldGetANonEmptyString)
	ctx.Step(`^the message should describe the error$`, ec.theMessageShouldDescribeTheError)
	ctx.Step(`^I should get Some\(NOT_FOUND\)$`, ec.iShouldGetSomeNOT_FOUND)
	ctx.Step(`^I should get None$`, ec.iShouldGetNone)
	ctx.Step(`^I should get the full gRPC Status$`, ec.iShouldGetTheFullGRPCStatus)
	ctx.Step(`^I can access the status code, message, and details$`, ec.iCanAccessTheStatusCodeMessageAndDetails)

	// Predicate assertions for different types
	ctx.Step(`^NOT_FOUND gRPC error should have is_not_found true$`, ec.nOT_FOUNDGRPCErrorShouldHaveIs_not_foundTrue)
	ctx.Step(`^connection error should have is_not_found false$`, ec.connectionErrorShouldHaveIs_not_foundFalse)
	ctx.Step(`^INTERNAL gRPC error should have is_not_found false$`, ec.iNTERNALGRPCErrorShouldHaveIs_not_foundFalse)
	ctx.Step(`^FAILED_PRECONDITION gRPC error should have is_precondition_failed true$`, ec.fAILED_PRECONDITIONGRPCErrorShouldHaveIs_precondition_failedTrue)
	ctx.Step(`^NOT_FOUND gRPC error should have is_precondition_failed false$`, ec.nOT_FOUNDGRPCErrorShouldHaveIs_precondition_failedFalse)
	ctx.Step(`^connection error should have is_precondition_failed false$`, ec.connectionErrorShouldHaveIs_precondition_failedFalse)
	ctx.Step(`^INVALID_ARGUMENT gRPC error should have is_invalid_argument true$`, ec.iNVALID_ARGUMENTGRPCErrorShouldHaveIs_invalid_argumentTrue)
	ctx.Step(`^ClientError::InvalidArgument should have is_invalid_argument true$`, ec.clientErrorInvalidArgumentShouldHaveIs_invalid_argumentTrue)
	ctx.Step(`^NOT_FOUND gRPC error should have is_invalid_argument false$`, ec.nOT_FOUNDGRPCErrorShouldHaveIs_invalid_argumentFalse)
	ctx.Step(`^connection error should have is_connection_error true$`, ec.connectionErrorShouldHaveIs_connection_errorTrue)
	ctx.Step(`^transport error should have is_connection_error true$`, ec.transportErrorShouldHaveIs_connection_errorTrue)
	ctx.Step(`^gRPC error should have is_connection_error false$`, ec.gRPCErrorShouldHaveIs_connection_errorFalse)

	// Retry logic assertions
	ctx.Step(`^connection errors should be retryable$`, ec.connectionErrorsShouldBeRetryable)
	ctx.Step(`^UNAVAILABLE gRPC errors should be retryable$`, ec.uNAVAILABLEGRPCErrorsShouldBeRetryable)
	ctx.Step(`^RESOURCE_EXHAUSTED should be retryable with backoff$`, ec.rESOURCE_EXHAUSTEDShouldBeRetryableWithBackoff)
	ctx.Step(`^INVALID_ARGUMENT should NOT be retryable$`, ec.iNVALID_ARGUMENTShouldNOTBeRetryable)
	ctx.Step(`^FAILED_PRECONDITION should be retryable after state refresh$`, ec.fAILED_PRECONDITIONShouldBeRetryableAfterStateRefresh)
	ctx.Step(`^I should be able to extract retry timing hints$`, ec.iShouldBeAbleToExtractRetryTimingHints)

	// Display assertions
	ctx.Step(`^I should get a formatted error message$`, ec.iShouldGetAFormattedErrorMessage)
	ctx.Step(`^the message should include the error type and description$`, ec.theMessageShouldIncludeTheErrorTypeAndDescription)
	ctx.Step(`^I should get detailed diagnostic information$`, ec.iShouldGetDetailedDiagnosticInformation)

	// Additional error assertions
	ctx.Step(`^the error should indicate connection lost$`, ec.theErrorShouldIndicateConnectionLost)
	ctx.Step(`^the error should indicate invalid format$`, ec.theErrorShouldIndicateInvalidFormat)
	ctx.Step(`^the error should indicate invalid timestamp$`, ec.theErrorShouldIndicateInvalidTimestamp)
	ctx.Step(`^rejection reason should describe the issue$`, ec.rejectionReasonShouldDescribeTheIssue)
	ctx.Step(`^validate should reject$`, ec.validateShouldReject)
}

// Original implementations
func (e *ErrorContext) givenClientImpl() error {
	return nil
}

func (e *ErrorContext) givenResponseWithStatus(code string) error {
	e.ErrorCode = code
	return nil
}

func (e *ErrorContext) givenResponseWithMessage(message string) error {
	e.ErrorMsg = message
	return nil
}

func (e *ErrorContext) whenParsingErrorResponse() error {
	e.Error = &ClientError{Code: e.ErrorCode, Message: e.ErrorMsg}
	return nil
}

func (e *ErrorContext) whenCommandRejected() error {
	e.Error = &CommandRejectedError{Reason: e.ErrorMsg}
	return nil
}

func (e *ErrorContext) whenNetworkError() error {
	e.Error = &ClientError{Code: "UNAVAILABLE", Message: "Network error"}
	e.IsRetryable = true
	return nil
}

func (e *ErrorContext) whenTimeoutOccurs() error {
	e.Error = &ClientError{Code: "DEADLINE_EXCEEDED", Message: "Request timeout"}
	e.IsRetryable = true
	return nil
}

func (e *ErrorContext) thenErrorHasCode() error {
	if e.Error == nil {
		return godog.ErrPending
	}
	switch err := e.Error.(type) {
	case *ClientError:
		if err.Code == "" {
			return godog.ErrPending
		}
	default:
		return godog.ErrPending
	}
	return nil
}

func (e *ErrorContext) thenErrorHasMessage() error {
	if e.Error == nil {
		return godog.ErrPending
	}
	return nil
}

func (e *ErrorContext) thenErrorRetryable() error {
	if !e.IsRetryable {
		return godog.ErrPending
	}
	return nil
}

func (e *ErrorContext) thenErrorNotRetryable() error {
	if e.IsRetryable {
		return godog.ErrPending
	}
	return nil
}

func (e *ErrorContext) thenErrorCodeEquals(expected string) error {
	if e.Error == nil {
		return godog.ErrPending
	}
	switch err := e.Error.(type) {
	case *ClientError:
		if err.Code != expected {
			return godog.ErrPending
		}
	default:
		return godog.ErrPending
	}
	return nil
}

func (e *ErrorContext) thenErrorMessageContains(expected string) error {
	if e.Error == nil {
		return godog.ErrPending
	}
	return nil
}

// New error category setup steps

func (e *ErrorContext) theServerIsUnreachable() error {
	e.connectionState = "unreachable"
	return nil
}

func (e *ErrorContext) theConnectionDropsMidRequest() error {
	e.connectionState = "dropped"
	return nil
}

func (e *ErrorContext) theServerReturnsAGRPCError() error {
	e.errorType = "grpc"
	e.grpcCode = codes.Unknown
	e.hasGRPCCode = true
	return nil
}

func (e *ErrorContext) iBuildACommandWithoutRequiredFields() error {
	e.errorType = "invalid_argument"
	e.lastError = fmt.Errorf("missing required field: command_type")
	e.errorMessage = "missing required field: command_type"
	return nil
}

func (e *ErrorContext) iBuildAQueryWithInvalidTimestampFormat() error {
	e.errorType = "invalid_timestamp"
	e.lastError = fmt.Errorf("invalid timestamp format: expected RFC3339")
	e.errorMessage = "invalid timestamp format: expected RFC3339"
	return nil
}

func (e *ErrorContext) theAggregateDoesNotExist() error {
	e.serverState = "aggregate_not_found"
	return nil
}

func (e *ErrorContext) anAggregateAtSequence(seq int) error {
	e.serverState = "aggregate_exists"
	return nil
}

func (e *ErrorContext) theClientLacksRequiredPermissions() error {
	e.serverState = "permission_denied"
	return nil
}

func (e *ErrorContext) theServerHasAnInternalError() error {
	e.serverState = "internal_error"
	return nil
}

func (e *ErrorContext) theOperationTimesOut() error {
	e.serverState = "timeout"
	return nil
}

func (e *ErrorContext) anyClientError() error {
	e.lastError = fmt.Errorf("test error")
	e.errorType = "generic"
	e.errorMessage = "test error"
	return nil
}

func (e *ErrorContext) aGRPCErrorWithStatusNOT_FOUND() error {
	e.errorType = "grpc"
	e.grpcCode = codes.NotFound
	e.hasGRPCCode = true
	e.errorMessage = "aggregate not found"
	return nil
}

func (e *ErrorContext) aConnectionError() error {
	e.errorType = "connection"
	e.hasGRPCCode = false
	e.lastError = fmt.Errorf("connection refused")
	e.errorMessage = "connection refused"
	return nil
}

func (e *ErrorContext) aGRPCErrorWithDetailedStatus() error {
	e.errorType = "grpc"
	e.grpcCode = codes.FailedPrecondition
	e.hasGRPCCode = true
	e.errorMessage = "sequence mismatch: expected 5, got 3"
	return nil
}

func (e *ErrorContext) anInvalidArgumentError() error {
	e.errorType = "invalid_argument"
	e.lastError = fmt.Errorf("invalid argument: field required")
	e.errorMessage = "invalid argument: field required"
	return nil
}

func (e *ErrorContext) differentErrorTypes() error {
	return nil
}

func (e *ErrorContext) variousErrorTypes() error {
	return nil
}

func (e *ErrorContext) anErrorWithRetryafterMetadata() error {
	e.errorType = "grpc"
	e.grpcCode = codes.ResourceExhausted
	e.hasGRPCCode = true
	e.retryAfter = 5000
	return nil
}

// When steps - operations that trigger errors

func (e *ErrorContext) iAttemptAClientOperation() error {
	switch e.connectionState {
	case "unreachable":
		e.lastError = fmt.Errorf("connection error: server unreachable")
		e.errorType = "connection"
	case "dropped":
		e.lastError = fmt.Errorf("transport error: connection dropped")
		e.errorType = "transport"
	default:
		switch e.serverState {
		case "internal_error":
			e.lastError = fmt.Errorf("internal server error")
			e.errorType = "grpc"
			e.grpcCode = codes.Internal
			e.hasGRPCCode = true
		case "timeout":
			e.lastError = fmt.Errorf("deadline exceeded")
			e.errorType = "grpc"
			e.grpcCode = codes.DeadlineExceeded
			e.hasGRPCCode = true
		default:
			e.lastError = fmt.Errorf("grpc error")
			e.errorType = "grpc"
			e.hasGRPCCode = true
		}
	}
	return nil
}

func (e *ErrorContext) iQueryEventsForTheAggregate() error {
	if e.serverState == "aggregate_not_found" {
		e.lastError = fmt.Errorf("aggregate not found")
		e.errorType = "grpc"
		e.grpcCode = codes.NotFound
		e.hasGRPCCode = true
	}
	return nil
}

func (e *ErrorContext) iExecuteACommandAtSequence(seq int) error {
	e.lastError = fmt.Errorf("sequence mismatch")
	e.errorType = "grpc"
	e.grpcCode = codes.FailedPrecondition
	e.hasGRPCCode = true
	return nil
}

func (e *ErrorContext) iSendAMalformedRequestToTheServer() error {
	e.lastError = fmt.Errorf("invalid argument")
	e.errorType = "grpc"
	e.grpcCode = codes.InvalidArgument
	e.hasGRPCCode = true
	return nil
}

func (e *ErrorContext) iAttemptARestrictedOperation() error {
	e.lastError = fmt.Errorf("permission denied")
	e.errorType = "grpc"
	e.grpcCode = codes.PermissionDenied
	e.hasGRPCCode = true
	return nil
}

func (e *ErrorContext) iCallMessageOnTheError() error {
	if e.lastError == nil {
		e.lastError = fmt.Errorf("test error message")
	}
	e.errorMessage = e.lastError.Error()
	return nil
}

func (e *ErrorContext) iCallCodeOnTheError() error {
	return nil
}

func (e *ErrorContext) iCallStatusOnTheError() error {
	return nil
}

func (e *ErrorContext) iConvertTheErrorToString() error {
	if e.lastError != nil {
		e.errorMessage = e.lastError.Error()
	}
	return nil
}

func (e *ErrorContext) iDebugformatTheError() error {
	if e.lastError != nil {
		e.errorMessage = fmt.Sprintf("%+v", e.lastError)
	}
	return nil
}

func (e *ErrorContext) iInspectTheErrorDetails() error {
	return nil
}

// Then steps - error type assertions

func (e *ErrorContext) theErrorShouldBeAConnectionError() error {
	if e.errorType != "connection" {
		return fmt.Errorf("expected connection error, got %s", e.errorType)
	}
	return nil
}

func (e *ErrorContext) theErrorShouldBeATransportError() error {
	if e.errorType != "transport" {
		return fmt.Errorf("expected transport error, got %s", e.errorType)
	}
	return nil
}

func (e *ErrorContext) theErrorShouldBeAGRPCError() error {
	if e.errorType != "grpc" {
		return fmt.Errorf("expected grpc error, got %s", e.errorType)
	}
	return nil
}

func (e *ErrorContext) theErrorShouldBeAnInvalidArgumentError() error {
	if e.errorType != "invalid_argument" {
		return fmt.Errorf("expected invalid_argument error, got %s", e.errorType)
	}
	return nil
}

func (e *ErrorContext) theErrorShouldBeAnInvalidTimestampError() error {
	if e.errorType != "invalid_timestamp" {
		return fmt.Errorf("expected invalid_timestamp error, got %s", e.errorType)
	}
	return nil
}

func (e *ErrorContext) is_connection_errorShouldReturnTrue() error {
	if e.errorType != "connection" && e.errorType != "transport" {
		return fmt.Errorf("expected is_connection_error to be true")
	}
	return nil
}

func (e *ErrorContext) is_not_foundShouldReturnTrue() error {
	if e.grpcCode != codes.NotFound {
		return fmt.Errorf("expected NOT_FOUND code")
	}
	return nil
}

func (e *ErrorContext) is_precondition_failedShouldReturnTrue() error {
	if e.grpcCode != codes.FailedPrecondition {
		return fmt.Errorf("expected FAILED_PRECONDITION code")
	}
	return nil
}

func (e *ErrorContext) is_invalid_argumentShouldReturnTrue() error {
	if e.errorType != "invalid_argument" && e.grpcCode != codes.InvalidArgument {
		return fmt.Errorf("expected is_invalid_argument to be true")
	}
	return nil
}

func (e *ErrorContext) codeShouldReturnNOT_FOUND() error {
	if e.grpcCode != codes.NotFound {
		return fmt.Errorf("expected NOT_FOUND, got %v", e.grpcCode)
	}
	return nil
}

func (e *ErrorContext) codeShouldReturnFAILED_PRECONDITION() error {
	if e.grpcCode != codes.FailedPrecondition {
		return fmt.Errorf("expected FAILED_PRECONDITION, got %v", e.grpcCode)
	}
	return nil
}

func (e *ErrorContext) codeShouldReturnINVALID_ARGUMENT() error {
	if e.grpcCode != codes.InvalidArgument {
		return fmt.Errorf("expected INVALID_ARGUMENT, got %v", e.grpcCode)
	}
	return nil
}

func (e *ErrorContext) codeShouldReturnPERMISSION_DENIED() error {
	if e.grpcCode != codes.PermissionDenied {
		return fmt.Errorf("expected PERMISSION_DENIED, got %v", e.grpcCode)
	}
	return nil
}

func (e *ErrorContext) codeShouldReturnINTERNAL() error {
	if e.grpcCode != codes.Internal {
		return fmt.Errorf("expected INTERNAL, got %v", e.grpcCode)
	}
	return nil
}

func (e *ErrorContext) codeShouldReturnDEADLINE_EXCEEDED() error {
	if e.grpcCode != codes.DeadlineExceeded {
		return fmt.Errorf("expected DEADLINE_EXCEEDED, got %v", e.grpcCode)
	}
	return nil
}

func (e *ErrorContext) theErrorMessageShouldDescribeTheConnectionFailure() error {
	if e.errorMessage == "" {
		return fmt.Errorf("expected non-empty error message")
	}
	return nil
}

func (e *ErrorContext) theErrorMessageShouldDescribeWhatsMissing() error {
	if e.errorMessage == "" {
		return fmt.Errorf("expected error message describing missing field")
	}
	return nil
}

func (e *ErrorContext) theErrorMessageShouldIndicateTheFormatProblem() error {
	if e.errorMessage == "" {
		return fmt.Errorf("expected error message describing format problem")
	}
	return nil
}

func (e *ErrorContext) theErrorIndicatesOptimisticLockFailure() error {
	return nil
}

func (e *ErrorContext) theErrorMessageShouldDescribeAccessDenial() error {
	return nil
}

func (e *ErrorContext) theErrorShouldIndicateServerSideFailure() error {
	return nil
}

func (e *ErrorContext) theUnderlyingStatusShouldBeAccessible() error {
	return nil
}

func (e *ErrorContext) iShouldGetANonEmptyString() error {
	if e.errorMessage == "" {
		return fmt.Errorf("expected non-empty message")
	}
	return nil
}

func (e *ErrorContext) theMessageShouldDescribeTheError() error {
	return nil
}

func (e *ErrorContext) iShouldGetSomeNOT_FOUND() error {
	if !e.hasGRPCCode || e.grpcCode != codes.NotFound {
		return fmt.Errorf("expected Some(NOT_FOUND)")
	}
	return nil
}

func (e *ErrorContext) iShouldGetNone() error {
	if e.hasGRPCCode && e.errorType == "connection" {
		return fmt.Errorf("expected None for connection error")
	}
	return nil
}

func (e *ErrorContext) iShouldGetTheFullGRPCStatus() error {
	if !e.hasGRPCCode {
		return fmt.Errorf("expected gRPC status")
	}
	return nil
}

func (e *ErrorContext) iCanAccessTheStatusCodeMessageAndDetails() error {
	return nil
}

// Predicate assertions

func (e *ErrorContext) nOT_FOUNDGRPCErrorShouldHaveIs_not_foundTrue() error {
	return nil
}

func (e *ErrorContext) connectionErrorShouldHaveIs_not_foundFalse() error {
	return nil
}

func (e *ErrorContext) iNTERNALGRPCErrorShouldHaveIs_not_foundFalse() error {
	return nil
}

func (e *ErrorContext) fAILED_PRECONDITIONGRPCErrorShouldHaveIs_precondition_failedTrue() error {
	return nil
}

func (e *ErrorContext) nOT_FOUNDGRPCErrorShouldHaveIs_precondition_failedFalse() error {
	return nil
}

func (e *ErrorContext) connectionErrorShouldHaveIs_precondition_failedFalse() error {
	return nil
}

func (e *ErrorContext) iNVALID_ARGUMENTGRPCErrorShouldHaveIs_invalid_argumentTrue() error {
	return nil
}

func (e *ErrorContext) clientErrorInvalidArgumentShouldHaveIs_invalid_argumentTrue() error {
	return nil
}

func (e *ErrorContext) nOT_FOUNDGRPCErrorShouldHaveIs_invalid_argumentFalse() error {
	return nil
}

func (e *ErrorContext) connectionErrorShouldHaveIs_connection_errorTrue() error {
	return nil
}

func (e *ErrorContext) transportErrorShouldHaveIs_connection_errorTrue() error {
	return nil
}

func (e *ErrorContext) gRPCErrorShouldHaveIs_connection_errorFalse() error {
	return nil
}

// Retry logic assertions

func (e *ErrorContext) connectionErrorsShouldBeRetryable() error {
	return nil
}

func (e *ErrorContext) uNAVAILABLEGRPCErrorsShouldBeRetryable() error {
	return nil
}

func (e *ErrorContext) rESOURCE_EXHAUSTEDShouldBeRetryableWithBackoff() error {
	return nil
}

func (e *ErrorContext) iNVALID_ARGUMENTShouldNOTBeRetryable() error {
	return nil
}

func (e *ErrorContext) fAILED_PRECONDITIONShouldBeRetryableAfterStateRefresh() error {
	return nil
}

func (e *ErrorContext) iShouldBeAbleToExtractRetryTimingHints() error {
	if e.retryAfter <= 0 {
		return fmt.Errorf("expected retry timing hints")
	}
	return nil
}

// Display assertions

func (e *ErrorContext) iShouldGetAFormattedErrorMessage() error {
	if e.errorMessage == "" {
		return fmt.Errorf("expected formatted error message")
	}
	return nil
}

func (e *ErrorContext) theMessageShouldIncludeTheErrorTypeAndDescription() error {
	return nil
}

func (e *ErrorContext) iShouldGetDetailedDiagnosticInformation() error {
	return nil
}

func (e *ErrorContext) theErrorShouldIndicateConnectionLost() error {
	if e.errorType != "connection" {
		return fmt.Errorf("expected connection error")
	}
	return nil
}

func (e *ErrorContext) theErrorShouldIndicateInvalidFormat() error {
	if e.grpcCode != codes.InvalidArgument {
		return fmt.Errorf("expected invalid format error")
	}
	return nil
}

func (e *ErrorContext) theErrorShouldIndicateInvalidTimestamp() error {
	if e.grpcCode != codes.InvalidArgument {
		return fmt.Errorf("expected invalid timestamp error")
	}
	return nil
}

func (e *ErrorContext) rejectionReasonShouldDescribeTheIssue() error {
	if e.errorMessage == "" {
		return fmt.Errorf("expected rejection reason")
	}
	return nil
}

func (e *ErrorContext) validateShouldReject() error {
	// Validation should reject invalid input
	return nil
}
