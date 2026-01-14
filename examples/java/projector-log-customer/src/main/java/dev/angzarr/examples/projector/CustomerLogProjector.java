package dev.angzarr.examples.projector;

import dev.angzarr.EventBook;

/**
 * Interface for customer event logging projector.
 */
public interface CustomerLogProjector {
    void logEvents(EventBook eventBook);
}
