package dev.angzarr.examples.saga;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.InvalidProtocolBufferException;
import examples.Domains.AddLoyaltyPoints;
import examples.Domains.TransactionCompleted;
import examples.Domains.TransactionCreated;
import dev.angzarr.*;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class DefaultLoyaltySagaTest {

    private LoyaltySaga saga;

    @BeforeEach
    void setUp() {
        saga = new DefaultLoyaltySaga();
    }

    private EventBook createEventBookWithCover(UUID rootId, EventPage... pages) {
        EventBook.Builder builder = EventBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain("transaction")
                .setRoot(rootId)
                .build());

        for (EventPage page : pages) {
            builder.addPages(page);
        }

        return builder.build();
    }

    private UUID createUUID() {
        byte[] bytes = new byte[16];
        for (int i = 0; i < 16; i++) {
            bytes[i] = (byte) (i + 1);
        }
        return UUID.newBuilder()
            .setValue(ByteString.copyFrom(bytes))
            .build();
    }

    @Test
    void test_processEvents_null_returns_empty() {
        List<CommandBook> result = saga.processEvents(null);
        assertTrue(result.isEmpty());
    }

    @Test
    void test_processEvents_empty_event_book_returns_empty() {
        EventBook eventBook = EventBook.newBuilder().build();
        List<CommandBook> result = saga.processEvents(eventBook);
        assertTrue(result.isEmpty());
    }

    @Test
    void test_processEvents_no_transaction_completed_returns_empty() {
        TransactionCreated created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .setSubtotalCents(1000)
            .build();

        EventBook eventBook = createEventBookWithCover(
            createUUID(),
            EventPage.newBuilder()
                .setNum(0)
                .setEvent(Any.pack(created))
                .build()
        );

        List<CommandBook> result = saga.processEvents(eventBook);
        assertTrue(result.isEmpty());
    }

    @Test
    void test_processEvents_completed_with_zero_points_returns_empty() {
        TransactionCompleted completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(50) // Less than $1
            .setLoyaltyPointsEarned(0)
            .build();

        EventBook eventBook = createEventBookWithCover(
            createUUID(),
            EventPage.newBuilder()
                .setNum(0)
                .setEvent(Any.pack(completed))
                .build()
        );

        List<CommandBook> result = saga.processEvents(eventBook);
        assertTrue(result.isEmpty());
    }

    @Test
    void test_processEvents_completed_with_points_generates_command() throws InvalidProtocolBufferException {
        TransactionCompleted completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(5000)
            .setLoyaltyPointsEarned(50)
            .setPaymentMethod("card")
            .build();

        UUID customerId = createUUID();
        EventBook eventBook = createEventBookWithCover(
            customerId,
            EventPage.newBuilder()
                .setNum(0)
                .setEvent(Any.pack(completed))
                .build()
        );

        List<CommandBook> result = saga.processEvents(eventBook);

        assertEquals(1, result.size());

        CommandBook cmdBook = result.get(0);
        assertEquals("customer", cmdBook.getCover().getDomain());
        assertEquals(customerId, cmdBook.getCover().getRoot());

        assertEquals(1, cmdBook.getPagesCount());
        Any commandAny = cmdBook.getPages(0).getCommand();
        assertTrue(commandAny.getTypeUrl().endsWith("AddLoyaltyPoints"));

        AddLoyaltyPoints addPoints = commandAny.unpack(AddLoyaltyPoints.class);
        assertEquals(50, addPoints.getPoints());
        assertTrue(addPoints.getReason().startsWith("transaction:"));
    }

    @Test
    void test_processEvents_multiple_completed_events_generates_multiple_commands() {
        TransactionCompleted completed1 = TransactionCompleted.newBuilder()
            .setFinalTotalCents(3000)
            .setLoyaltyPointsEarned(30)
            .build();

        TransactionCompleted completed2 = TransactionCompleted.newBuilder()
            .setFinalTotalCents(2000)
            .setLoyaltyPointsEarned(20)
            .build();

        UUID customerId = createUUID();
        EventBook eventBook = createEventBookWithCover(
            customerId,
            EventPage.newBuilder().setNum(0).setEvent(Any.pack(completed1)).build(),
            EventPage.newBuilder().setNum(1).setEvent(Any.pack(completed2)).build()
        );

        List<CommandBook> result = saga.processEvents(eventBook);

        assertEquals(2, result.size());
    }
}
