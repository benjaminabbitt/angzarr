package dev.angzarr.client.router;

import dev.angzarr.EventBook;
import dev.angzarr.Projection;

import java.util.List;

/**
 * Handler interface for a single domain's events in a projector.
 *
 * <p>Projectors consume events and produce external output (read models,
 * caches, external systems).
 *
 * <p>Example:
 * <pre>{@code
 * public class PlayerProjectorHandler implements ProjectorDomainHandler {
 *
 *     @Override
 *     public List<String> eventTypes() {
 *         return List.of("PlayerRegistered", "FundsDeposited");
 *     }
 *
 *     @Override
 *     public Projection project(EventBook events) throws ProjectionError {
 *         for (EventPage page : events.getPagesList()) {
 *             if (!page.hasEvent()) continue;
 *             String typeUrl = page.getEvent().getTypeUrl();
 *             if (typeUrl.endsWith("PlayerRegistered")) {
 *                 // Update read model
 *             } else if (typeUrl.endsWith("FundsDeposited")) {
 *                 // Update read model
 *             }
 *         }
 *         return Projection.getDefaultInstance();
 *     }
 * }
 * }</pre>
 */
public interface ProjectorDomainHandler {

    /**
     * Event type suffixes this handler processes.
     *
     * @return List of event type suffixes (e.g., "PlayerRegistered", "FundsDeposited")
     */
    List<String> eventTypes();

    /**
     * Project events to external output.
     *
     * @param events The event book containing events to project
     * @return The projection result
     * @throws ProjectionError if projection fails
     */
    Projection project(EventBook events) throws ProjectionError;

    /**
     * Exception type for projection errors.
     */
    class ProjectionError extends Exception {
        public ProjectionError(String message) {
            super(message);
        }

        public ProjectionError(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
