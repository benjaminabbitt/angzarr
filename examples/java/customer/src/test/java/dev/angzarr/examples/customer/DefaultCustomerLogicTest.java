package dev.angzarr.examples.customer;

import com.google.protobuf.Any;
import examples.Domains.CustomerCreated;
import examples.Domains.LoyaltyPointsAdded;
import examples.Domains.LoyaltyPointsRedeemed;
import io.grpc.Status;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class DefaultCustomerLogicTest {

    private CustomerLogic logic;

    @BeforeEach
    void setUp() {
        logic = new DefaultCustomerLogic();
    }

    // --- rebuildState tests ---

    @Test
    void test_rebuildState_null_returns_empty_state() {
        CustomerState state = logic.rebuildState(null);

        assertFalse(state.exists());
        assertEquals("", state.name());
        assertEquals("", state.email());
        assertEquals(0, state.loyaltyPoints());
        assertEquals(0, state.lifetimePoints());
    }

    @Test
    void test_rebuildState_empty_event_book_returns_empty_state() {
        EventBook eventBook = EventBook.newBuilder().build();

        CustomerState state = logic.rebuildState(eventBook);

        assertFalse(state.exists());
    }

    @Test
    void test_rebuildState_with_customer_created_event() {
        CustomerCreated event = CustomerCreated.newBuilder()
            .setName("John Doe")
            .setEmail("john@example.com")
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder()
                .setNum(0)
                .setEvent(Any.pack(event))
                .build())
            .build();

        CustomerState state = logic.rebuildState(eventBook);

        assertTrue(state.exists());
        assertEquals("John Doe", state.name());
        assertEquals("john@example.com", state.email());
        assertEquals(0, state.loyaltyPoints());
    }

    @Test
    void test_rebuildState_with_loyalty_points_added() {
        CustomerCreated created = CustomerCreated.newBuilder()
            .setName("John Doe")
            .setEmail("john@example.com")
            .build();

        LoyaltyPointsAdded added = LoyaltyPointsAdded.newBuilder()
            .setPoints(100)
            .setNewBalance(100)
            .setReason("welcome bonus")
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder()
                .setNum(0)
                .setEvent(Any.pack(created))
                .build())
            .addPages(EventPage.newBuilder()
                .setNum(1)
                .setEvent(Any.pack(added))
                .build())
            .build();

        CustomerState state = logic.rebuildState(eventBook);

        assertEquals(100, state.loyaltyPoints());
        assertEquals(100, state.lifetimePoints());
    }

    @Test
    void test_rebuildState_with_points_added_and_redeemed() {
        CustomerCreated created = CustomerCreated.newBuilder()
            .setName("John Doe")
            .setEmail("john@example.com")
            .build();

        LoyaltyPointsAdded added = LoyaltyPointsAdded.newBuilder()
            .setPoints(100)
            .setNewBalance(100)
            .build();

        LoyaltyPointsRedeemed redeemed = LoyaltyPointsRedeemed.newBuilder()
            .setPoints(30)
            .setNewBalance(70)
            .build();

        EventBook eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(added)).build())
            .addPages(EventPage.newBuilder().setNum(2).setEvent(Any.pack(redeemed)).build())
            .build();

        CustomerState state = logic.rebuildState(eventBook);

        assertEquals(70, state.loyaltyPoints());
        assertEquals(100, state.lifetimePoints()); // Lifetime not reduced
    }

    // --- handleCreateCustomer tests ---

    @Test
    void test_handleCreateCustomer_success() throws CommandValidationException {
        CustomerState state = CustomerState.empty();

        EventBook result = logic.handleCreateCustomer(state, "Jane Doe", "jane@example.com");

        assertNotNull(result);
        assertEquals(1, result.getPagesCount());

        Any eventAny = result.getPages(0).getEvent();
        assertTrue(eventAny.getTypeUrl().endsWith("CustomerCreated"));
    }

    @Test
    void test_handleCreateCustomer_already_exists_throws() {
        CustomerState state = new CustomerState("Existing", "existing@test.com", 0, 0);

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleCreateCustomer(state, "New Name", "new@test.com")
        );

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.getStatusCode());
        assertTrue(ex.getMessage().contains("already exists"));
    }

    @Test
    void test_handleCreateCustomer_empty_name_throws() {
        CustomerState state = CustomerState.empty();

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleCreateCustomer(state, "", "email@test.com")
        );

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.getStatusCode());
        assertTrue(ex.getMessage().contains("name"));
    }

    @Test
    void test_handleCreateCustomer_empty_email_throws() {
        CustomerState state = CustomerState.empty();

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleCreateCustomer(state, "Name", "")
        );

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.getStatusCode());
        assertTrue(ex.getMessage().contains("email"));
    }

    // --- handleAddLoyaltyPoints tests ---

    @Test
    void test_handleAddLoyaltyPoints_success() throws CommandValidationException {
        CustomerState state = new CustomerState("John", "john@test.com", 50, 100);

        EventBook result = logic.handleAddLoyaltyPoints(state, 25, "purchase");

        assertNotNull(result);
        assertEquals(1, result.getPagesCount());
        assertTrue(result.getPages(0).getEvent().getTypeUrl().endsWith("LoyaltyPointsAdded"));
    }

    @Test
    void test_handleAddLoyaltyPoints_customer_not_exists_throws() {
        CustomerState state = CustomerState.empty();

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleAddLoyaltyPoints(state, 25, "purchase")
        );

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.getStatusCode());
    }

    @Test
    void test_handleAddLoyaltyPoints_zero_points_throws() {
        CustomerState state = new CustomerState("John", "john@test.com", 50, 100);

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleAddLoyaltyPoints(state, 0, "purchase")
        );

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.getStatusCode());
    }

    @Test
    void test_handleAddLoyaltyPoints_negative_points_throws() {
        CustomerState state = new CustomerState("John", "john@test.com", 50, 100);

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleAddLoyaltyPoints(state, -10, "purchase")
        );

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.getStatusCode());
    }

    // --- handleRedeemLoyaltyPoints tests ---

    @Test
    void test_handleRedeemLoyaltyPoints_success() throws CommandValidationException {
        CustomerState state = new CustomerState("John", "john@test.com", 100, 200);

        EventBook result = logic.handleRedeemLoyaltyPoints(state, 50, "discount");

        assertNotNull(result);
        assertEquals(1, result.getPagesCount());
        assertTrue(result.getPages(0).getEvent().getTypeUrl().endsWith("LoyaltyPointsRedeemed"));
    }

    @Test
    void test_handleRedeemLoyaltyPoints_customer_not_exists_throws() {
        CustomerState state = CustomerState.empty();

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleRedeemLoyaltyPoints(state, 50, "discount")
        );

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.getStatusCode());
    }

    @Test
    void test_handleRedeemLoyaltyPoints_insufficient_points_throws() {
        CustomerState state = new CustomerState("John", "john@test.com", 30, 100);

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleRedeemLoyaltyPoints(state, 50, "discount")
        );

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.getStatusCode());
        assertTrue(ex.getMessage().contains("Insufficient"));
    }

    @Test
    void test_handleRedeemLoyaltyPoints_zero_points_throws() {
        CustomerState state = new CustomerState("John", "john@test.com", 100, 200);

        CommandValidationException ex = assertThrows(
            CommandValidationException.class,
            () -> logic.handleRedeemLoyaltyPoints(state, 0, "discount")
        );

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.getStatusCode());
    }
}
