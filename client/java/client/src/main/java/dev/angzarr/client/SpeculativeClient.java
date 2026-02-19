package dev.angzarr.client;

import dev.angzarr.*;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.StatusRuntimeException;

/**
 * Client for speculative (what-if) execution across coordinator types.
 *
 * <p>Speculative execution allows testing commands, events, and projections
 * without persisting results. Use this for:
 * <ul>
 *   <li>Form validation: "Will this order succeed?"</li>
 *   <li>Preview: "What events would this produce?"</li>
 *   <li>Testing: Verify business logic without polluting event store</li>
 * </ul>
 *
 * <p>Example:
 * <pre>{@code
 * var client = SpeculativeClient.connect("localhost:1310");
 * var response = client.aggregate(SpeculateAggregateRequest.newBuilder()
 *     .setCommand(command)
 *     .setEvents(priorEvents)
 *     .build());
 * }</pre>
 */
public class SpeculativeClient implements AutoCloseable {

    private final AggregateCoordinatorServiceGrpc.AggregateCoordinatorServiceBlockingStub aggregateStub;
    private final SagaCoordinatorServiceGrpc.SagaCoordinatorServiceBlockingStub sagaStub;
    private final ProjectorCoordinatorServiceGrpc.ProjectorCoordinatorServiceBlockingStub projectorStub;
    private final ProcessManagerCoordinatorServiceGrpc.ProcessManagerCoordinatorServiceBlockingStub pmStub;
    private final ManagedChannel channel;

    private SpeculativeClient(ManagedChannel channel) {
        this.channel = channel;
        this.aggregateStub = AggregateCoordinatorServiceGrpc.newBlockingStub(channel);
        this.sagaStub = SagaCoordinatorServiceGrpc.newBlockingStub(channel);
        this.projectorStub = ProjectorCoordinatorServiceGrpc.newBlockingStub(channel);
        this.pmStub = ProcessManagerCoordinatorServiceGrpc.newBlockingStub(channel);
    }

    /**
     * Connect to coordinator services at the given endpoint.
     *
     * @param endpoint the host:port to connect to
     * @return a connected SpeculativeClient
     * @throws Errors.ConnectionError if connection fails
     */
    public static SpeculativeClient connect(String endpoint) {
        try {
            String[] parts = endpoint.split(":");
            if (parts.length != 2) {
                throw new Errors.ConnectionError("Invalid endpoint format: " + endpoint);
            }
            ManagedChannel channel = ManagedChannelBuilder
                .forAddress(parts[0], Integer.parseInt(parts[1]))
                .usePlaintext()
                .build();
            return new SpeculativeClient(channel);
        } catch (NumberFormatException e) {
            throw new Errors.ConnectionError("Invalid port in endpoint: " + endpoint, e);
        }
    }

    /**
     * Connect using an endpoint from environment variable with fallback.
     *
     * @param envVar the environment variable name
     * @param defaultEndpoint the default endpoint if env var is not set
     * @return a connected SpeculativeClient
     */
    public static SpeculativeClient fromEnv(String envVar, String defaultEndpoint) {
        String endpoint = System.getenv(envVar);
        if (endpoint == null || endpoint.isEmpty()) {
            endpoint = defaultEndpoint;
        }
        return connect(endpoint);
    }

    /**
     * Create a client from an existing channel.
     *
     * @param channel the gRPC channel to use
     * @return a SpeculativeClient using the channel
     */
    public static SpeculativeClient fromChannel(ManagedChannel channel) {
        return new SpeculativeClient(channel);
    }

    /**
     * Execute a command speculatively against temporal aggregate state.
     *
     * @param request the speculative aggregate request
     * @return the command response (without persistence)
     * @throws Errors.GrpcError if the RPC fails
     */
    public CommandResponse aggregate(SpeculateAggregateRequest request) {
        try {
            return aggregateStub.handleSyncSpeculative(request);
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Execute a projector speculatively against events.
     *
     * @param request the speculative projector request
     * @return the projection result (without persistence)
     * @throws Errors.GrpcError if the RPC fails
     */
    public dev.angzarr.Projection projector(SpeculateProjectorRequest request) {
        try {
            return projectorStub.handleSpeculative(request);
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Execute a saga speculatively against events.
     *
     * @param request the speculative saga request
     * @return the saga response (without persistence)
     * @throws Errors.GrpcError if the RPC fails
     */
    public SagaResponse saga(SpeculateSagaRequest request) {
        try {
            return sagaStub.executeSpeculative(request);
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Execute a process manager speculatively.
     *
     * @param request the speculative PM request
     * @return the PM response (without persistence)
     * @throws Errors.GrpcError if the RPC fails
     */
    public ProcessManagerHandleResponse processManager(SpeculatePmRequest request) {
        try {
            return pmStub.handleSpeculative(request);
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Close the underlying channel.
     */
    @Override
    public void close() {
        if (channel != null && !channel.isShutdown()) {
            channel.shutdown();
        }
    }
}
