package dev.angzarr.client;

import dev.angzarr.EventBook;
import dev.angzarr.Query;
import dev.angzarr.EventQueryServiceGrpc;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.StatusRuntimeException;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.TimeUnit;

/**
 * Client for querying events from the EventQueryService.
 *
 * <p>QueryClient provides read access to aggregate event streams for:
 * <ul>
 *   <li>State reconstruction - rebuild aggregate state from events</li>
 *   <li>Audit trails - read complete history for compliance</li>
 *   <li>Projections - feed events to read-model projectors</li>
 *   <li>Testing - verify events were persisted after commands</li>
 * </ul>
 *
 * <p>Usage:
 * <pre>{@code
 * QueryClient client = QueryClient.connect("localhost:1340");
 * try {
 *     Query query = Query.newBuilder()
 *         .setCover(Cover.newBuilder().setDomain("test").setRoot(rootUuid))
 *         .build();
 *     EventBook events = client.getEventBook(query);
 * } finally {
 *     client.close();
 * }
 * }</pre>
 */
public class QueryClient implements AutoCloseable {

    private final EventQueryServiceGrpc.EventQueryServiceBlockingStub stub;
    private final ManagedChannel channel;
    private final boolean ownsChannel;

    private QueryClient(ManagedChannel channel, boolean ownsChannel) {
        this.channel = channel;
        this.ownsChannel = ownsChannel;
        this.stub = EventQueryServiceGrpc.newBlockingStub(channel);
    }

    /**
     * Connect to an event query service at the given endpoint.
     *
     * @param endpoint The server endpoint (host:port or unix:///path for UDS)
     * @return A new QueryClient
     * @throws Errors.ConnectionError if connection fails
     */
    public static QueryClient connect(String endpoint) {
        try {
            ManagedChannel channel = ManagedChannelBuilder
                .forTarget(formatEndpoint(endpoint))
                .usePlaintext()
                .build();
            return new QueryClient(channel, true);
        } catch (Exception e) {
            throw new Errors.ConnectionError("Failed to connect to " + endpoint, e);
        }
    }

    /**
     * Connect using an environment variable with fallback.
     *
     * @param envVar The environment variable name
     * @param defaultEndpoint Fallback endpoint if env var is not set
     * @return A new QueryClient
     */
    public static QueryClient fromEnv(String envVar, String defaultEndpoint) {
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
     * @return A new QueryClient that does not own the channel
     */
    public static QueryClient fromChannel(ManagedChannel channel) {
        return new QueryClient(channel, false);
    }

    /**
     * Retrieve a single EventBook for the query.
     *
     * @param query The query specifying which events to retrieve
     * @return The EventBook containing matching events
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public EventBook getEventBook(Query query) {
        try {
            return stub.getEventBook(query);
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Retrieve all EventBooks matching the query.
     *
     * @param query The query specifying which events to retrieve
     * @return List of EventBooks
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public List<EventBook> getEvents(Query query) {
        try {
            Iterator<EventBook> iterator = stub.getEvents(query);
            List<EventBook> events = new ArrayList<>();
            while (iterator.hasNext()) {
                events.add(iterator.next());
            }
            return events;
        } catch (StatusRuntimeException e) {
            throw new Errors.GrpcError(e.getMessage(), e.getStatus().getCode());
        }
    }

    /**
     * Start building a query for the given domain and root.
     *
     * @param domain The aggregate domain
     * @param root The aggregate root UUID
     * @return A QueryBuilder for fluent construction
     */
    public QueryBuilder query(String domain, UUID root) {
        return new QueryBuilder(this, domain, root);
    }

    /**
     * Start building a query by domain only (use with byCorrelationId).
     *
     * @param domain The aggregate domain
     * @return A QueryBuilder for fluent construction
     */
    public QueryBuilder queryDomain(String domain) {
        return new QueryBuilder(this, domain);
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
