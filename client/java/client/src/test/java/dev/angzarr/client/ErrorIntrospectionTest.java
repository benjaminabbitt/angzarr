package dev.angzarr.client;

import io.grpc.Status;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Tests for error introspection methods.
 *
 * Error introspection allows callers to check the nature of errors
 * without type casting or exception handling boilerplate.
 */
class ErrorIntrospectionTest {

    // =========================================================================
    // isNotFound Tests
    // =========================================================================

    @Test
    void grpcError_with_NOT_FOUND_should_return_true_for_isNotFound() {
        var error = new Errors.GrpcError("not found", Status.Code.NOT_FOUND);
        assertThat(error.isNotFound()).isTrue();
    }

    @Test
    void grpcError_with_other_code_should_return_false_for_isNotFound() {
        var error = new Errors.GrpcError("internal error", Status.Code.INTERNAL);
        assertThat(error.isNotFound()).isFalse();
    }

    // =========================================================================
    // isPreconditionFailed Tests
    // =========================================================================

    @Test
    void grpcError_with_FAILED_PRECONDITION_should_return_true_for_isPreconditionFailed() {
        var error = new Errors.GrpcError("precondition failed", Status.Code.FAILED_PRECONDITION);
        assertThat(error.isPreconditionFailed()).isTrue();
    }

    @Test
    void grpcError_with_other_code_should_return_false_for_isPreconditionFailed() {
        var error = new Errors.GrpcError("internal error", Status.Code.INTERNAL);
        assertThat(error.isPreconditionFailed()).isFalse();
    }

    @Test
    void commandRejectedError_should_return_true_for_isPreconditionFailed() {
        // CommandRejectedError defaults to FAILED_PRECONDITION
        var error = new Errors.CommandRejectedError("rejected");
        assertThat(error.isPreconditionFailed()).isTrue();
    }

    // =========================================================================
    // isInvalidArgument Tests
    // =========================================================================

    @Test
    void grpcError_with_INVALID_ARGUMENT_should_return_true_for_isInvalidArgument() {
        var error = new Errors.GrpcError("invalid argument", Status.Code.INVALID_ARGUMENT);
        assertThat(error.isInvalidArgument()).isTrue();
    }

    @Test
    void invalidArgumentError_should_return_true_for_isInvalidArgument() {
        var error = new Errors.InvalidArgumentError("bad input");
        assertThat(error.isInvalidArgument()).isTrue();
    }

    @Test
    void grpcError_with_other_code_should_return_false_for_isInvalidArgument() {
        var error = new Errors.GrpcError("internal error", Status.Code.INTERNAL);
        assertThat(error.isInvalidArgument()).isFalse();
    }

    // =========================================================================
    // isConnectionError Tests
    // =========================================================================

    @Test
    void connectionError_should_return_true_for_isConnectionError() {
        var error = new Errors.ConnectionError("connection refused");
        assertThat(error.isConnectionError()).isTrue();
    }

    @Test
    void transportError_should_return_true_for_isConnectionError() {
        var error = new Errors.TransportError("transport failed");
        assertThat(error.isConnectionError()).isTrue();
    }

    @Test
    void grpcError_with_UNAVAILABLE_should_return_true_for_isConnectionError() {
        var error = new Errors.GrpcError("unavailable", Status.Code.UNAVAILABLE);
        assertThat(error.isConnectionError()).isTrue();
    }

    @Test
    void grpcError_with_other_code_should_return_false_for_isConnectionError() {
        var error = new Errors.GrpcError("internal error", Status.Code.INTERNAL);
        assertThat(error.isConnectionError()).isFalse();
    }

    // =========================================================================
    // Base Class Default Behavior Tests
    // =========================================================================

    @Test
    void clientError_should_have_default_false_for_all_introspection_methods() {
        // Direct ClientError instance should return false for all introspection
        var error = new Errors.ClientError("generic error");
        assertThat(error.isNotFound()).isFalse();
        assertThat(error.isPreconditionFailed()).isFalse();
        assertThat(error.isInvalidArgument()).isFalse();
        assertThat(error.isConnectionError()).isFalse();
    }
}
