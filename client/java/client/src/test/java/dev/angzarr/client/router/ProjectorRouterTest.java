package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.*;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Tests for ProjectorRouter.
 *
 * Verifies the router for projector components (events -> external output, multi-domain).
 * Domains are registered via fluent .domain() calls.
 */
class ProjectorRouterTest {

    // =========================================================================
    // Test Handlers
    // =========================================================================

    static class PlayerProjectorHandler implements ProjectorDomainHandler {

        @Override
        public List<String> eventTypes() {
            return List.of("PlayerRegistered", "FundsDeposited");
        }

        @Override
        public Projection project(EventBook events) throws ProjectionError {
            // Simulate projection to read model
            return Projection.newBuilder()
                    .setProjector("player-projection")
                    .setProjection(Any.newBuilder()
                            .setTypeUrl("type.googleapis.com/test.PlayerReadModel")
                            .build())
                    .build();
        }
    }

    static class HandProjectorHandler implements ProjectorDomainHandler {

        @Override
        public List<String> eventTypes() {
            return List.of("HandStarted", "CardsDealt", "HandComplete");
        }

        @Override
        public Projection project(EventBook events) throws ProjectionError {
            return Projection.newBuilder()
                    .setProjector("hand-projection")
                    .setProjection(Any.newBuilder()
                            .setTypeUrl("type.googleapis.com/test.HandReadModel")
                            .build())
                    .build();
        }
    }

    static class FailingProjectorHandler implements ProjectorDomainHandler {

        @Override
        public List<String> eventTypes() {
            return List.of("TestEvent");
        }

        @Override
        public Projection project(EventBook events) throws ProjectionError {
            throw new ProjectionError("Database connection failed");
        }
    }

    // =========================================================================
    // Basic Router Tests
    // =========================================================================

    @Nested
    class BasicRouterTests {

        @Test
        void create_returns_empty_router() {
            ProjectorRouter router = ProjectorRouter.create("prj-test");

            assertThat(router.getName()).isEqualTo("prj-test");
            assertThat(router.subscriptions()).isEmpty();
        }

        @Test
        void domain_adds_handler_and_returns_new_router() {
            ProjectorRouter router = ProjectorRouter.create("prj-test")
                    .domain("player", new PlayerProjectorHandler());

            assertThat(router.subscriptions()).hasSize(1);
            assertThat(router.subscriptions().get(0).getKey()).isEqualTo("player");
        }

        @Test
        void multiple_domains_registers_all() {
            ProjectorRouter router = ProjectorRouter.create("prj-output")
                    .domain("player", new PlayerProjectorHandler())
                    .domain("hand", new HandProjectorHandler());

            List<Map.Entry<String, List<String>>> subs = router.subscriptions();

            assertThat(subs).hasSize(2);
            List<String> domains = subs.stream()
                    .map(Map.Entry::getKey)
                    .toList();
            assertThat(domains).containsExactlyInAnyOrder("player", "hand");
        }

        @Test
        void subscriptions_includes_event_types_per_domain() {
            ProjectorRouter router = ProjectorRouter.create("prj-output")
                    .domain("player", new PlayerProjectorHandler())
                    .domain("hand", new HandProjectorHandler());

            Map<String, List<String>> subMap = router.subscriptions().stream()
                    .collect(java.util.stream.Collectors.toMap(
                            Map.Entry::getKey, Map.Entry::getValue));

            assertThat(subMap.get("player"))
                    .containsExactly("PlayerRegistered", "FundsDeposited");
            assertThat(subMap.get("hand"))
                    .containsExactly("HandStarted", "CardsDealt", "HandComplete");
        }
    }

    // =========================================================================
    // Dispatch Tests
    // =========================================================================

    @Nested
    class DispatchTests {

        private ProjectorRouter router;

        @BeforeEach
        void setUp() {
            router = ProjectorRouter.create("prj-output")
                    .domain("player", new PlayerProjectorHandler())
                    .domain("hand", new HandProjectorHandler());
        }

        @Test
        void dispatch_unknown_domain_throws_exception() {
            EventBook events = makeEventBook("unknown", "SomeEvent");

            assertThatThrownBy(() -> router.dispatch(events))
                    .isInstanceOf(ProjectorRouter.RouterException.class)
                    .hasMessageContaining("No handler for domain: unknown");
        }

        @Test
        void dispatch_empty_domain_throws_exception() {
            EventBook events = EventBook.newBuilder()
                    .addPages(EventPage.newBuilder()
                            .setEvent(Any.newBuilder()
                                    .setTypeUrl("type.googleapis.com/test.SomeEvent")
                                    .build())
                            .build())
                    .build();

            assertThatThrownBy(() -> router.dispatch(events))
                    .isInstanceOf(ProjectorRouter.RouterException.class)
                    .hasMessageContaining("No handler for domain:");
        }

        @Test
        void dispatch_player_event_projects_successfully() throws Exception {
            EventBook events = makeEventBook("player", "PlayerRegistered");

            Projection result = router.dispatch(events);

            assertThat(result.getProjector()).isEqualTo("player-projection");
            assertThat(result.getProjection().getTypeUrl()).endsWith("PlayerReadModel");
        }

        @Test
        void dispatch_hand_event_projects_successfully() throws Exception {
            EventBook events = makeEventBook("hand", "CardsDealt");

            Projection result = router.dispatch(events);

            assertThat(result.getProjector()).isEqualTo("hand-projection");
            assertThat(result.getProjection().getTypeUrl()).endsWith("HandReadModel");
        }

        @Test
        void dispatch_multiple_pages_projects_all() throws Exception {
            EventBook events = EventBook.newBuilder()
                    .setCover(Cover.newBuilder()
                            .setDomain("hand")
                            .build())
                    .addPages(EventPage.newBuilder()
                            .setEvent(Any.newBuilder()
                                    .setTypeUrl("type.googleapis.com/test.HandStarted")
                                    .build())
                            .build())
                    .addPages(EventPage.newBuilder()
                            .setEvent(Any.newBuilder()
                                    .setTypeUrl("type.googleapis.com/test.CardsDealt")
                                    .build())
                            .build())
                    .build();

            Projection result = router.dispatch(events);

            assertThat(result.getProjector()).isEqualTo("hand-projection");
        }
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    @Nested
    class ErrorHandlingTests {

        @Test
        void dispatch_projection_error_wraps_in_router_exception() {
            ProjectorRouter router = ProjectorRouter.create("prj-test")
                    .domain("failing", new FailingProjectorHandler());

            EventBook events = makeEventBook("failing", "TestEvent");

            assertThatThrownBy(() -> router.dispatch(events))
                    .isInstanceOf(ProjectorRouter.RouterException.class)
                    .hasMessageContaining("Projection failed")
                    .hasMessageContaining("Database connection failed");
        }
    }

    // =========================================================================
    // Immutability Tests
    // =========================================================================

    @Nested
    class ImmutabilityTests {

        @Test
        void domain_returns_new_router_instance() {
            ProjectorRouter router1 = ProjectorRouter.create("prj-test");
            ProjectorRouter router2 = router1.domain("player", new PlayerProjectorHandler());

            assertThat(router1.subscriptions()).isEmpty();
            assertThat(router2.subscriptions()).hasSize(1);
        }

        @Test
        void chained_domain_calls_accumulate() {
            ProjectorRouter router = ProjectorRouter.create("prj-output")
                    .domain("player", new PlayerProjectorHandler())
                    .domain("hand", new HandProjectorHandler());

            assertThat(router.subscriptions()).hasSize(2);
        }

        @Test
        void original_router_unmodified_after_domain() {
            ProjectorRouter original = ProjectorRouter.create("prj-test");
            ProjectorRouter withPlayer = original.domain("player", new PlayerProjectorHandler());
            ProjectorRouter withBoth = withPlayer.domain("hand", new HandProjectorHandler());

            assertThat(original.subscriptions()).isEmpty();
            assertThat(withPlayer.subscriptions()).hasSize(1);
            assertThat(withBoth.subscriptions()).hasSize(2);
        }
    }

    // =========================================================================
    // Handler Interface Tests
    // =========================================================================

    @Nested
    class HandlerInterfaceTests {

        @Test
        void projectionError_preserves_message() {
            ProjectorDomainHandler.ProjectionError error =
                    new ProjectorDomainHandler.ProjectionError("Test error");

            assertThat(error.getMessage()).isEqualTo("Test error");
        }

        @Test
        void projectionError_preserves_cause() {
            RuntimeException cause = new RuntimeException("underlying error");
            ProjectorDomainHandler.ProjectionError error =
                    new ProjectorDomainHandler.ProjectionError("Test error", cause);

            assertThat(error.getMessage()).isEqualTo("Test error");
            assertThat(error.getCause()).isEqualTo(cause);
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
}
