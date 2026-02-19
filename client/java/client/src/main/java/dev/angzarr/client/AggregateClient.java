package dev.angzarr.client;

import dev.angzarr.CommandBook;
import dev.angzarr.CommandResponse;
import dev.angzarr.SpeculateAggregateRequest;
import dev.angzarr.SyncCommandBook;
import dev.angzarr.AggregateCoordinatorServiceGrpc;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.StatusRuntimeException;

import java.util.UUID;
import java.util.concurrent.TimeUnit;

/**
 * Client for sending commands to aggregates through the coordinator.
 *
 * <p>AggregateClient handles command routing, response parsing, and provides
 * multiple execution modes:
 * <ul>
 *   <li>Async (fire-and-forget) - for high-throughput scenarios</li>
 *   <li>Sync - wait for persistence, receive resulting events</li>
 *   <li>Speculative - what-if execution without persistence</li>
 * </ul>
 *
 * <p>Usage:
 * <pre>{@code
 * AggregateClient client = AggregateClient.connect("localhost:1310");
 * try {
 *     CommandBook command = buildCommand();
 *     CommandResponse response = client.handle(command);
 * } finally {
 *     client.close();
 * }
 * }</pre>
 */
public class AggregateClient implements AutoCloseable {

    private final AggregateCoordinatorServiceGrpc.AggregateCoordinatorServiceBlockingStub stub;
    private final ManagedChannel channel;
    private final boolean ownsChannel;

    private AggregateClient(ManagedChannel channel, boolean ownsChannel) {
        this.channel = channel;
        this.ownsChannel = ownsChannel;
        this.stub = AggregateCoordinatorServiceGrpc.newBlockingStub(channel);
    }

    /**
     * Connect to an aggregate coordinator at the given endpoint.
     *
     * @param endpoint The server endpoint (host:port or unix:///path for UDS)
     * @return A new AggregateClient
     * @throws Errors.ConnectionError if connection fails
     */
    public static AggregateClient connect(String endpoint) {
        try {
            ManagedChannel channel = ManagedChannelBuilder
                .forTarget(formatEndpoint(endpoint))
                .usePlaintext()
                .build();
            return new AggregateClient(channel, true);
        } catch (Exception e) {
            throw new Errors.ConnectionError("Failed to connect to " + endpoint, e);
        }
    }

    /**
     * Connect using an environment variable with fallback.
     *
     * @param envVar The environment variable name
     * @param defaultEndpoint Fallback endpoint if env var is not set
     * @return A new AggregateClient
     */
    public static AggregateClient fromEnv(String envVar, String defaultEndpoint) {
        String endpoint = System.getenv(envVar);
        if (endpoint == null || endpoint.isEmpty()) {
            endpoint = defaultEndpoint;
        }
        return connect(endpoint);
    }

    /**
     * Create a client from an existing channel.
     *
     * @param channel The gRPC channel to use
     * @return A new AggregateClient that does not own the channel
     */
    public static AggregateClient fromChannel(ManagedChannel channel) {
        return new AggregateClient(channel, false);
    }

    /**
     * Execute a command asynchronously (fire-and-forget).
     *
     * <p>Returns immediately after the coordinator accepts the command.
     * The command is guaranteed to be processed, but the client doesn't wait.
     *
     * @param command The command to execute
     * @return The command response
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public CommandResponse handle(CommandBook command) {
        try {
            return stub.handle(command);
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Execute a command synchronously.
     *
     * <p>Blocks until the aggregate processes the command and events are persisted.
     * The response includes the resulting events.
     *
     * @param command The sync command to execute
     * @return The command response with resulting events
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public CommandResponse handleSync(SyncCommandBook command) {
        try {
            return stub.handleSync(command);
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Execute a command speculatively against temporal state (no persistence).
     *
     * <p>Use for:
     * <ul>
     *   <li>Form validation - "Will this order succeed?"</li>
     *   <li>Preview - "What events would this produce?"</li>
     *   <li>Testing - verify business logic without polluting event store</li>
     * </ul>
     *
     * @param request The speculative execution request
     * @return The command response with projected events
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public CommandResponse handleSyncSpeculative(SpeculateAggregateRequest request) {
        try {
            return stub.handleSyncSpeculative(request);
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Start building a command for the given domain and root.
     *
     * @param domain The aggregate domain
     * @param root The aggregate root UUID
     * @return A CommandBuilder for fluent construction
     */
    public CommandBuilder command(String domain, UUID root) {
        return new CommandBuilder(this, domain, root);
    }

    /**
     * Start building a command for a new aggregate (no root yet).
     *
     * @param domain The aggregate domain
     * @return A CommandBuilder for fluent construction
     */
    public CommandBuilder commandNew(String domain) {
        return new CommandBuilder(this, domain);
    }

    /**
     * Close the underlying channel if this client owns it.
     */
    @Override
    public void close() {
        if (ownsChannel && channel != null) {
            try {
                channel.shutdown().awaitTermination(5, TimeUnit.SECONDS);
            } catch (InterruptedException e) {
                channel.shutdownNow();
                Thread.currentThread().interrupt();
            }
        }
    }

    private static String formatEndpoint(String endpoint) {
        if (endpoint.startsWith("/") || endpoint.startsWith("./")) {
            return "unix://" + endpoint;
        }
        if (endpoint.startsWith("unix://")) {
            return endpoint;
        }
        return endpoint;
    }
}
