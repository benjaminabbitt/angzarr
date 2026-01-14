package dev.angzarr.examples.saga;

import dev.angzarr.CommandBook;
import dev.angzarr.EventBook;

import java.util.List;

/**
 * Interface for loyalty points saga.
 */
public interface LoyaltySaga {

    /**
     * Processes events and generates commands to award loyalty points.
     *
     * @param eventBook the event history
     * @return list of command books to dispatch
     */
    List<CommandBook> processEvents(EventBook eventBook);
}
