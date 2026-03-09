package dev.angzarr.client;

import com.google.protobuf.ByteString;
import dev.angzarr.*;
import dev.angzarr.EventPage;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Context for saga handlers, providing access to destination aggregate state.
 *
 * <p>Used in the splitter pattern where one event triggers commands to multiple aggregates.
 * Provides sequence number lookup for optimistic concurrency control.
 *
 * <p>Usage:
 * <pre>{@code
 * List<CommandBook> handleTableSettled(TableSettled event, SagaContext ctx) {
 *     List<CommandBook> commands = new ArrayList<>();
 *     for (var payout : event.getPayoutsList()) {
 *         long targetSeq = ctx.getSequence("player", payout.getPlayerRoot());
 *         // ... build CommandBook with sequence
 *     }
 *     return commands;
 * }
 * }</pre>
 */
public class SagaContext {
    private final Map<String, EventBook> destinations;

    /**
     * Create a context from a list of destination EventBooks.
     */
    public SagaContext(List<EventBook> destinationBooks) {
        this.destinations = new HashMap<>();
        for (EventBook book : destinationBooks) {
            if (book.hasCover() && !book.getCover().getDomain().isEmpty()) {
                String key = makeKey(book.getCover().getDomain(), book.getCover().getRoot().getValue());
                destinations.put(key, book);
            }
        }
    }

    /**
     * Get the next sequence number for a destination aggregate.
     * Returns 1 if the aggregate doesn't exist yet.
     */
    public long getSequence(String domain, ByteString aggregateRoot) {
        String key = makeKey(domain, aggregateRoot);
        EventBook book = destinations.get(key);
        if (book == null || book.getPagesList().isEmpty()) {
            return 1;
        }
        EventPage lastPage = book.getPagesList().get(book.getPagesList().size() - 1);
        if (lastPage.hasHeader()) {
            return lastPage.getHeader().getSequence() + 1;
        }
        return 1;
    }

    /**
     * Get the EventBook for a destination aggregate, if available.
     */
    public EventBook getDestination(String domain, ByteString aggregateRoot) {
        return destinations.get(makeKey(domain, aggregateRoot));
    }

    private static String makeKey(String domain, ByteString root) {
        return domain + ":" + root.toStringUtf8();
    }
}
