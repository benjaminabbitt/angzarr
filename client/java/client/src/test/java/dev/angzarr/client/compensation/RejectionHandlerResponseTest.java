package dev.angzarr.client.compensation;

import com.google.protobuf.Any;
import dev.angzarr.*;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Integration tests for RejectionHandlerResponse.
 *
 * Tests the unified response type for rejection handlers that can return
 * both compensation events AND upstream notification.
 */
class RejectionHandlerResponseTest {

    // =========================================================================
    // RejectionHandlerResponse Tests
    // =========================================================================

    @Test
    void empty_response_has_no_events_or_notification() {
        RejectionHandlerResponse response = RejectionHandlerResponse.empty();

        assertThat(response.hasEvents()).isFalse();
        assertThat(response.hasNotification()).isFalse();
        assertThat(response.getEvents()).isNull();
        assertThat(response.getNotification()).isNull();
    }

    @Test
    void response_with_events_only() {
        EventBook eventBook = makeEventBook();

        RejectionHandlerResponse response = RejectionHandlerResponse.withEvents(eventBook);

        assertThat(response.hasEvents()).isTrue();
        assertThat(response.hasNotification()).isFalse();
        assertThat(response.getEvents()).isEqualTo(eventBook);
        assertThat(response.getNotification()).isNull();
    }

    @Test
    void response_with_notification_only() {
        Notification notification = makeNotification("inventory", "ReserveStock", "out of stock");

        RejectionHandlerResponse response = RejectionHandlerResponse.withNotification(notification);

        assertThat(response.hasEvents()).isFalse();
        assertThat(response.hasNotification()).isTrue();
        assertThat(response.getEvents()).isNull();
        assertThat(response.getNotification()).isEqualTo(notification);
    }

    @Test
    void response_with_both_events_and_notification() {
        EventBook eventBook = makeEventBook();
        Notification notification = makeNotification("payment", "ProcessPayment", "declined");

        RejectionHandlerResponse response = RejectionHandlerResponse.withBoth(eventBook, notification);

        assertThat(response.hasEvents()).isTrue();
        assertThat(response.hasNotification()).isTrue();
        assertThat(response.getEvents()).isEqualTo(eventBook);
        assertThat(response.getNotification()).isEqualTo(notification);
    }

    @Test
    void response_events_pages_accessible() {
        EventBook eventBook = EventBook.newBuilder()
                .addPages(EventPage.newBuilder()
                        .setEvent(Any.newBuilder()
                                .setTypeUrl("type.googleapis.com/test.CompensationEvent1")
                                .build())
                        .build())
                .addPages(EventPage.newBuilder()
                        .setEvent(Any.newBuilder()
                                .setTypeUrl("type.googleapis.com/test.CompensationEvent2")
                                .build())
                        .build())
                .build();

        RejectionHandlerResponse response = RejectionHandlerResponse.withEvents(eventBook);

        assertThat(response.getEvents().getPagesCount()).isEqualTo(2);
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    private static EventBook makeEventBook() {
        return EventBook.newBuilder()
                .addPages(EventPage.newBuilder()
                        .setEvent(Any.newBuilder()
                                .setTypeUrl("type.googleapis.com/test.TestEvent")
                                .build())
                        .build())
                .build();
    }

    private static Notification makeNotification(String domain, String commandType, String reason) {
        CommandBook rejectedCommand = CommandBook.newBuilder()
                .setCover(Cover.newBuilder()
                        .setDomain(domain)
                        .build())
                .addPages(CommandPage.newBuilder()
                        .setCommand(Any.newBuilder()
                                .setTypeUrl("type.googleapis.com/test." + commandType)
                                .build())
                        .build())
                .build();

        RejectionNotification rejection = RejectionNotification.newBuilder()
                .setIssuerName("test-saga")
                .setIssuerType("saga")
                .setRejectionReason(reason)
                .setRejectedCommand(rejectedCommand)
                .build();

        return Notification.newBuilder()
                .setPayload(Any.pack(rejection))
                .build();
    }
}
