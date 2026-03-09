package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.*;
import dev.angzarr.client.Helpers;
import dev.angzarr.client.StateRouter;
import dev.angzarr.client.compensation.RejectionHandlerResponse;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Tests for CommandHandlerRouter.
 *
 * Verifies the router for command handler components (commands -> events, single domain).
 * Domain is set at construction time with no additional domain registration possible.
 */
class CommandHandlerRouterTest {

    // =========================================================================
    // Test State and Handler
    // =========================================================================

    static class TestState {
        int value = 0;
        boolean exists = false;
    }

    static class TestHandler implements CommandHandlerDomainHandler<TestState> {
        private final StateRouter<TestState> stateRouter;

        TestHandler() {
            this.stateRouter = new StateRouter<>(TestState::new);
            // No event registration needed for basic tests
        }

        @Override
        public List<String> commandTypes() {
            return List.of("CreateEntity", "UpdateValue");
        }

        @Override
        public StateRouter<TestState> stateRouter() {
            return stateRouter;
        }

        @Override
        public EventBook handle(CommandBook cmd, Any payload, TestState state, int seq)
                throws CommandRejectedError {
            String typeUrl = payload.getTypeUrl();
            if (typeUrl.endsWith("CreateEntity")) {
                if (state.exists) {
                    throw CommandRejectedError.of("Entity already exists");
                }
                return EventBook.newBuilder()
                        .setCover(cmd.getCover())
                        .addPages(EventPage.newBuilder()
                                .setHeader(PageHeader.newBuilder().setSequence(seq).build())
                                .setEvent(Any.newBuilder()
                                        .setTypeUrl("type.googleapis.com/test.EntityCreated")
                                        .build())
                                .build())
                        .build();
            } else if (typeUrl.endsWith("UpdateValue")) {
                if (!state.exists) {
                    throw CommandRejectedError.of("Entity does not exist");
                }
                return EventBook.newBuilder()
                        .setCover(cmd.getCover())
                        .addPages(EventPage.newBuilder()
                                .setHeader(PageHeader.newBuilder().setSequence(seq).build())
                                .setEvent(Any.newBuilder()
                                        .setTypeUrl("type.googleapis.com/test.ValueUpdated")
                                        .build())
                                .build())
                        .build();
            }
            throw CommandRejectedError.of("Unknown command: " + typeUrl);
        }
    }

    // =========================================================================
    // Basic Router Tests
    // =========================================================================

    @Nested
    class BasicRouterTests {

        private CommandHandlerRouter<TestState> router;

        @BeforeEach
        void setUp() {
            router = new CommandHandlerRouter<>("test-handler", "test-domain", new TestHandler());
        }

        @Test
        void getName_returns_router_name() {
            assertThat(router.getName()).isEqualTo("test-handler");
        }

        @Test
        void getDomain_returns_domain() {
            assertThat(router.getDomain()).isEqualTo("test-domain");
        }

        @Test
        void getCommandTypes_returns_handler_command_types() {
            assertThat(router.getCommandTypes())
                    .containsExactly("CreateEntity", "UpdateValue");
        }

        @Test
        void subscriptions_returns_domain_and_command_types() {
            List<Map.Entry<String, List<String>>> subs = router.subscriptions();

            assertThat(subs).hasSize(1);
            assertThat(subs.get(0).getKey()).isEqualTo("test-domain");
            assertThat(subs.get(0).getValue()).containsExactly("CreateEntity", "UpdateValue");
        }

        @Test
        void rebuildState_uses_handler_state_router() {
            EventBook emptyBook = EventBook.getDefaultInstance();

            TestState state = router.rebuildState(emptyBook);

            assertThat(state).isNotNull();
            assertThat(state.value).isEqualTo(0);
            assertThat(state.exists).isFalse();
        }
    }

    // =========================================================================
    // Dispatch Tests
    // =========================================================================

    @Nested
    class DispatchTests {

        private CommandHandlerRouter<TestState> router;

        @BeforeEach
        void setUp() {
            router = new CommandHandlerRouter<>("test-handler", "test-domain", new TestHandler());
        }

        @Test
        void dispatch_missing_command_book_throws_exception() {
            ContextualCommand cmd = ContextualCommand.newBuilder()
                    .build();

            assertThatThrownBy(() -> router.dispatch(cmd))
                    .isInstanceOf(CommandHandlerRouter.RouterException.class)
                    .hasMessageContaining("Missing command book");
        }

        @Test
        void dispatch_empty_pages_throws_exception() {
            ContextualCommand cmd = ContextualCommand.newBuilder()
                    .setCommand(CommandBook.newBuilder().build())
                    .build();

            assertThatThrownBy(() -> router.dispatch(cmd))
                    .isInstanceOf(CommandHandlerRouter.RouterException.class)
                    .hasMessageContaining("Missing command book or pages");
        }

        @Test
        void dispatch_successful_command_returns_events() throws Exception {
            ContextualCommand cmd = makeContextualCommand(
                    "test-domain", "CreateEntity", EventBook.getDefaultInstance());

            BusinessResponse response = router.dispatch(cmd);

            assertThat(response.hasEvents()).isTrue();
            assertThat(response.getEvents().getPagesCount()).isEqualTo(1);
            assertThat(response.getEvents().getPages(0).getEvent().getTypeUrl())
                    .endsWith("EntityCreated");
        }

        @Test
        void dispatch_rejected_command_throws_router_exception() {
            // Create with existing state (exists=true) would require event application
            // For this test, we use UpdateValue on non-existent entity
            ContextualCommand cmd = makeContextualCommand(
                    "test-domain", "UpdateValue", EventBook.getDefaultInstance());

            assertThatThrownBy(() -> router.dispatch(cmd))
                    .isInstanceOf(CommandHandlerRouter.RouterException.class)
                    .hasMessageContaining("Command rejected")
                    .hasMessageContaining("Entity does not exist");
        }

        @Test
        void dispatch_unknown_command_throws_router_exception() {
            ContextualCommand cmd = makeContextualCommand(
                    "test-domain", "UnknownCommand", EventBook.getDefaultInstance());

            assertThatThrownBy(() -> router.dispatch(cmd))
                    .isInstanceOf(CommandHandlerRouter.RouterException.class)
                    .hasMessageContaining("Unknown command");
        }
    }

    // =========================================================================
    // Notification Handling Tests
    // =========================================================================

    @Nested
    class NotificationHandlingTests {

        static class NotificationHandler implements CommandHandlerDomainHandler<TestState> {
            private final StateRouter<TestState> stateRouter = new StateRouter<>(TestState::new);
            private boolean onRejectedCalled = false;
            private String lastTargetDomain = "";
            private String lastTargetCommand = "";

            @Override
            public List<String> commandTypes() {
                return List.of("TestCommand");
            }

            @Override
            public StateRouter<TestState> stateRouter() {
                return stateRouter;
            }

            @Override
            public EventBook handle(CommandBook cmd, Any payload, TestState state, int seq)
                    throws CommandRejectedError {
                return EventBook.getDefaultInstance();
            }

            @Override
            public RejectionHandlerResponse onRejected(
                    Notification notification,
                    TestState state,
                    String targetDomain,
                    String targetCommand) throws CommandRejectedError {
                onRejectedCalled = true;
                lastTargetDomain = targetDomain;
                lastTargetCommand = targetCommand;
                return RejectionHandlerResponse.empty();
            }
        }

        @Test
        void dispatch_notification_calls_onRejected() throws Exception {
            NotificationHandler handler = new NotificationHandler();
            CommandHandlerRouter<TestState> router = new CommandHandlerRouter<>(
                    "test-handler", "test-domain", handler);

            ContextualCommand cmd = makeNotificationCommand("inventory", "ReserveStock", "out of stock");

            BusinessResponse response = router.dispatch(cmd);

            assertThat(handler.onRejectedCalled).isTrue();
            assertThat(handler.lastTargetDomain).isEqualTo("inventory");
            // Type name includes package prefix from type URL after the last '/'
            assertThat(handler.lastTargetCommand).isEqualTo("test.ReserveStock");
            // Empty response results in revocation
            assertThat(response.hasRevocation()).isTrue();
        }

        @Test
        void dispatch_notification_with_events_returns_events() throws Exception {
            CommandHandlerDomainHandler<TestState> handler = new CommandHandlerDomainHandler<>() {
                private final StateRouter<TestState> stateRouter = new StateRouter<>(TestState::new);

                @Override
                public List<String> commandTypes() {
                    return List.of("TestCommand");
                }

                @Override
                public StateRouter<TestState> stateRouter() {
                    return stateRouter;
                }

                @Override
                public EventBook handle(CommandBook cmd, Any payload, TestState state, int seq) {
                    return EventBook.getDefaultInstance();
                }

                @Override
                public RejectionHandlerResponse onRejected(
                        Notification notification, TestState state,
                        String targetDomain, String targetCommand) {
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

            CommandHandlerRouter<TestState> router = new CommandHandlerRouter<>(
                    "test-handler", "test-domain", handler);

            ContextualCommand cmd = makeNotificationCommand("inventory", "ReserveStock", "out of stock");

            BusinessResponse response = router.dispatch(cmd);

            assertThat(response.hasEvents()).isTrue();
            assertThat(response.getEvents().getPages(0).getEvent().getTypeUrl())
                    .endsWith("CompensationEvent");
        }
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    private static ContextualCommand makeContextualCommand(
            String domain, String commandType, EventBook priorEvents) {
        return ContextualCommand.newBuilder()
                .setCommand(CommandBook.newBuilder()
                        .setCover(Cover.newBuilder()
                                .setDomain(domain)
                                .build())
                        .addPages(CommandPage.newBuilder()
                                .setCommand(Any.newBuilder()
                                        .setTypeUrl("type.googleapis.com/test." + commandType)
                                        .build())
                                .build())
                        .build())
                .setEvents(priorEvents)
                .build();
    }

    private static ContextualCommand makeNotificationCommand(
            String targetDomain, String targetCommand, String reason) {
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

        return ContextualCommand.newBuilder()
                .setCommand(CommandBook.newBuilder()
                        .setCover(Cover.newBuilder()
                                .setDomain("test-domain")
                                .build())
                        .addPages(CommandPage.newBuilder()
                                .setCommand(Any.pack(notification))
                                .build())
                        .build())
                .setEvents(EventBook.getDefaultInstance())
                .build();
    }
}
