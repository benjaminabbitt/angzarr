package dev.angzarr.client;

import com.google.protobuf.Timestamp;
import dev.angzarr.Cover;
import dev.angzarr.Edition;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.Query;
import dev.angzarr.SequenceRange;
import dev.angzarr.TemporalQuery;

import java.time.Instant;
import java.time.format.DateTimeParseException;
import java.util.List;
import java.util.UUID;

/**
 * Fluent builder for constructing and executing event queries.
 *
 * <p>QueryBuilder supports multiple access patterns:
 * <ul>
 *   <li>By root - fetch all events for a specific aggregate</li>
 *   <li>By correlation ID - fetch events across aggregates in a workflow</li>
 *   <li>By sequence range - fetch specific event windows for pagination</li>
 *   <li>By temporal point - reconstruct historical state (as-of queries)</li>
 *   <li>By edition - query from specific schema versions</li>
 * </ul>
 *
 * <p>Usage:
 * <pre>{@code
 * EventBook events = client.query("orders", orderId)
 *     .range(10)
 *     .getEventBook();
 *
 * // Or temporal query
 * EventBook historical = client.query("orders", orderId)
 *     .asOfSequence(42)
 *     .getEventBook();
 * }</pre>
 */
public class QueryBuilder {

    private final QueryClient client;
    private final String domain;
    private UUID root;
    private String correlationId;
    private SequenceRange rangeSelect;
    private TemporalQuery temporal;
    private String edition;
    private RuntimeException err;

    /**
     * Create a query builder for a specific aggregate.
     *
     * @param client The query client to use
     * @param domain The aggregate domain
     * @param root The aggregate root UUID
     */
    public QueryBuilder(QueryClient client, String domain, UUID root) {
        this.client = client;
        this.domain = domain;
        this.root = root;
    }

    /**
     * Create a query builder by domain only (use with byCorrelationId).
     *
     * @param client The query client to use
     * @param domain The aggregate domain
     */
    public QueryBuilder(QueryClient client, String domain) {
        this.client = client;
        this.domain = domain;
        this.root = null;
    }

    /**
     * Query by correlation ID instead of root.
     *
     * <p>Correlation IDs link events across aggregates in a distributed workflow.
     *
     * @param id The correlation ID
     * @return This builder for chaining
     */
    public QueryBuilder byCorrelationId(String id) {
        this.correlationId = id;
        this.root = null;
        return this;
    }

    /**
     * Query events from a specific edition.
     *
     * <p>After upcasting (event schema migration), events exist in multiple editions.
     *
     * @param edition The edition name
     * @return This builder for chaining
     */
    public QueryBuilder withEdition(String edition) {
        this.edition = edition;
        return this;
    }

    /**
     * Query a range of sequences from lower (inclusive).
     *
     * <p>Use for incremental sync: "give me events since sequence N"
     *
     * @param lower The lower bound (inclusive)
     * @return This builder for chaining
     */
    public QueryBuilder range(int lower) {
        this.rangeSelect = SequenceRange.newBuilder()
            .setLower(lower)
            .build();
        return this;
    }

    /**
     * Query a range of sequences with upper bound (inclusive).
     *
     * <p>Use for pagination: fetch events 100-200, then 200-300.
     *
     * @param lower The lower bound (inclusive)
     * @param upper The upper bound (inclusive)
     * @return This builder for chaining
     */
    public QueryBuilder rangeTo(int lower, int upper) {
        this.rangeSelect = SequenceRange.newBuilder()
            .setLower(lower)
            .setUpper(upper)
            .build();
        return this;
    }

    /**
     * Query state as of a specific sequence number.
     *
     * <p>Essential for debugging: "What was the state when this bug occurred?"
     *
     * @param seq The sequence number
     * @return This builder for chaining
     */
    public QueryBuilder asOfSequence(int seq) {
        this.temporal = TemporalQuery.newBuilder()
            .setAsOfSequence(seq)
            .build();
        return this;
    }

    /**
     * Query state as of a specific timestamp (RFC3339 format).
     *
     * <p>Example: "2024-01-15T10:30:00Z"
     *
     * @param rfc3339 The timestamp in RFC3339 format
     * @return This builder for chaining
     */
    public QueryBuilder asOfTime(String rfc3339) {
        try {
            Instant instant = Instant.parse(rfc3339);
            Timestamp ts = Timestamp.newBuilder()
                .setSeconds(instant.getEpochSecond())
                .setNanos(instant.getNano())
                .build();
            this.temporal = TemporalQuery.newBuilder()
                .setAsOfTime(ts)
                .build();
        } catch (DateTimeParseException e) {
            this.err = new Errors.InvalidTimestampError("Failed to parse timestamp: " + rfc3339);
        }
        return this;
    }

    /**
     * Build the Query without executing.
     *
     * @return The constructed Query
     * @throws Errors.InvalidTimestampError if timestamp parsing failed
     */
    public Query build() {
        if (err != null) {
            throw err;
        }

        Cover.Builder coverBuilder = Cover.newBuilder()
            .setDomain(domain);

        if (correlationId != null && !correlationId.isEmpty()) {
            coverBuilder.setCorrelationId(correlationId);
        }
        if (root != null) {
            coverBuilder.setRoot(Helpers.uuidToProto(root));
        }
        if (edition != null && !edition.isEmpty()) {
            coverBuilder.setEdition(Edition.newBuilder().setName(edition).build());
        }

        Query.Builder queryBuilder = Query.newBuilder()
            .setCover(coverBuilder.build());

        if (rangeSelect != null) {
            queryBuilder.setRange(rangeSelect);
        } else if (temporal != null) {
            queryBuilder.setTemporal(temporal);
        }

        return queryBuilder.build();
    }

    /**
     * Execute the query and return a single EventBook.
     *
     * @return The EventBook containing matching events
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public EventBook getEventBook() {
        Query query = build();
        return client.getEventBook(query);
    }

    /**
     * Execute the query and return all matching EventBooks.
     *
     * @return List of EventBooks
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public List<EventBook> getEvents() {
        Query query = build();
        return client.getEvents(query);
    }

    /**
     * Execute the query and return just the event pages.
     *
     * <p>Convenience method when you only need events, not metadata.
     *
     * @return List of EventPages
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public List<EventPage> getPages() {
        EventBook book = getEventBook();
        return book.getPagesList();
    }
}
