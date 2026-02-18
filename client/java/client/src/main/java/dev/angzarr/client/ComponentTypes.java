package dev.angzarr.client;

/**
 * Component type constants for descriptors.
 */
public final class ComponentTypes {
    public static final String AGGREGATE = "aggregate";
    public static final String SAGA = "saga";
    public static final String PROCESS_MANAGER = "process_manager";
    public static final String PROJECTOR = "projector";
    public static final String UPCASTER = "upcaster";

    private ComponentTypes() {
        // Prevent instantiation
    }
}
