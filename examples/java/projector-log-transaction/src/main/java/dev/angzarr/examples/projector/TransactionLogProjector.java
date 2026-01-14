package dev.angzarr.examples.projector;

import dev.angzarr.EventBook;

/**
 * Interface for transaction event logging projector.
 */
public interface TransactionLogProjector {
    void logEvents(EventBook eventBook);
}
