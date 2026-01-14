package dev.angzarr.examples.transaction;

import com.google.protobuf.Any;
import examples.Domains.*;
import io.grpc.Status;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class DefaultTransactionLogicTest {

    private TransactionLogic logic;

    @BeforeEach
    void setUp() {
        logic = new DefaultTransactionLogic();
    }

    private LineItem createItem(String name, int quantity, int priceCents) {
        return LineItem.newBuilder()
            .setProductId("prod-" + name)
            .setName(name)
            .setQuantity(quantity)
            .setUnitPriceCents(priceCents)
            .build();
    }

    // --- rebuildState tests ---

    @Test
    void test_rebuildState_null_returns_new_state() {
        TransactionState state = logic.rebuildState(null);

        assertTrue(state.isNew());
        assertEquals(TransactionState.Status.NEW, state.status());
    }

    @Test
    void test_rebuildState_with_transaction_created() {
        TransactionCreated event = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .addItems(createItem("Widget", 2, 1000))
            .setSubtotalCents(2000)
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder()
                .setNum(0)
                .setEvent(Any.pack(event))
                .build())
            .build();

        TransactionState state = logic.rebuildState(eventBook);

        assertTrue(state.isPending());
        assertEquals("cust-123", state.customerId());
        assertEquals(2000, state.subtotalCents());
        assertEquals(1, state.items().size());
    }

    @Test
    void test_rebuildState_with_discount_applied() {
        TransactionCreated created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .setSubtotalCents(2000)
            .build();

        DiscountApplied discount = DiscountApplied.newBuilder()
            .setDiscountType("percentage")
            .setValue(10)
            .setDiscountCents(200)
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(discount)).build())
            .build();

        TransactionState state = logic.rebuildState(eventBook);

        assertEquals(200, state.discountCents());
        assertEquals("percentage", state.discountType());
    }

    @Test
    void test_rebuildState_with_completed() {
        TransactionCreated created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .setSubtotalCents(2000)
            .build();

        TransactionCompleted completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(2000)
            .setPaymentMethod("card")
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(completed)).build())
            .build();

        TransactionState state = logic.rebuildState(eventBook);

        assertEquals(TransactionState.Status.COMPLETED, state.status());
    }

    // --- handleCreateTransaction tests ---

    @Test
    void test_handleCreateTransaction_success() throws CommandValidationException {
        TransactionState state = TransactionState.empty();
        List<LineItem> items = List.of(createItem("Widget", 2, 1000));

        EventBook result = logic.handleCreateTransaction(state, "cust-123", items);

        assertNotNull(result);
        assertEquals(1, result.getPagesCount());
        assertTrue(result.getPages(0).getEvent().getTypeUrl().endsWith("TransactionCreated"));
    }

    @Test
    void test_handleCreateTransaction_already_exists_throws() {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 1000, 0, "", TransactionState.Status.PENDING
        );

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleCreateTransaction(state, "cust-456", List.of(createItem("X", 1, 100)))
        );

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.getStatusCode());
    }

    @Test
    void test_handleCreateTransaction_no_customer_throws() {
        TransactionState state = TransactionState.empty();

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleCreateTransaction(state, "", List.of(createItem("X", 1, 100)))
        );

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.getStatusCode());
    }

    @Test
    void test_handleCreateTransaction_no_items_throws() {
        TransactionState state = TransactionState.empty();

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleCreateTransaction(state, "cust-123", List.of())
        );

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.getStatusCode());
    }

    // --- handleApplyDiscount tests ---

    @Test
    void test_handleApplyDiscount_percentage_success() throws CommandValidationException {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 10000, 0, "", TransactionState.Status.PENDING
        );

        EventBook result = logic.handleApplyDiscount(state, "percentage", 20, null);

        assertNotNull(result);
        assertTrue(result.getPages(0).getEvent().getTypeUrl().endsWith("DiscountApplied"));
    }

    @Test
    void test_handleApplyDiscount_fixed_success() throws CommandValidationException {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 10000, 0, "", TransactionState.Status.PENDING
        );

        EventBook result = logic.handleApplyDiscount(state, "fixed", 500, null);

        assertNotNull(result);
    }

    @Test
    void test_handleApplyDiscount_not_pending_throws() {
        TransactionState state = TransactionState.empty();

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleApplyDiscount(state, "percentage", 10, null)
        );

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.getStatusCode());
    }

    @Test
    void test_handleApplyDiscount_invalid_percentage_throws() {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 10000, 0, "", TransactionState.Status.PENDING
        );

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleApplyDiscount(state, "percentage", 150, null)
        );

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.getStatusCode());
    }

    // --- handleCompleteTransaction tests ---

    @Test
    void test_handleCompleteTransaction_success() throws CommandValidationException {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 5000, 500, "fixed", TransactionState.Status.PENDING
        );

        EventBook result = logic.handleCompleteTransaction(state, "card");

        assertNotNull(result);
        assertTrue(result.getPages(0).getEvent().getTypeUrl().endsWith("TransactionCompleted"));
    }

    @Test
    void test_handleCompleteTransaction_not_pending_throws() {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 5000, 0, "", TransactionState.Status.COMPLETED
        );

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleCompleteTransaction(state, "card")
        );

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.getStatusCode());
    }

    // --- handleCancelTransaction tests ---

    @Test
    void test_handleCancelTransaction_success() throws CommandValidationException {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 5000, 0, "", TransactionState.Status.PENDING
        );

        EventBook result = logic.handleCancelTransaction(state, "Customer requested");

        assertNotNull(result);
        assertTrue(result.getPages(0).getEvent().getTypeUrl().endsWith("TransactionCancelled"));
    }

    @Test
    void test_handleCancelTransaction_not_pending_throws() {
        TransactionState state = TransactionState.empty();

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleCancelTransaction(state, "reason")
        );

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.getStatusCode());
    }

    // --- State calculation tests ---

    @Test
    void test_calculateFinalTotal() {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 5000, 500, "fixed", TransactionState.Status.PENDING
        );

        assertEquals(4500, state.calculateFinalTotal());
    }

    @Test
    void test_calculateLoyaltyPoints() {
        TransactionState state = new TransactionState(
            "cust-123", List.of(), 5000, 500, "fixed", TransactionState.Status.PENDING
        );

        assertEquals(45, state.calculateLoyaltyPoints()); // 4500 cents = $45 = 45 points
    }
}
