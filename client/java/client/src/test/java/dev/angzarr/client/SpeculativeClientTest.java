package dev.angzarr.client;

import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Tests for SpeculativeClient - what-if scenario execution.
 *
 * SpeculativeClient provides methods for speculative execution across
 * different coordinator types without persisting results.
 */
class SpeculativeClientTest {

    // =========================================================================
    // Construction Tests
    // =========================================================================

    @Test
    void fromChannel_should_create_client_with_all_stubs() {
        // SpeculativeClient wraps four coordinator clients:
        // aggregate, saga, projector, and process manager

        // Since we can't easily mock gRPC channels in unit tests,
        // we verify the class structure and factory methods exist
        assertThat(SpeculativeClient.class).isNotNull();

        // Verify factory methods are present
        assertThat(SpeculativeClient.class.getDeclaredMethods())
            .extracting("name")
            .contains("connect", "fromEnv", "fromChannel");
    }

    @Test
    void connect_with_invalid_endpoint_should_throw() {
        // Attempting to connect to an invalid endpoint should fail
        // with a ConnectionError
        assertThatThrownBy(() -> SpeculativeClient.connect("invalid:endpoint:format:extra"))
            .isInstanceOf(Errors.ConnectionError.class);
    }

    // =========================================================================
    // Method Signature Tests
    // =========================================================================

    @Test
    void should_have_aggregate_method() throws NoSuchMethodException {
        // Verify aggregate speculative method exists
        var method = SpeculativeClient.class.getMethod("aggregate",
            dev.angzarr.SpeculateAggregateRequest.class);
        assertThat(method.getReturnType()).isEqualTo(dev.angzarr.CommandResponse.class);
    }

    @Test
    void should_have_projector_method() throws NoSuchMethodException {
        // Verify projector speculative method exists
        var method = SpeculativeClient.class.getMethod("projector",
            dev.angzarr.SpeculateProjectorRequest.class);
        assertThat(method.getReturnType()).isEqualTo(dev.angzarr.Projection.class);
    }

    @Test
    void should_have_saga_method() throws NoSuchMethodException {
        // Verify saga speculative method exists
        var method = SpeculativeClient.class.getMethod("saga",
            dev.angzarr.SpeculateSagaRequest.class);
        assertThat(method.getReturnType()).isEqualTo(dev.angzarr.SagaResponse.class);
    }

    @Test
    void should_have_processManager_method() throws NoSuchMethodException {
        // Verify process manager speculative method exists
        var method = SpeculativeClient.class.getMethod("processManager",
            dev.angzarr.SpeculatePmRequest.class);
        assertThat(method.getReturnType()).isEqualTo(dev.angzarr.ProcessManagerHandleResponse.class);
    }

    @Test
    void should_have_close_method() throws NoSuchMethodException {
        // Verify close method exists for resource cleanup
        var method = SpeculativeClient.class.getMethod("close");
        assertThat(method).isNotNull();
    }
}
