package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.*;
import dev.angzarr.client.compensation.RejectionHandlerResponse;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;

import java.util.Collections;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Tests for ProcessManagerRouter.
 *
 * Verifies the router for process manager components (events -> commands + PM events, multi-domain).
 * Domains are registered via fluent .domain() calls.
 */
class ProcessManagerRouterTest {

    // =========================================================================
    // Test State and Handler
    // =========================================================================

    static class TestPmState {
        int eventsProcessed = 0;
        boolean orderReceived = false;
        boolean inventoryChecked = false;
    }

    static class OrderPmHandler implements ProcessManagerDomainHandler<TestPmState> {

        @Override
        public List<String> eventTypes() {
            return List.of("OrderCreated", "OrderCompleted");
        }

        @Override
        public List<Cover> prepare(EventBook trigger, TestPmState state, Any event) {
            if (event.getTypeUrl().endsWith("OrderCreated")) {
                return List.of(Cover.newBuilder()
                        .setDomain("inventory")
                        .build());
            }
            return Collections.emptyList();
        }

        @Override
        public ProcessManagerResponse handle(
                EventBook trigger, TestPmState state, Any event, List<EventBook> destinations)
                throws CommandRejectedError {
            if (event.getTypeUrl().endsWith("OrderCreated")) {
                // Issue command to inventory and emit PM event
                CommandBook inventoryCmd = CommandBook.newBuilder()
                        .setCover(Cover.newBuilder()
                                .setDomain("inventory")
                                .build())
                        .addPages(CommandPage.newBuilder()
                                .setCommand(Any.newBuilder()
                                        .setTypeUrl("type.googleapis.com/test.ReserveStock")
                                        .build())
                                .build())
                        .build();

                EventBook pmEvent = EventBook.newBuilder()
                        .addPages(EventPage.newBuilder()
                                .setEvent(Any.newBuilder()
                                        .setTypeUrl("type.googleapis.com/test.OrderProcessingStarted")
                                        .build())
                                .build())
                        .build();

                return ProcessManagerResponse.withBoth(List.of(inventoryCmd), pmEvent);
            }
            return ProcessManagerResponse.empty();
        }
    }

    static class InventoryPmHandler implements ProcessManagerDomainHandler<TestPmState> {

        @Override
        public List<String> eventTypes() {
            return List.of("StockReserved", "StockUnavailable");
        }

        @Override
        public List<Cover> prepare(EventBook trigger, TestPmState state, Any event) {
            return Collections.emptyList();
        }

        @Override
        public ProcessManagerResponse handle(
                EventBook trigger, TestPmState state, Any event, List<EventBook> destinations)
                throws CommandRejectedError {
            if (event.getTypeUrl().endsWith("StockReserved")) {
                // PM transition - emit process event only
                return ProcessManagerResponse.withProcessEvents(
                        EventBook.newBuilder()
                                .addPages(EventPage.newBuilder()
                                        .setEvent(Any.newBuilder()
                                                .setTypeUrl("type.googleapis.com/test.InventoryConfirmed")
                                                .build())
                                        .build())
                                .build());
            }
            return ProcessManagerResponse.empty();
        }
    }

    // =========================================================================
    // Basic Router Tests
    // =========================================================================

    @Nested
    class BasicRouterTests {

        @Test
        void create_returns_empty_router() {
            ProcessManagerRouter<TestPmState> router = ProcessManagerRouter.create(
                    "pmg-test", "test-pm", events -> new TestPmState());

            assertThat(router.getName()).isEqualTo("pmg-test");
            assertThat(router.getPmDomain()).isEqualTo("test-pm");
            assertThat(router.subscriptions()).isEmpty();
        }

        @Test
        void domain_adds_handler_and_returns_new_router() {
            ProcessManagerRouter<TestPmState> router = ProcessManagerRouter
                    .<TestPmState>create("pmg-test", "test-pm", events -> new TestPmState())
                    .domain("order", new OrderPmHandler());

            assertThat(router.subscriptions()).hasSize(1);
            assertThat(router.subscriptions().get(0).getKey()).isEqualTo("order");
        }

        @Test
        void multiple_domains_registers_all() {
            ProcessManagerRouter<TestPmState> router = ProcessManagerRouter
                    .<TestPmState>create("pmg-test", "test-pm", events -> new TestPmState())
                    .domain("order", new OrderPmHandler())
                    .domain("inventory", new InventoryPmHandler());

            List<Map.Entry<String, List<String>>> subs = router.subscriptions();

            assertThat(subs).hasSize(2);
            // Extract domains from subscriptions
            List<String> domains = subs.stream()
                    .map(Map.Entry::getKey)
                    .toList();
            assertThat(domains).containsExactlyInAnyOrder("order", "inventory");
        }

        @Test
        void subscriptions_includes_event_types_per_domain() {
            ProcessManagerRouter<TestPmState> router = ProcessManagerRouter
                    .<TestPmState>create("pmg-test", "test-pm", events -> new TestPmState())
                    .domain("order", new OrderPmHandler())
                    .domain("inventory", new InventoryPmHandler());

            Map<String, List<String>> subMap = router.subscriptions().stream()
                    .collect(java.util.stream.Collectors.toMap(
                            Map.Entry::getKey, Map.Entry::getValue));

            assertThat(subMap.get("order"))
                    .containsExactly("OrderCreated", "OrderCompleted");
            assertThat(subMap.get("inventory"))
                    .containsExactly("StockReserved", "StockUnavailable");
        }
    }

    // =========================================================================
    // State Rebuilding Tests
    // =========================================================================

    @Nested
    class StateRebuildingTests {

        @Test
        void rebuildState_uses_provided_function() {
            ProcessManagerRouter<TestPmState> router = ProcessManagerRouter.create(
                    "pmg-test", "test-pm", events -> {
                        TestPmState state = new TestPmState();
                        state.eventsProcessed = events != null ? events.getPagesCount() : 0;
                        return state;
                    });

            EventBook events = EventBook.newBuilder()
                    .addPages(EventPage.getDefaultInstance())
                    .addPages(EventPage.getDefaultInstance())
                    .build();

            TestPmState state = router.rebuildState(events);

            assertThat(state.eventsProcessed).isEqualTo(2);
        }
    }

    // =========================================================================
    // Prepare Destinations Tests
    // =========================================================================

    @Nested
    class PrepareDestinationsTests {

        private ProcessManagerRouter<TestPmState> router;

        @BeforeEach
        void setUp() {
            router = ProcessManagerRouter
                    .<TestPmState>create("pmg-test", "test-pm", events -> new TestPmState())
                    .domain("order", new OrderPmHandler())
                    .domain("inventory", new InventoryPmHandler());
        }

        @Test
        void prepareDestinations_null_trigger_returns_empty() {
            List<Cover> destinations = router.prepareDestinations(null, null);

            assertThat(destinations).isEmpty();
        }

        @Test
        void prepareDestinations_empty_trigger_returns_empty() {
            List<Cover> destinations = router.prepareDestinations(
                    EventBook.getDefaultInstance(), null);

            assertThat(destinations).isEmpty();
        }

        @Test
        void prepareDestinations_unknown_domain_returns_empty() {
            EventBook trigger = makeEventBook("unknown", "SomeEvent");

            List<Cover> destinations = router.prepareDestinations(trigger, null);

            assertThat(destinations).isEmpty();
        }

        @Test
        void prepareDestinations_for_order_event_returns_inventory_cover() {
            EventBook trigger = makeEventBook("order", "OrderCreated");

            List<Cover> destinations = router.prepareDestinations(trigger, null);

            assertThat(destinations).hasSize(1);
            assertThat(destinations.get(0).getDomain()).isEqualTo("inventory");
        }
    }

    // =========================================================================
    // Dispatch Tests
    // =========================================================================

    @Nested
    class DispatchTests {

        private ProcessManagerRouter<TestPmState> router;

        @BeforeEach
        void setUp() {
            router = ProcessManagerRouter
                    .<TestPmState>create("pmg-test", "test-pm", events -> new TestPmState())
                    .domain("order", new OrderPmHandler())
                    .domain("inventory", new InventoryPmHandler());
        }

        @Test
        void dispatch_unknown_domain_throws_exception() {
            EventBook trigger = makeEventBook("unknown", "SomeEvent");

            assertThatThrownBy(() -> router.dispatch(trigger, null, Collections.emptyList()))
                    .isInstanceOf(ProcessManagerRouter.RouterException.class)
                    .hasMessageContaining("No handler for domain: unknown");
        }

        @Test
        void dispatch_empty_trigger_throws_exception() {
            EventBook trigger = EventBook.newBuilder()
                    .setCover(Cover.newBuilder().setDomain("order").build())
                    .build();

            assertThatThrownBy(() -> router.dispatch(trigger, null, Collections.emptyList()))
                    .isInstanceOf(ProcessManagerRouter.RouterException.class)
                    .hasMessageContaining("Trigger event book has no events");
        }

        @Test
        void dispatch_order_event_produces_command_and_pm_event() throws Exception {
            EventBook trigger = makeEventBook("order", "OrderCreated");

            ProcessManagerHandleResponse response = router.dispatch(
                    trigger, null, Collections.emptyList());

            assertThat(response.getCommandsCount()).isEqualTo(1);
            assertThat(response.getCommands(0).getCover().getDomain()).isEqualTo("inventory");
            assertThat(response.hasProcessEvents()).isTrue();
            assertThat(response.getProcessEvents().getPages(0).getEvent().getTypeUrl())
                    .endsWith("OrderProcessingStarted");
        }

        @Test
        void dispatch_inventory_event_produces_pm_event_only() throws Exception {
            EventBook trigger = makeEventBook("inventory", "StockReserved");

            ProcessManagerHandleResponse response = router.dispatch(
                    trigger, null, Collections.emptyList());

            assertThat(response.getCommandsCount()).isEqualTo(0);
            assertThat(response.hasProcessEvents()).isTrue();
            assertThat(response.getProcessEvents().getPages(0).getEvent().getTypeUrl())
                    .endsWith("InventoryConfirmed");
        }
    }

    // =========================================================================
    // Notification Handling Tests
    // =========================================================================

    @Nested
    class NotificationHandlingTests {

        @Test
        void dispatch_notification_calls_onRejected() throws Exception {
            final boolean[] onRejectedCalled = {false};
            final String[] capturedDomain = {""};
            final String[] capturedCommand = {""};

            ProcessManagerDomainHandler<TestPmState> handler = new ProcessManagerDomainHandler<>() {
                @Override
                public List<String> eventTypes() {
                    return List.of("TestEvent");
                }

                @Override
                public List<Cover> prepare(EventBook trigger, TestPmState state, Any event) {
                    return Collections.emptyList();
                }

                @Override
                public ProcessManagerResponse handle(
                        EventBook trigger, TestPmState state, Any event, List<EventBook> destinations) {
                    return ProcessManagerResponse.empty();
                }

                @Override
                public RejectionHandlerResponse onRejected(
                        Notification notification, TestPmState state,
                        String targetDomain, String targetCommand) {
                    onRejectedCalled[0] = true;
                    capturedDomain[0] = targetDomain;
                    capturedCommand[0] = targetCommand;
                    return RejectionHandlerResponse.withEvents(
                            EventBook.newBuilder()
                                    .addPages(EventPage.newBuilder()
                                            .setEvent(Any.newBuilder()
                                                    .setTypeUrl("type.googleapis.com/test.CompensationEvent")
                                                    .build())
                                            .build())
                                    .build());
                }
            };

            ProcessManagerRouter<TestPmState> router = ProcessManagerRouter
                    .<TestPmState>create("pmg-test", "test-pm", events -> new TestPmState())
                    .domain("test", handler);

            EventBook trigger = makeNotificationTrigger("test", "inventory", "ReserveStock", "out of stock");

            ProcessManagerHandleResponse response = router.dispatch(trigger, null, Collections.emptyList());

            assertThat(onRejectedCalled[0]).isTrue();
            assertThat(capturedDomain[0]).isEqualTo("inventory");
            // Type name includes package prefix from type URL after the last '/'
            assertThat(capturedCommand[0]).isEqualTo("test.ReserveStock");
            assertThat(response.hasProcessEvents()).isTrue();
        }
    }

    // =========================================================================
    // Immutability Tests
    // =========================================================================

    @Nested
    class ImmutabilityTests {

        @Test
        void domain_returns_new_router_instance() {
            ProcessManagerRouter<TestPmState> router1 = ProcessManagerRouter.create(
                    "pmg-test", "test-pm", events -> new TestPmState());

            ProcessManagerRouter<TestPmState> router2 = router1.domain("order", new OrderPmHandler());

            assertThat(router1.subscriptions()).isEmpty();
            assertThat(router2.subscriptions()).hasSize(1);
        }

        @Test
        void chained_domain_calls_accumulate() {
            ProcessManagerRouter<TestPmState> router = ProcessManagerRouter
                    .<TestPmState>create("pmg-test", "test-pm", events -> new TestPmState())
                    .domain("order", new OrderPmHandler())
                    .domain("inventory", new InventoryPmHandler());

            assertThat(router.subscriptions()).hasSize(2);
        }
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    private static EventBook makeEventBook(String domain, String eventType) {
        return EventBook.newBuilder()
                .setCover(Cover.newBuilder()
                        .setDomain(domain)
                        .build())
                .addPages(EventPage.newBuilder()
                        .setEvent(Any.newBuilder()
                                .setTypeUrl("type.googleapis.com/test." + eventType)
                                .build())
                        .build())
                .build();
    }

    private static EventBook makeNotificationTrigger(
            String triggerDomain, String targetDomain, String targetCommand, String reason) {
        CommandBook rejectedCommand = CommandBook.newBuilder()
                .setCover(Cover.newBuilder()
                        .setDomain(targetDomain)
                        .build())
                .addPages(CommandPage.newBuilder()
                        .setCommand(Any.newBuilder()
                                .setTypeUrl("type.googleapis.com/test." + targetCommand)
                                .build())
                        .build())
                .build();

        RejectionNotification rejection = RejectionNotification.newBuilder()
                .setRejectionReason(reason)
                .setRejectedCommand(rejectedCommand)
                .build();

        Notification notification = Notification.newBuilder()
                .setPayload(Any.pack(rejection))
                .build();

        return EventBook.newBuilder()
                .setCover(Cover.newBuilder()
                        .setDomain(triggerDomain)
                        .build())
                .addPages(EventPage.newBuilder()
                        .setEvent(Any.pack(notification))
                        .build())
                .build();
    }
}
