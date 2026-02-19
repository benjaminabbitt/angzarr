package dev.angzarr.client;

import dev.angzarr.CommandBook;
import dev.angzarr.CommandResponse;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;

import java.util.UUID;
import java.util.concurrent.TimeUnit;

/**
 * Combined client for aggregate commands and event queries.
 *
 * <p>DomainClient combines QueryClient and AggregateClient into a single unified
 * interface. This is the recommended entry point for most applications because:
 * <ul>
 *   <li>Single connection - one endpoint, one channel, reduced resource usage</li>
 *   <li>Unified API - both queries and commands through one object</li>
 *   <li>Builder access - fluent builders attached to the client instance</li>
 *   <li>Simpler DI - inject one client instead of two</li>
 * </ul>
 *
 * <p>For advanced use cases (separate scaling, different endpoints), use
 * QueryClient and AggregateClient directly.
 *
 * <p>Usage:
 * <pre>{@code
 * DomainClient client = DomainClient.connect("localhost:1310");
 * try {
 *     // Send a command
 *     CommandResponse response = client.command("orders", orderId)
 *         .withCommand(typeUrl, createOrderCmd)
 *         .execute();
 *
 *     // Query events
 *     EventBook events = client.query("orders", orderId)
 *         .getEventBook();
 * } finally {
 *     client.close();
 * }
 * }</pre>
 */
public class DomainClient implements AutoCloseable {

    private final AggregateClient aggregate;
    private final QueryClient query;
    private final ManagedChannel channel;

    private DomainClient(ManagedChannel channel, AggregateClient aggregate, QueryClient query) {
        this.channel = channel;
        this.aggregate = aggregate;
        this.query = query;
    }

    /**
     * Connect to a domain's coordinator at the given endpoint.
     *
     * @param endpoint The server endpoint (host:port or unix:///path for UDS)
     * @return A new DomainClient
     * @throws Errors.ConnectionError if connection fails
     */
    public static DomainClient connect(String endpoint) {
        try {
            ManagedChannel channel = ManagedChannelBuilder
                .forTarget(formatEndpoint(endpoint))
                .usePlaintext()
                .build();
            return new DomainClient(
                channel,
                AggregateClient.fromChannel(channel),
                QueryClient.fromChannel(channel)
            );
        } catch (Exception e) {
            throw new Errors.ConnectionError("Failed to connect to " + endpoint, e);
        }
    }

    /**
     * Connect using an environment variable with fallback.
     *
     * @param envVar The environment variable name
     * @param defaultEndpoint Fallback endpoint if env var is not set
     * @return A new DomainClient
     */
    public static DomainClient fromEnv(String envVar, String defaultEndpoint) {
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
     * @return A new DomainClient
     */
    public static DomainClient fromChannel(ManagedChannel channel) {
        return new DomainClient(
            null, // Don't own the channel
            AggregateClient.fromChannel(channel),
            QueryClient.fromChannel(channel)
        );
    }

    /**
     * Get the aggregate client for direct access.
     *
     * @return The underlying AggregateClient
     */
    public AggregateClient getAggregate() {
        return aggregate;
    }

    /**
     * Get the query client for direct access.
     *
     * @return The underlying QueryClient
     */
    public QueryClient getQuery() {
        return query;
    }

    /**
     * Execute a command (convenience method delegating to aggregate).
     *
     * @param command The command to execute
     * @return The command response
     */
    public CommandResponse execute(CommandBook command) {
        return aggregate.handle(command);
    }

    /**
     * Start building a command for the given domain and root.
     *
     * @param domain The aggregate domain
     * @param root The aggregate root UUID
     * @return A CommandBuilder for fluent construction
     */
    public CommandBuilder command(String domain, UUID root) {
        return aggregate.command(domain, root);
    }

    /**
     * Start building a command for a new aggregate (no root yet).
     *
     * @param domain The aggregate domain
     * @return A CommandBuilder for fluent construction
     */
    public CommandBuilder commandNew(String domain) {
        return aggregate.commandNew(domain);
    }

    /**
     * Start building a query for the given domain and root.
     *
     * @param domain The aggregate domain
     * @param root The aggregate root UUID
     * @return A QueryBuilder for fluent construction
     */
    public QueryBuilder query(String domain, UUID root) {
        return query.query(domain, root);
    }

    /**
     * Start building a query by domain only (use with byCorrelationId).
     *
     * @param domain The aggregate domain
     * @return A QueryBuilder for fluent construction
     */
    public QueryBuilder queryDomain(String domain) {
        return query.queryDomain(domain);
    }

    /**
     * Close the underlying channel and clients.
     */
    @Override
    public void close() {
        // Close individual clients first (they won't close the channel since they don't own it)
        aggregate.close();
        query.close();

        // Then close the channel if we own it
        if (channel != null) {
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
