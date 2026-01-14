package dev.angzarr.examples.projector;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import examples.Domains.*;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.Projection;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class DefaultReceiptProjectorTest {

    private ReceiptProjector projector;

    @BeforeEach
    void setUp() {
        projector = new DefaultReceiptProjector();
    }

    private LineItem createItem(String name, int quantity, int priceCents) {
        return LineItem.newBuilder()
            .setProductId("prod-" + name)
            .setName(name)
            .setQuantity(quantity)
            .setUnitPriceCents(priceCents)
            .build();
    }

    @Test
    void test_project_null_returns_null() {
        Projection result = projector.project(null);
        assertNull(result);
    }

    @Test
    void test_project_empty_event_book_returns_null() {
        EventBook eventBook = EventBook.newBuilder().build();
        Projection result = projector.project(eventBook);
        assertNull(result);
    }

    @Test
    void test_project_incomplete_transaction_returns_null() {
        TransactionCreated created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .addItems(createItem("Widget", 2, 1000))
            .setSubtotalCents(2000)
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder()
                .setNum(0)
                .setEvent(Any.pack(created))
                .build())
            .build();

        Projection result = projector.project(eventBook);
        assertNull(result);
    }

    @Test
    void test_project_completed_transaction_returns_receipt() throws InvalidProtocolBufferException {
        TransactionCreated created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .addItems(createItem("Widget", 2, 1000))
            .setSubtotalCents(2000)
            .build();

        TransactionCompleted completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(2000)
            .setPaymentMethod("card")
            .setLoyaltyPointsEarned(20)
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(completed)).build())
            .build();

        Projection result = projector.project(eventBook);

        assertNotNull(result);
        assertEquals("receipt", result.getProjector());

        Receipt receipt = result.getProjection().unpack(Receipt.class);
        assertEquals("cust-123", receipt.getCustomerId());
        assertEquals(2000, receipt.getFinalTotalCents());
        assertEquals("card", receipt.getPaymentMethod());
        assertEquals(20, receipt.getLoyaltyPointsEarned());
        assertFalse(receipt.getFormattedText().isEmpty());
    }

    @Test
    void test_project_with_discount_includes_discount_in_receipt() throws InvalidProtocolBufferException {
        TransactionCreated created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .addItems(createItem("Widget", 1, 5000))
            .setSubtotalCents(5000)
            .build();

        DiscountApplied discount = DiscountApplied.newBuilder()
            .setDiscountType("percentage")
            .setValue(10)
            .setDiscountCents(500)
            .build();

        TransactionCompleted completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(4500)
            .setPaymentMethod("cash")
            .setLoyaltyPointsEarned(45)
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(discount)).build())
            .addPages(EventPage.newBuilder().setNum(2).setEvent(Any.pack(completed)).build())
            .build();

        Projection result = projector.project(eventBook);

        assertNotNull(result);
        Receipt receipt = result.getProjection().unpack(Receipt.class);
        assertEquals(5000, receipt.getSubtotalCents());
        assertEquals(500, receipt.getDiscountCents());
        assertEquals(4500, receipt.getFinalTotalCents());
        assertTrue(receipt.getFormattedText().contains("Discount"));
    }
}
