package dev.angzarr.client.router;

import dev.angzarr.*;

import java.util.*;

/**
 * Router for projector components (events -> external output, multi-domain).
 *
 * <p>Domains are registered via fluent {@code .domain()} calls, supporting
 * multiple input domains.
 *
 * <p>Example:
 * <pre>{@code
 * ProjectorRouter router = ProjectorRouter.create("prj-output")
 *     .domain("player", new PlayerProjectorHandler())
 *     .domain("hand", new HandProjectorHandler());
 *
 * // Get subscriptions for registration
 * List<Map.Entry<String, List<String>>> subs = router.subscriptions();
 *
 * // Project events
 * Projection result = router.dispatch(events);
 * }</pre>
 */
public class ProjectorRouter {

    private final String name;
    private final Map<String, ProjectorDomainHandler> domains;

    private ProjectorRouter(String name, Map<String, ProjectorDomainHandler> domains) {
        this.name = name;
        this.domains = domains;
    }

    /**
     * Create a new projector router.
     *
     * @param name The router name
     * @return A new router ready for domain registration
     */
    public static ProjectorRouter create(String name) {
        return new ProjectorRouter(name, new HashMap<>());
    }

    /**
     * Register a domain handler.
     *
     * <p>Projectors can have multiple input domains.
     * Returns a new router with the additional domain registered.
     *
     * @param domainName The domain name to listen for
     * @param handler The handler for events from this domain
     * @return A new router with the domain registered
     */
    public ProjectorRouter domain(String domainName, ProjectorDomainHandler handler) {
        Map<String, ProjectorDomainHandler> newDomains = new HashMap<>(this.domains);
        newDomains.put(domainName, handler);
        return new ProjectorRouter(name, newDomains);
    }

    /**
     * Get the router name.
     */
    public String getName() {
        return name;
    }

    /**
     * Get subscriptions (domain + event types) for this projector.
     *
     * @return List of (domain, event types) pairs
     */
    public List<Map.Entry<String, List<String>>> subscriptions() {
        List<Map.Entry<String, List<String>>> subs = new ArrayList<>();
        for (Map.Entry<String, ProjectorDomainHandler> entry : domains.entrySet()) {
            subs.add(new AbstractMap.SimpleEntry<>(entry.getKey(), entry.getValue().eventTypes()));
        }
        return subs;
    }

    /**
     * Dispatch events to the appropriate handler.
     *
     * @param events The event book to project
     * @return The projection result
     * @throws RouterException if dispatch fails
     */
    public Projection dispatch(EventBook events) throws RouterException {
        String domain = events.hasCover() ? events.getCover().getDomain() : "";

        ProjectorDomainHandler handler = domains.get(domain);
        if (handler == null) {
            throw new RouterException("No handler for domain: " + domain);
        }

        try {
            return handler.project(events);
        } catch (ProjectorDomainHandler.ProjectionError e) {
            throw new RouterException("Projection failed: " + e.getMessage(), e);
        }
    }

    /**
     * Exception type for router errors.
     */
    public static class RouterException extends Exception {
        public RouterException(String message) {
            super(message);
        }

        public RouterException(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
