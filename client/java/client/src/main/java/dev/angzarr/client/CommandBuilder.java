package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.Message;
import dev.angzarr.CommandBook;
import dev.angzarr.CommandPage;
import dev.angzarr.CommandResponse;
import dev.angzarr.Cover;

import java.util.UUID;

/**
 * Fluent builder for constructing and executing commands.
 *
 * <p>CommandBuilder reduces boilerplate when creating commands:
 * <ul>
 *   <li>Chain method calls instead of nested object construction</li>
 *   <li>Type-safe methods prevent invalid field combinations</li>
 *   <li>Auto-generates correlation IDs when not provided</li>
 *   <li>Build incrementally, execute when ready</li>
 * </ul>
 *
 * <p>Usage:
 * <pre>{@code
 * CommandResponse response = client.command("orders", orderId)
 *     .withCorrelationId("corr-123")
 *     .withSequence(5)
 *     .withCommand(typeUrl, createOrderCmd)
 *     .execute();
 * }</pre>
 */
public class CommandBuilder {

    private final AggregateClient client;
    private final String domain;
    private final UUID root;
    private String correlationId;
    private int sequence = 0;
    private String typeUrl;
    private byte[] payload;
    private RuntimeException err;

    /**
     * Create a command builder for an existing aggregate.
     *
     * @param client The aggregate client to use
     * @param domain The aggregate domain
     * @param root The aggregate root UUID
     */
    public CommandBuilder(AggregateClient client, String domain, UUID root) {
        this.client = client;
        this.domain = domain;
        this.root = root;
    }

    /**
     * Create a command builder for a new aggregate (no root yet).
     *
     * @param client The aggregate client to use
     * @param domain The aggregate domain
     */
    public CommandBuilder(AggregateClient client, String domain) {
        this.client = client;
        this.domain = domain;
        this.root = null;
    }

    /**
     * Set the correlation ID for request tracing.
     *
     * <p>Correlation IDs link related operations across services.
     * If not set, a UUID will be auto-generated on build.
     *
     * @param id The correlation ID
     * @return This builder for chaining
     */
    public CommandBuilder withCorrelationId(String id) {
        this.correlationId = id;
        return this;
    }

    /**
     * Set the expected sequence number for optimistic locking.
     *
     * <p>Defaults to 0 for new aggregates.
     *
     * @param seq The sequence number
     * @return This builder for chaining
     */
    public CommandBuilder withSequence(int seq) {
        this.sequence = seq;
        return this;
    }

    /**
     * Set the command type URL and message.
     *
     * @param typeUrl The fully-qualified type URL (e.g., "type.googleapis.com/orders.CreateOrder")
     * @param message The protobuf command message
     * @return This builder for chaining
     */
    public CommandBuilder withCommand(String typeUrl, Message message) {
        try {
            this.typeUrl = typeUrl;
            this.payload = message.toByteArray();
        } catch (Exception e) {
            this.err = new Errors.InvalidArgumentError("Failed to serialize command: " + e.getMessage());
        }
        return this;
    }

    /**
     * Build the CommandBook without executing.
     *
     * @return The constructed CommandBook
     * @throws Errors.InvalidArgumentError if required fields are missing
     */
    public CommandBook build() {
        if (err != null) {
            throw err;
        }
        if (typeUrl == null || typeUrl.isEmpty()) {
            throw new Errors.InvalidArgumentError("command type_url not set");
        }
        if (payload == null) {
            throw new Errors.InvalidArgumentError("command payload not set");
        }

        String corrId = correlationId;
        if (corrId == null || corrId.isEmpty()) {
            corrId = UUID.randomUUID().toString();
        }

        Cover.Builder coverBuilder = Cover.newBuilder()
            .setDomain(domain)
            .setCorrelationId(corrId);

        if (root != null) {
            coverBuilder.setRoot(Helpers.uuidToProto(root));
        }

        Any commandAny = Any.newBuilder()
            .setTypeUrl(typeUrl)
            .setValue(com.google.protobuf.ByteString.copyFrom(payload))
            .build();

        CommandPage page = CommandPage.newBuilder()
            .setSequence(sequence)
            .setCommand(commandAny)
            .build();

        return CommandBook.newBuilder()
            .setCover(coverBuilder.build())
            .addPages(page)
            .build();
    }

    /**
     * Build and execute the command.
     *
     * @return The command response
     * @throws Errors.InvalidArgumentError if required fields are missing
     * @throws Errors.GrpcError if the gRPC call fails
     */
    public CommandResponse execute() {
        CommandBook cmd = build();
        return client.handle(cmd);
    }
}
