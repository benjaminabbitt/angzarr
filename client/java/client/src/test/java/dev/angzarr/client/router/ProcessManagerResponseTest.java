package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.*;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Tests for ProcessManagerResponse.
 *
 * Verifies the response type from process manager handlers that can contain
 * both commands (to send to other aggregates) and process events (to persist PM state).
 */
class ProcessManagerResponseTest {

    @Test
    void empty_response_has_no_commands_or_events() {
        ProcessManagerResponse response = ProcessManagerResponse.empty();

        assertThat(response.hasCommands()).isFalse();
        assertThat(response.hasProcessEvents()).isFalse();
        assertThat(response.getCommands()).isEmpty();
        assertThat(response.getProcessEvents()).isNull();
    }

    @Test
    void response_with_commands_only() {
        List<CommandBook> commands = List.of(makeCommandBook("order", "CreateOrder"));

        ProcessManagerResponse response = ProcessManagerResponse.withCommands(commands);

        assertThat(response.hasCommands()).isTrue();
        assertThat(response.hasProcessEvents()).isFalse();
        assertThat(response.getCommands()).hasSize(1);
        assertThat(response.getProcessEvents()).isNull();
    }

    @Test
    void response_with_process_events_only() {
        EventBook processEvents = makeEventBook("pm-domain", "StateUpdated");

        ProcessManagerResponse response = ProcessManagerResponse.withProcessEvents(processEvents);

        assertThat(response.hasCommands()).isFalse();
        assertThat(response.hasProcessEvents()).isTrue();
        assertThat(response.getCommands()).isEmpty();
        assertThat(response.getProcessEvents()).isEqualTo(processEvents);
    }

    @Test
    void response_with_both_commands_and_events() {
        List<CommandBook> commands = List.of(
                makeCommandBook("inventory", "ReserveStock"),
                makeCommandBook("fulfillment", "StartFulfillment"));
        EventBook processEvents = makeEventBook("pm-domain", "ProcessStarted");

        ProcessManagerResponse response = ProcessManagerResponse.withBoth(commands, processEvents);

        assertThat(response.hasCommands()).isTrue();
        assertThat(response.hasProcessEvents()).isTrue();
        assertThat(response.getCommands()).hasSize(2);
        assertThat(response.getProcessEvents()).isEqualTo(processEvents);
    }

    @Test
    void response_commands_are_accessible() {
        List<CommandBook> commands = List.of(
                makeCommandBook("order", "CreateOrder"),
                makeCommandBook("inventory", "ReserveStock"));

        ProcessManagerResponse response = ProcessManagerResponse.withCommands(commands);

        assertThat(response.getCommands()).hasSize(2);
        assertThat(response.getCommands().get(0).getCover().getDomain()).isEqualTo("order");
        assertThat(response.getCommands().get(1).getCover().getDomain()).isEqualTo("inventory");
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    private static CommandBook makeCommandBook(String domain, String commandType) {
        return CommandBook.newBuilder()
                .setCover(Cover.newBuilder()
                        .setDomain(domain)
                        .build())
                .addPages(CommandPage.newBuilder()
                        .setCommand(Any.newBuilder()
                                .setTypeUrl("type.googleapis.com/test." + commandType)
                                .build())
                        .build())
                .build();
    }

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
