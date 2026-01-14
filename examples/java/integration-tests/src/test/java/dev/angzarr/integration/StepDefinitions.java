package dev.angzarr.integration;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import examples.Domains.CreateCustomer;
import examples.Domains.CreateTransaction;
import examples.Domains.CompleteTransaction;
import examples.Domains.LineItem;
import examples.Domains.Receipt;
import io.cucumber.datatable.DataTable;
import io.cucumber.java.After;
import io.cucumber.java.Before;
import io.cucumber.java.en.And;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import dev.angzarr.BusinessCoordinatorGrpc;
import dev.angzarr.EventQueryGrpc;
import dev.angzarr.Angzarr.CommandBook;
import dev.angzarr.Angzarr.CommandPage;
import dev.angzarr.Angzarr.Cover;
import dev.angzarr.Angzarr.EventBook;
import dev.angzarr.Angzarr.Projection;
import dev.angzarr.Angzarr.Query;
import dev.angzarr.Angzarr.CommandResponse;
import dev.angzarr.Angzarr.UUID;

import java.nio.ByteBuffer;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.*;

public class StepDefinitions {

    private static final String CUSTOMER_DOMAIN = "customer";
    private static final String TRANSACTION_DOMAIN = "transaction";

    private final TestContext context = new TestContext();

    @Before
    public void setUp() {
        context.reset();
    }

    @After
    public void tearDown() {
        ManagedChannel channel = context.getChannel();
        if (channel != null && !channel.isShutdown()) {
            try {
                channel.shutdown().awaitTermination(5, TimeUnit.SECONDS);
            } catch (InterruptedException e) {
                channel.shutdownNow();
            }
        }
    }

    @Given("the angzarr system is running at {string}")
    public void theAngzarrSystemIsRunningAt(String hostPort) {
        String[] parts = hostPort.split(":");
        context.setAngzarrHost(parts[0]);
        context.setAngzarrPort(Integer.parseInt(parts[1]));

        ManagedChannel channel = ManagedChannelBuilder
                .forAddress(context.getAngzarrHost(), context.getAngzarrPort())
                .usePlaintext()
                .build();
        context.setChannel(channel);
    }

    @Given("a new customer id")
    public void aNewCustomerId() {
        context.setCurrentCustomerId(java.util.UUID.randomUUID());
    }

    @Given("a new transaction id for the customer")
    public void aNewTransactionIdForTheCustomer() {
        context.setCurrentTransactionId(java.util.UUID.randomUUID());
    }

    @When("I send a CreateCustomer command with name {string} and email {string}")
    public void iSendACreateCustomerCommandWithNameAndEmail(String name, String email) {
        CreateCustomer command = CreateCustomer.newBuilder()
                .setName(name)
                .setEmail(email)
                .build();

        sendCommand(CUSTOMER_DOMAIN, context.getCurrentCustomerId(), command);
    }

    @When("I send a CreateTransaction command with items:")
    public void iSendACreateTransactionCommandWithItems(DataTable dataTable) {
        List<Map<String, String>> rows = dataTable.asMaps();
        CreateTransaction.Builder builder = CreateTransaction.newBuilder()
                .setCustomerId(context.getCurrentCustomerId().toString());

        for (Map<String, String> row : rows) {
            LineItem item = LineItem.newBuilder()
                    .setProductId(row.get("product_id"))
                    .setName(row.get("name"))
                    .setQuantity(Integer.parseInt(row.get("quantity")))
                    .setUnitPriceCents(Integer.parseInt(row.get("unit_price_cents")))
                    .build();
            builder.addItems(item);
        }

        sendCommand(TRANSACTION_DOMAIN, context.getCurrentTransactionId(), builder.build());
    }

    @When("I send a CompleteTransaction command with payment method {string}")
    public void iSendACompleteTransactionCommandWithPaymentMethod(String paymentMethod) {
        CompleteTransaction command = CompleteTransaction.newBuilder()
                .setPaymentMethod(paymentMethod)
                .build();

        sendCommand(TRANSACTION_DOMAIN, context.getCurrentTransactionId(), command);
    }

    @When("I query events for the customer aggregate")
    public void iQueryEventsForTheCustomerAggregate() {
        queryEvents(CUSTOMER_DOMAIN, context.getCurrentCustomerId());
    }

    @Then("the command succeeds")
    public void theCommandSucceeds() {
        assertNull(context.getLastException(), "Expected command to succeed but got: " + context.getLastException());
        assertNotNull(context.getLastResponse(), "Expected a response but got null");
    }

    @Then("the customer aggregate has {int} event(s)")
    public void theCustomerAggregateHasEvents(int expectedCount) {
        int eventCount = getEventCount(CUSTOMER_DOMAIN, context.getCurrentCustomerId());
        assertEquals(expectedCount, eventCount, "Customer aggregate event count mismatch");
    }

    @Then("the transaction aggregate has {int} event(s)")
    public void theTransactionAggregateHasEvents(int expectedCount) {
        int eventCount = getEventCount(TRANSACTION_DOMAIN, context.getCurrentTransactionId());
        assertEquals(expectedCount, eventCount, "Transaction aggregate event count mismatch");
    }

    @Then("the latest event type is {string}")
    public void theLatestEventTypeIs(String expectedType) {
        CommandResponse response = context.getLastResponse();
        assertNotNull(response, "No response available");
        assertTrue(response.hasEvents(), "No events in response");

        EventBook book = response.getEvents();
        assertFalse(book.getPagesList().isEmpty(), "No events in book");

        String eventTypeUrl = book.getPages(book.getPagesCount() - 1).getEvent().getTypeUrl();
        String actualType = eventTypeUrl.substring(eventTypeUrl.lastIndexOf('.') + 1);
        assertEquals(expectedType, actualType, "Event type mismatch");
    }

    @Then("a projection was returned from projector {string}")
    public void aProjectionWasReturnedFromProjector(String projectorName) {
        CommandResponse response = context.getLastResponse();
        assertNotNull(response, "No response available");

        boolean found = response.getProjectionsList().stream()
                .anyMatch(p -> p.getProjector().equals(projectorName));
        assertTrue(found, "No projection from projector: " + projectorName);
    }

    @Then("the projection contains a Receipt with total {int} cents")
    public void theProjectionContainsAReceiptWithTotalCents(int expectedTotal) throws Exception {
        CommandResponse response = context.getLastResponse();
        assertNotNull(response, "No response available");

        Projection receiptProjection = response.getProjectionsList().stream()
                .filter(p -> p.getProjector().equals("receipt"))
                .findFirst()
                .orElseThrow(() -> new AssertionError("No receipt projection found"));

        Receipt receipt = receiptProjection.getProjection().unpack(Receipt.class);
        assertEquals(expectedTotal, receipt.getFinalTotalCents(), "Receipt total mismatch");
    }

    @Then("I receive {int} event(s)")
    public void iReceiveEvents(int expectedCount) {
        EventBook book = context.getLastEventBook();
        assertNotNull(book, "No event book available");
        assertEquals(expectedCount, book.getPagesCount(), "Event count mismatch");
    }

    @Then("the event at sequence {int} has type {string}")
    public void theEventAtSequenceHasType(int sequence, String expectedType) {
        EventBook book = context.getLastEventBook();
        assertNotNull(book, "No event book available");
        assertTrue(sequence < book.getPagesCount(), "Sequence out of bounds");

        String eventTypeUrl = book.getPages(sequence).getEvent().getTypeUrl();
        String actualType = eventTypeUrl.substring(eventTypeUrl.lastIndexOf('.') + 1);
        assertEquals(expectedType, actualType, "Event type mismatch at sequence " + sequence);
    }

    private void sendCommand(String domain, java.util.UUID aggregateId, com.google.protobuf.Message command) {
        try {
            BusinessCoordinatorGrpc.BusinessCoordinatorBlockingStub stub =
                    BusinessCoordinatorGrpc.newBlockingStub(context.getChannel());

            CommandBook commandBook = CommandBook.newBuilder()
                    .setCover(Cover.newBuilder()
                            .setDomain(domain)
                            .setRoot(toProtoUUID(aggregateId))
                            .build())
                    .addPages(CommandPage.newBuilder()
                            .setSequence(0)
                            .setSynchronous(true)
                            .setCommand(Any.pack(command))
                            .build())
                    .build();

            CommandResponse response = stub.handle(commandBook);
            context.setLastResponse(response);
            context.setLastException(null);
        } catch (Exception e) {
            context.setLastException(e);
            context.setLastResponse(null);
        }
    }

    private void queryEvents(String domain, java.util.UUID aggregateId) {
        try {
            EventQueryGrpc.EventQueryBlockingStub stub =
                    EventQueryGrpc.newBlockingStub(context.getChannel());

            Query query = Query.newBuilder()
                    .setDomain(domain)
                    .setRoot(toProtoUUID(aggregateId))
                    .build();

            Iterator<EventBook> results = stub.getEvents(query);
            if (results.hasNext()) {
                context.setLastEventBook(results.next());
            }
            context.setLastException(null);
        } catch (Exception e) {
            context.setLastException(e);
            context.setLastEventBook(null);
        }
    }

    private int getEventCount(String domain, java.util.UUID aggregateId) {
        try {
            EventQueryGrpc.EventQueryBlockingStub stub =
                    EventQueryGrpc.newBlockingStub(context.getChannel());

            Query query = Query.newBuilder()
                    .setDomain(domain)
                    .setRoot(toProtoUUID(aggregateId))
                    .build();

            Iterator<EventBook> results = stub.getEvents(query);
            if (results.hasNext()) {
                return results.next().getPagesCount();
            }
            return 0;
        } catch (Exception e) {
            return 0;
        }
    }

    private UUID toProtoUUID(java.util.UUID uuid) {
        ByteBuffer buffer = ByteBuffer.allocate(16);
        buffer.putLong(uuid.getMostSignificantBits());
        buffer.putLong(uuid.getLeastSignificantBits());
        return UUID.newBuilder()
                .setValue(ByteString.copyFrom(buffer.array()))
                .build();
    }
}
