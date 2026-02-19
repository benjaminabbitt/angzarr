package dev.angzarr.client.compensation;

import dev.angzarr.EventBook;
import dev.angzarr.Notification;

/**
 * Response from rejection handlers - can emit events AND/OR notification.
 *
 * <p>Rejection handlers may need to:
 * <ul>
 *   <li>Emit events (EventBook) - to fix/retry/compensate state</li>
 *   <li>Emit notification upstream - to forward rejection to originating component</li>
 *   <li>Both - compensate locally AND notify upstream</li>
 * </ul>
 */
public class RejectionHandlerResponse {

    private final EventBook events;
    private final Notification notification;

    private RejectionHandlerResponse(EventBook events, Notification notification) {
        this.events = events;
        this.notification = notification;
    }

    /**
     * Create empty response (no events, no notification).
     */
    public static RejectionHandlerResponse empty() {
        return new RejectionHandlerResponse(null, null);
    }

    /**
     * Create response with compensation events.
     */
    public static RejectionHandlerResponse withEvents(EventBook events) {
        return new RejectionHandlerResponse(events, null);
    }

    /**
     * Create response with upstream notification.
     */
    public static RejectionHandlerResponse withNotification(Notification notification) {
        return new RejectionHandlerResponse(null, notification);
    }

    /**
     * Create response with both events and notification.
     */
    public static RejectionHandlerResponse withBoth(EventBook events, Notification notification) {
        return new RejectionHandlerResponse(events, notification);
    }

    /**
     * Events to persist to own state (compensation).
     */
    public EventBook getEvents() {
        return events;
    }

    /**
     * Check if response has events.
     */
    public boolean hasEvents() {
        return events != null;
    }

    /**
     * Notification to forward upstream (rejection propagation).
     */
    public Notification getNotification() {
        return notification;
    }

    /**
     * Check if response has notification.
     */
    public boolean hasNotification() {
        return notification != null;
    }
}
