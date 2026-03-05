package dev.angzarr.client.steps;

/**
 * Shared test context for Cucumber step definitions.
 *
 * This class holds state that needs to be shared across multiple
 * step definition files within a single scenario. Cucumber's PicoContainer
 * DI automatically creates and injects a single instance per scenario.
 */
public class SharedTestContext {

    // Payload corruption state - shared between EventDecodingSteps and StateBuildingSteps
    public boolean payloadCorrupted = false;
    public byte[] payloadBytes = null;

    // Error state - shared across multiple step files
    public String errorMessage = null;
    public boolean errorRaised = false;

    /**
     * Reset all shared state. Called automatically by Cucumber before each scenario.
     */
    public void reset() {
        payloadCorrupted = false;
        payloadBytes = null;
        errorMessage = null;
        errorRaised = false;
    }
}
