package dev.angzarr.examples.projector;

import dev.angzarr.EventBook;
import dev.angzarr.Projection;

/**
 * Interface for receipt projection logic.
 */
public interface ReceiptProjector {

    /**
     * Projects an event book to a receipt if the transaction is completed.
     *
     * @param eventBook the event history
     * @return projection containing the receipt, or null if not applicable
     */
    Projection project(EventBook eventBook);
}
