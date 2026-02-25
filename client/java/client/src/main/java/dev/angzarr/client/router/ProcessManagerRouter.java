package dev.angzarr.client.router;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.*;
import dev.angzarr.client.Helpers;
import dev.angzarr.client.compensation.RejectionHandlerResponse;

import java.util.*;
import java.util.function.Function;

/**
 * Router for process manager components (events -> commands + PM events, multi-domain).
 *
 * <p>Domains are registered via fluent {@code .domain()} calls, supporting
 * multiple input domains.
 *
 * <p>Example:
 * <pre>{@code
 * ProcessManagerRouter<HandFlowState> router = ProcessManagerRouter
 *     .<HandFlowState>create("pmg-hand-flow", "hand-flow", HandFlowState::new)
 *     .domain("order", new OrderPmHandler())
 *     .domain("inventory", new InventoryPmHandler());
 *
 * // Get subscriptions for registration
 * List<Map.Entry<String, List<String>>> subs = router.subscriptions();
 *
 * // Prepare phase: get destinations needed
 * List<Cover> destinations = router.prepareDestinations(trigger, processState);
 *
 * // Execute phase: produce commands and PM events
 * ProcessManagerHandleResponse response = router.dispatch(trigger, processState, destinations);
 * }</pre>
 *
 * @param <S> The PM state type
 */
public class ProcessManagerRouter<S> {

    private final String name;
    private final String pmDomain;
    private final Function<EventBook, S> rebuildFn;
    private final Map<String, ProcessManagerDomainHandler<S>> domains;

    private ProcessManagerRouter(
            String name,
            String pmDomain,
            Function<EventBook, S> rebuildFn,
            Map<String, ProcessManagerDomainHandler<S>> domains) {
        this.name = name;
        this.pmDomain = pmDomain;
        this.rebuildFn = rebuildFn;
        this.domains = domains;
    }

    /**
     * Create a new process manager router builder.
     *
     * @param name The router name
     * @param pmDomain The PM's own domain for state storage
     * @param rebuildFn Function to rebuild PM state from events
     * @param <S> The PM state type
     * @return A new router ready for domain registration
     */
    public static <S> ProcessManagerRouter<S> create(
            String name,
            String pmDomain,
            Function<EventBook, S> rebuildFn) {
        return new ProcessManagerRouter<>(name, pmDomain, rebuildFn, new HashMap<>());
    }

    /**
     * Register a domain handler.
     *
     * <p>Process managers can have multiple input domains.
     * Returns a new router with the additional domain registered.
     *
     * @param domainName The domain name to listen for
     * @param handler The handler for events from this domain
     * @return A new router with the domain registered
     */
    public ProcessManagerRouter<S> domain(String domainName, ProcessManagerDomainHandler<S> handler) {
        Map<String, ProcessManagerDomainHandler<S>> newDomains = new HashMap<>(this.domains);
        newDomains.put(domainName, handler);
        return new ProcessManagerRouter<>(name, pmDomain, rebuildFn, newDomains);
    }

    /**
     * Get the router name.
     */
    public String getName() {
        return name;
    }

    /**
     * Get the PM's own domain (for state storage).
     */
    public String getPmDomain() {
        return pmDomain;
    }

    /**
     * Get subscriptions (domain + event types) for this PM.
     *
     * @return List of (domain, event types) pairs
     */
    public List<Map.Entry<String, List<String>>> subscriptions() {
        List<Map.Entry<String, List<String>>> subs = new ArrayList<>();
        for (Map.Entry<String, ProcessManagerDomainHandler<S>> entry : domains.entrySet()) {
            subs.add(new AbstractMap.SimpleEntry<>(entry.getKey(), entry.getValue().eventTypes()));
        }
        return subs;
    }

    /**
     * Rebuild PM state from events.
     *
     * @param events The PM's prior events
     * @return The rebuilt PM state
     */
    public S rebuildState(EventBook events) {
        return rebuildFn.apply(events);
    }

    /**
     * Get destinations needed for the given trigger and process state.
     *
     * <p>This is the prepare phase of the two-phase protocol.
     *
     * @param trigger The triggering event book (may be null)
     * @param processState The PM's prior state events (may be null)
     * @return List of covers for destinations that need to be fetched
     */
    public List<Cover> prepareDestinations(EventBook trigger, EventBook processState) {
        if (trigger == null || trigger.getPagesList().isEmpty()) {
            return Collections.emptyList();
        }

        String triggerDomain = trigger.hasCover() ?
                trigger.getCover().getDomain() : "";

        EventPage eventPage = trigger.getPages(trigger.getPagesCount() - 1);
        if (!eventPage.hasEvent()) {
            return Collections.emptyList();
        }

        Any eventAny = eventPage.getEvent();
        S state = processState != null ? rebuildState(processState) : rebuildFn.apply(null);

        ProcessManagerDomainHandler<S> handler = domains.get(triggerDomain);
        if (handler == null) {
            return Collections.emptyList();
        }

        return handler.prepare(trigger, state, eventAny);
    }

    /**
     * Dispatch a trigger event to the appropriate handler.
     *
     * <p>This is the execute phase of the two-phase protocol.
     *
     * @param trigger The triggering event book
     * @param processState The PM's prior state events
     * @param destinations The fetched destination event books
     * @return The PM response containing commands and process events
     * @throws RouterException if dispatch fails
     */
    public ProcessManagerHandleResponse dispatch(
            EventBook trigger,
            EventBook processState,
            List<EventBook> destinations) throws RouterException {

        String triggerDomain = trigger.hasCover() ?
                trigger.getCover().getDomain() : "";

        ProcessManagerDomainHandler<S> handler = domains.get(triggerDomain);
        if (handler == null) {
            throw new RouterException("No handler for domain: " + triggerDomain);
        }

        if (trigger.getPagesList().isEmpty()) {
            throw new RouterException("Trigger event book has no events");
        }

        EventPage eventPage = trigger.getPages(trigger.getPagesCount() - 1);
        if (!eventPage.hasEvent()) {
            throw new RouterException("Missing event payload");
        }

        Any eventAny = eventPage.getEvent();
        S state = processState != null ? rebuildState(processState) : rebuildFn.apply(null);

        // Check for Notification
        if (eventAny.getTypeUrl().endsWith("Notification")) {
            return dispatchNotification(handler, eventAny, state);
        }

        try {
            ProcessManagerResponse response = handler.handle(trigger, state, eventAny, destinations);

            ProcessManagerHandleResponse.Builder builder = ProcessManagerHandleResponse.newBuilder()
                    .addAllCommands(response.getCommands());
            if (response.hasProcessEvents()) {
                builder.setProcessEvents(response.getProcessEvents());
            }
            return builder.build();
        } catch (CommandRejectedError e) {
            throw new RouterException("Event processing failed: " + e.getReason(), e);
        }
    }

    /**
     * Dispatch a notification to the PM's rejection handler.
     */
    private ProcessManagerHandleResponse dispatchNotification(
            ProcessManagerDomainHandler<S> handler,
            Any eventAny,
            S state) throws RouterException {
        try {
            Notification notification = eventAny.unpack(Notification.class);

            String targetDomain = "";
            String targetCommand = "";

            if (notification.hasPayload()) {
                try {
                    RejectionNotification rejection = notification.getPayload()
                            .unpack(RejectionNotification.class);
                    if (rejection.hasRejectedCommand() &&
                            rejection.getRejectedCommand().getPagesCount() > 0) {
                        CommandBook rejectedCmd = rejection.getRejectedCommand();
                        targetDomain = rejectedCmd.hasCover() ?
                                rejectedCmd.getCover().getDomain() : "";
                        targetCommand = Helpers.typeNameFromUrl(
                                rejectedCmd.getPages(0).getCommand().getTypeUrl());
                    }
                } catch (InvalidProtocolBufferException ignored) {
                    // Malformed rejection notification
                }
            }

            RejectionHandlerResponse response = handler.onRejected(
                    notification, state, targetDomain, targetCommand);

            ProcessManagerHandleResponse.Builder builder = ProcessManagerHandleResponse.newBuilder();
            if (response.hasEvents()) {
                builder.setProcessEvents(response.getEvents());
            }
            return builder.build();
        } catch (InvalidProtocolBufferException e) {
            throw new RouterException("Failed to decode Notification", e);
        } catch (CommandRejectedError e) {
            throw new RouterException("Rejection handler failed: " + e.getReason(), e);
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
