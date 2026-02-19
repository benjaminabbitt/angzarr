package dev.angzarr.client.steps;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.Duration;
import dev.angzarr.CommandBook;
import dev.angzarr.client.CommandBuilder;
import dev.angzarr.client.Helpers;
import io.cucumber.datatable.DataTable;
import io.cucumber.java.Before;
import io.cucumber.java.en.And;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.util.Map;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for CommandBuilder feature tests.
 *
 * Tests the fluent builder API for constructing CommandBook messages.
 * These are unit tests that don't require a running gRPC server.
 */
public class CommandBuilderSteps {

    private CommandBuilder builder;
    private CommandBook commandBook;
    private UUID testRoot;

    @Before
    public void setup() {
        builder = null;
        commandBook = null;
        testRoot = null;
    }

    // --- Background ---

    @Given("an AggregateClient connected to the coordinator")
    public void anAggregateClientConnectedToCoordinator() {
        // For build() tests, we don't need an actual client.
        // The builder accepts null client when only building, not executing.
    }

    // --- When steps ---

    @When("I build a command using CommandBuilder:")
    public void iBuildACommandUsingCommandBuilder(DataTable dataTable) {
        Map<String, String> fields = dataTable.asMap(String.class, String.class);

        String domain = fields.get("domain");
        String rootStr = fields.get("root");
        String correlationId = fields.get("correlation_id");
        String sequenceStr = fields.get("sequence");

        testRoot = rootStr != null ? UUID.fromString(rootStr) : null;

        builder = testRoot != null
            ? new CommandBuilder(null, domain, testRoot)
            : new CommandBuilder(null, domain);

        if (correlationId != null) {
            builder.withCorrelationId(correlationId);
        }
        if (sequenceStr != null) {
            builder.withSequence(Integer.parseInt(sequenceStr));
        }

        // Add a dummy command payload for build to succeed
        Duration dummyMessage = Duration.newBuilder().setSeconds(1).build();
        builder.withCommand("type.googleapis.com/test.TestCommand", dummyMessage);

        commandBook = builder.build();
    }

    @When("I build a command without specifying correlation_id")
    public void iBuildACommandWithoutSpecifyingCorrelationId() {
        Duration dummyMessage = Duration.newBuilder().setSeconds(1).build();
        builder = new CommandBuilder(null, "test")
            .withCommand("type.googleapis.com/test.TestCommand", dummyMessage);
        commandBook = builder.build();
    }

    @When("I build a command for domain {string} without specifying root")
    public void iBuildACommandForDomainWithoutSpecifyingRoot(String domain) {
        Duration dummyMessage = Duration.newBuilder().setSeconds(1).build();
        builder = new CommandBuilder(null, domain)
            .withCommand("type.googleapis.com/test.TestCommand", dummyMessage);
        commandBook = builder.build();
    }

    @When("I create a CommandBuilder for domain {string}")
    public void iCreateACommandBuilderForDomain(String domain) {
        builder = new CommandBuilder(null, domain);
    }

    @When("I chain with_correlation_id {string}")
    public void iChainWithCorrelationId(String correlationId) {
        builder = builder.withCorrelationId(correlationId);
    }

    @When("I chain with_sequence {int}")
    public void iChainWithSequence(int sequence) {
        builder = builder.withSequence(sequence);
    }

    @When("I chain with_command for a TestCommand message")
    public void iChainWithCommandForATestCommandMessage() {
        Duration dummyMessage = Duration.newBuilder().setSeconds(42).build();
        builder = builder.withCommand("type.googleapis.com/test.TestCommand", dummyMessage);
    }

    @When("I call build")
    public void iCallBuild() {
        commandBook = builder.build();
    }

    @When("I build a command without specifying sequence")
    public void iBuildACommandWithoutSpecifyingSequence() {
        Duration dummyMessage = Duration.newBuilder().setSeconds(1).build();
        builder = new CommandBuilder(null, "test")
            .withCommand("type.googleapis.com/test.TestCommand", dummyMessage);
        commandBook = builder.build();
    }

    @When("I build a command with_command {string} and message")
    public void iBuildACommandWithCommandAndMessage(String typeUrl) {
        Duration dummyMessage = Duration.newBuilder().setSeconds(42).build();
        builder = new CommandBuilder(null, "test")
            .withCommand(typeUrl, dummyMessage);
        commandBook = builder.build();
    }

    // --- Then steps ---

    @Then("the resulting CommandBook should have:")
    public void theResultingCommandBookShouldHave(DataTable dataTable) {
        Map<String, String> expected = dataTable.asMap(String.class, String.class);

        if (expected.containsKey("domain")) {
            assertThat(commandBook.getCover().getDomain()).isEqualTo(expected.get("domain"));
        }
        if (expected.containsKey("root")) {
            UUID expectedRoot = UUID.fromString(expected.get("root"));
            UUID actualRoot = Helpers.protoToUuid(commandBook.getCover().getRoot());
            assertThat(actualRoot).isEqualTo(expectedRoot);
        }
        if (expected.containsKey("correlation_id")) {
            assertThat(commandBook.getCover().getCorrelationId()).isEqualTo(expected.get("correlation_id"));
        }
        if (expected.containsKey("sequence")) {
            int expectedSeq = Integer.parseInt(expected.get("sequence"));
            assertThat(commandBook.getPages(0).getSequence()).isEqualTo(expectedSeq);
        }
        if (expected.containsKey("type_url")) {
            assertThat(commandBook.getPages(0).getCommand().getTypeUrl()).isEqualTo(expected.get("type_url"));
        }
    }

    @Then("the resulting CommandBook should have a non-empty correlation_id")
    public void theResultingCommandBookShouldHaveANonEmptyCorrelationId() {
        assertThat(commandBook.getCover().getCorrelationId()).isNotNull();
        assertThat(commandBook.getCover().getCorrelationId()).isNotEmpty();
    }

    @Then("the resulting CommandBook should have no root UUID")
    public void theResultingCommandBookShouldHaveNoRootUuid() {
        assertThat(commandBook.getCover().hasRoot()).isFalse();
    }

    @Then("the CommandBook should reflect all chained values")
    public void theCommandBookShouldReflectAllChainedValues() {
        assertThat(commandBook.getCover().getCorrelationId()).isEqualTo("chain-test");
        assertThat(commandBook.getPages(0).getSequence()).isEqualTo(10);
        assertThat(commandBook.getPages(0).getCommand().getTypeUrl())
            .isEqualTo("type.googleapis.com/test.TestCommand");
    }

    @Then("the resulting CommandBook should have sequence {int}")
    public void theResultingCommandBookShouldHaveSequence(int expectedSequence) {
        assertThat(commandBook.getPages(0).getSequence()).isEqualTo(expectedSequence);
    }

    @Then("the payload should be correctly serialized")
    public void thePayloadShouldBeCorrectlySerialized() {
        ByteString payload = commandBook.getPages(0).getCommand().getValue();
        assertThat(payload).isNotNull();
        assertThat(payload.size()).isGreaterThan(0);
    }

    // --- Skipped scenarios that need real gRPC ---

    @Given("a registered aggregate handler for domain {string}")
    public void aRegisteredAggregateHandlerForDomain(String domain) {
        // Skip - requires real gRPC server
        org.junit.jupiter.api.Assumptions.assumeTrue(false,
            "Skipping: requires running gRPC server");
    }

    @When("I use CommandBuilder to build and execute a command")
    public void iUseCommandBuilderToBuildAndExecuteACommand() {
        // Skip - requires real gRPC server
        org.junit.jupiter.api.Assumptions.assumeTrue(false,
            "Skipping: requires running gRPC server");
    }

    @Then("I should receive a CommandResponse")
    public void iShouldReceiveACommandResponse() {
        // Skip - requires real gRPC server
    }
}
