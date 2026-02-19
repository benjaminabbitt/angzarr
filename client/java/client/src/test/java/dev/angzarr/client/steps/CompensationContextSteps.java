package dev.angzarr.client.steps;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import dev.angzarr.CommandBook;
import dev.angzarr.CommandPage;
import dev.angzarr.Cover;
import dev.angzarr.Notification;
import dev.angzarr.RejectionNotification;
import dev.angzarr.client.compensation.CompensationContext;
import io.cucumber.datatable.DataTable;
import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for CompensationContext feature tests.
 *
 * Tests extraction of rejection details from Notification messages.
 * These are unit tests that don't require a running gRPC server.
 */
public class CompensationContextSteps {

    private Notification notification;
    private CompensationContext context;

    @Before
    public void setup() {
        notification = null;
        context = null;
    }

    // --- Given steps ---

    @Given("a Notification containing a RejectionNotification with:")
    public void aNotificationContainingARejectionNotificationWith(DataTable dataTable) {
        Map<String, String> fields = dataTable.asMap(String.class, String.class);

        RejectionNotification.Builder rejectionBuilder = RejectionNotification.newBuilder();

        if (fields.containsKey("issuer_name")) {
            rejectionBuilder.setIssuerName(fields.get("issuer_name"));
        }
        if (fields.containsKey("issuer_type")) {
            rejectionBuilder.setIssuerType(fields.get("issuer_type"));
        }
        if (fields.containsKey("source_event_sequence")) {
            rejectionBuilder.setSourceEventSequence(Integer.parseInt(fields.get("source_event_sequence")));
        }
        if (fields.containsKey("rejection_reason")) {
            rejectionBuilder.setRejectionReason(fields.get("rejection_reason"));
        }

        RejectionNotification rejection = rejectionBuilder.build();
        Any payload = Any.pack(rejection);

        notification = Notification.newBuilder()
            .setPayload(payload)
            .build();
    }

    @Given("a Notification with a rejected command of type {string}")
    public void aNotificationWithARejectedCommandOfType(String commandType) {
        // Create a command with the specified type
        Any command = Any.newBuilder()
            .setTypeUrl("type.googleapis.com/test." + commandType)
            .setValue(ByteString.EMPTY)
            .build();

        CommandPage page = CommandPage.newBuilder()
            .setCommand(command)
            .build();

        CommandBook rejectedCommand = CommandBook.newBuilder()
            .setCover(Cover.newBuilder().setDomain("test"))
            .addPages(page)
            .build();

        RejectionNotification rejection = RejectionNotification.newBuilder()
            .setRejectedCommand(rejectedCommand)
            .build();

        notification = Notification.newBuilder()
            .setPayload(Any.pack(rejection))
            .build();
    }

    @Given("a Notification with source_aggregate cover for domain {string}")
    public void aNotificationWithSourceAggregateCoverForDomain(String domain) {
        Cover sourceAggregate = Cover.newBuilder()
            .setDomain(domain)
            .build();

        RejectionNotification rejection = RejectionNotification.newBuilder()
            .setSourceAggregate(sourceAggregate)
            .build();

        notification = Notification.newBuilder()
            .setPayload(Any.pack(rejection))
            .build();
    }

    @Given("a Notification without a rejected command")
    public void aNotificationWithoutARejectedCommand() {
        RejectionNotification rejection = RejectionNotification.newBuilder()
            .setIssuerName("test-saga")
            .setRejectionReason("some reason")
            // No rejected_command set
            .build();

        notification = Notification.newBuilder()
            .setPayload(Any.pack(rejection))
            .build();
    }

    @Given("a Notification with empty payload")
    public void aNotificationWithEmptyPayload() {
        notification = Notification.newBuilder().build();
    }

    // --- When steps ---

    @When("I create a CompensationContext from the Notification")
    public void iCreateACompensationContextFromTheNotification() {
        context = CompensationContext.from(notification);
    }

    // --- Then steps ---

    @Then("the CompensationContext should have:")
    public void theCompensationContextShouldHave(DataTable dataTable) {
        Map<String, String> expected = dataTable.asMap(String.class, String.class);

        if (expected.containsKey("issuer_name")) {
            assertThat(context.getIssuerName()).isEqualTo(expected.get("issuer_name"));
        }
        if (expected.containsKey("issuer_type")) {
            assertThat(context.getIssuerType()).isEqualTo(expected.get("issuer_type"));
        }
        if (expected.containsKey("source_event_sequence")) {
            assertThat(context.getSourceEventSequence())
                .isEqualTo(Integer.parseInt(expected.get("source_event_sequence")));
        }
        if (expected.containsKey("rejection_reason")) {
            assertThat(context.getRejectionReason()).isEqualTo(expected.get("rejection_reason"));
        }
    }

    @Then("the rejected_command_type should end with {string}")
    public void theRejectedCommandTypeShouldEndWith(String suffix) {
        String commandType = context.getRejectedCommandType();
        assertThat(commandType).isNotNull();
        assertThat(commandType).endsWith(suffix);
    }

    @Then("the source_aggregate should have domain {string}")
    public void theSourceAggregateShouldHaveDomain(String domain) {
        assertThat(context.getSourceAggregate()).isNotNull();
        assertThat(context.getSourceAggregate().getDomain()).isEqualTo(domain);
    }

    @Then("rejected_command should be null")
    public void rejectedCommandShouldBeNull() {
        assertThat(context.getRejectedCommand()).isNull();
    }

    @Then("rejected_command_type should return null")
    public void rejectedCommandTypeShouldReturnNull() {
        assertThat(context.getRejectedCommandType()).isNull();
    }

    @Then("all fields should have default\\/empty values")
    public void allFieldsShouldHaveDefaultEmptyValues() {
        assertThat(context.getIssuerName()).isEmpty();
        assertThat(context.getIssuerType()).isEmpty();
        assertThat(context.getSourceEventSequence()).isEqualTo(0);
        assertThat(context.getRejectionReason()).isEmpty();
        assertThat(context.getRejectedCommand()).isNull();
        assertThat(context.getSourceAggregate()).isNull();
    }
}
