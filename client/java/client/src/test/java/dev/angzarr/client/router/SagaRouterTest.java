package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.*;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;

import java.util.Collections;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Tests for SagaRouter.
 *
 * Verifies the router for saga components (events -> commands, single domain, stateless).
 * Domain is set at construction time with no additional domain registration possible.
 */
class SagaRouterTest {

    // =========================================================================
    // Test Handler
    // =========================================================================

    static class TestSagaHandler implements SagaDomainHandler {

        @Override
        public List<String> eventTypes() {
            return List.of("OrderCompleted", "OrderCancelled");
        }

        @Override
        public List<Cover> prepare(EventBook source, Any event) {
            String typeUrl = event.getTypeUrl();
            if (typeUrl.endsWith("OrderCompleted")) {
                // Need to fetch fulfillment aggregate state
                String rootId = source.hasCover() && source.getCover().hasRoot()
                        ? source.getCover().getRoot().getValue().toStringUtf8()
                        : "default";
                return List.of(Cover.newBuilder()
                        .setDomain("fulfillment")
                        .setRoot(source.getCover().getRoot())
                        .build());
            }
            return Collections.emptyList();
        }

        @Override
        public SagaHandlerResponse execute(EventBook source, Any event, List<EventBook> destinations)
                throws CommandRejectedError {
            String typeUrl = event.getTypeUrl();
            if (typeUrl.endsWith("OrderCompleted")) {
                // Get destination sequence for optimistic concurrency
                int destSeq = destinations.isEmpty() ? 0 : destinations.get(0).getPagesCount();

                return SagaHandlerResponse.withCommands(List.of(CommandBook.newBuilder()
                        .setCover(Cover.newBuilder()
                                .setDomain("fulfillment")
                                .setRoot(source.getCover().getRoot())
                                .build())
                        .addPages(CommandPage.newBuilder()
                                .setHeader(PageHeader.newBuilder().setSequence(destSeq).build())
                                .setCommand(Any.newBuilder()
                                        .setTypeUrl("type.googleapis.com/test.StartFulfillment")
                                        .build())
                                .build())
                        .build()));
            } else if (typeUrl.endsWith("OrderCancelled")) {
                return SagaHandlerResponse.empty();
            }
            return SagaHandlerResponse.empty();
        }
    }

    // =========================================================================
    // Basic Router Tests
    // =========================================================================

    @Nested
    class BasicRouterTests {

        private SagaRouter router;

        @BeforeEach
        void setUp() {
            router = new SagaRouter("saga-order-fulfillment", "order", new TestSagaHandler());
        }

        @Test
        void getName_returns_router_name() {
            assertThat(router.getName()).isEqualTo("saga-order-fulfillment");
        }

        @Test
        void getInputDomain_returns_domain() {
            assertThat(router.getInputDomain()).isEqualTo("order");
        }

        @Test
        void getEventTypes_returns_handler_event_types() {
            assertThat(router.getEventTypes())
                    .containsExactly("OrderCompleted", "OrderCancelled");
        }

        @Test
        void subscriptions_returns_domain_and_event_types() {
            List<Map.Entry<String, List<String>>> subs = router.subscriptions();

            assertThat(subs).hasSize(1);
            assertThat(subs.get(0).getKey()).isEqualTo("order");
            assertThat(subs.get(0).getValue()).containsExactly("OrderCompleted", "OrderCancelled");
        }
    }

    // =========================================================================
    // Prepare Destinations Tests
    // =========================================================================

    @Nested
    class PrepareDestinationsTests {

        private SagaRouter router;

        @BeforeEach
        void setUp() {
            router = new SagaRouter("saga-order-fulfillment", "order", new TestSagaHandler());
        }

        @Test
        void prepareDestinations_null_source_returns_empty() {
            List<Cover> destinations = router.prepareDestinations(null);

            assertThat(destinations).isEmpty();
        }

        @Test
        void prepareDestinations_empty_source_returns_empty() {
            EventBook source = EventBook.getDefaultInstance();

            List<Cover> destinations = router.prepareDestinations(source);

            assertThat(destinations).isEmpty();
        }

        @Test
        void prepareDestinations_for_OrderCompleted_returns_fulfillment_cover() {
            EventBook source = makeEventBook("order", "OrderCompleted");

            List<Cover> destinations = router.prepareDestinations(source);

            assertThat(destinations).hasSize(1);
            assertThat(destinations.get(0).getDomain()).isEqualTo("fulfillment");
        }

        @Test
        void prepareDestinations_for_OrderCancelled_returns_empty() {
            EventBook source = makeEventBook("order", "OrderCancelled");

            List<Cover> destinations = router.prepareDestinations(source);

            assertThat(destinations).isEmpty();
        }
    }

    // =========================================================================
    // Dispatch Tests
    // =========================================================================

    @Nested
    class DispatchTests {

        private SagaRouter router;

        @BeforeEach
        void setUp() {
            router = new SagaRouter("saga-order-fulfillment", "order", new TestSagaHandler());
        }

        @Test
        void dispatch_null_source_throws_exception() {
            assertThatThrownBy(() -> router.dispatch(null, Collections.emptyList()))
                    .isInstanceOf(SagaRouter.RouterException.class)
                    .hasMessageContaining("Source event book has no events");
        }

        @Test
        void dispatch_empty_source_throws_exception() {
            EventBook source = EventBook.getDefaultInstance();

            assertThatThrownBy(() -> router.dispatch(source, Collections.emptyList()))
                    .isInstanceOf(SagaRouter.RouterException.class)
                    .hasMessageContaining("Source event book has no events");
        }

        @Test
        void dispatch_OrderCompleted_produces_fulfillment_command() throws Exception {
            EventBook source = makeEventBook("order", "OrderCompleted");
            List<EventBook> destinations = List.of(
                    EventBook.newBuilder()
                            .setCover(Cover.newBuilder()
                                    .setDomain("fulfillment")
                                    .build())
                            .build());

            SagaResponse response = router.dispatch(source, destinations);

            assertThat(response.getCommandsCount()).isEqualTo(1);
            assertThat(response.getCommands(0).getCover().getDomain()).isEqualTo("fulfillment");
            assertThat(response.getCommands(0).getPages(0).getCommand().getTypeUrl())
                    .endsWith("StartFulfillment");
        }

        @Test
        void dispatch_OrderCancelled_produces_no_commands() throws Exception {
            EventBook source = makeEventBook("order", "OrderCancelled");

            SagaResponse response = router.dispatch(source, Collections.emptyList());

            assertThat(response.getCommandsCount()).isEqualTo(0);
        }

        @Test
        void dispatch_uses_destination_sequence() throws Exception {
            EventBook source = makeEventBook("order", "OrderCompleted");
            // Destination has 3 prior events
            List<EventBook> destinations = List.of(
                    EventBook.newBuilder()
                            .setCover(Cover.newBuilder()
                                    .setDomain("fulfillment")
                                    .build())
                            .addPages(EventPage.getDefaultInstance())
                            .addPages(EventPage.getDefaultInstance())
                            .addPages(EventPage.getDefaultInstance())
                            .build());

            SagaResponse response = router.dispatch(source, destinations);

            assertThat(response.getCommands(0).getPages(0).getHeader().getSequence()).isEqualTo(3);
        }

        @Test
        void dispatch_no_events_field_in_response() throws Exception {
            EventBook source = makeEventBook("order", "OrderCompleted");

            SagaResponse response = router.dispatch(source, Collections.emptyList());

            // Sagas only produce commands, not events (stateless)
            assertThat(response.getEventsCount()).isEqualTo(0);
        }
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    @Nested
    class ErrorHandlingTests {

        @Test
        void dispatch_handler_throws_rejection_wraps_in_router_exception() {
            SagaDomainHandler errorHandler = new SagaDomainHandler() {
                @Override
                public List<String> eventTypes() {
                    return List.of("TestEvent");
                }

                @Override
                public List<Cover> prepare(EventBook source, Any event) {
                    return Collections.emptyList();
                }

                @Override
                public SagaHandlerResponse execute(EventBook source, Any event, List<EventBook> destinations)
                        throws CommandRejectedError {
                    throw CommandRejectedError.of("Cannot process this event");
                }
            };

            SagaRouter router = new SagaRouter("error-saga", "test", errorHandler);
            EventBook source = makeEventBook("test", "TestEvent");

            assertThatThrownBy(() -> router.dispatch(source, Collections.emptyList()))
                    .isInstanceOf(SagaRouter.RouterException.class)
                    .hasMessageContaining("Event processing failed")
                    .hasMessageContaining("Cannot process this event");
        }
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    private static EventBook makeEventBook(String domain, String eventType) {
        return EventBook.newBuilder()
                .setCover(Cover.newBuilder()
                        .setDomain(domain)
                        .setRoot(UUID.newBuilder().build())
                        .build())
                .addPages(EventPage.newBuilder()
                        .setEvent(Any.newBuilder()
                                .setTypeUrl("type.googleapis.com/test." + eventType)
                                .build())
                        .build())
                .build();
    }
}
