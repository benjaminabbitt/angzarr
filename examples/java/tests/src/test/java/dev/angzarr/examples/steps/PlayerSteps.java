package dev.angzarr.examples.steps;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.Message;
import dev.angzarr.Cover;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.player.Player;
import dev.angzarr.examples.Currency;
import dev.angzarr.examples.DepositFunds;
import dev.angzarr.examples.FundsDeposited;
import dev.angzarr.examples.FundsReleased;
import dev.angzarr.examples.FundsReserved;
import dev.angzarr.examples.FundsWithdrawn;
import dev.angzarr.examples.PlayerRegistered;
import dev.angzarr.examples.PlayerType;
import dev.angzarr.examples.RegisterPlayer;
import dev.angzarr.examples.ReleaseFunds;
import dev.angzarr.examples.ReserveFunds;
import dev.angzarr.examples.WithdrawFunds;
import io.cucumber.java.Before;
import io.cucumber.java.en.And;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;
import io.grpc.Status;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Cucumber step definitions for Player aggregate tests.
 */
public class PlayerSteps {

    private Player player;
    private List<EventPage> eventPages;
    private Message resultEvent;
    private Errors.CommandRejectedError rejectedError;

    @Before
    public void setup() {
        player = new Player();
        eventPages = new ArrayList<>();
        resultEvent = null;
        rejectedError = null;
    }

    // --- Given steps ---

    @Given("no prior events for the player aggregate")
    public void noPriorEvents() {
        eventPages.clear();
        rehydratePlayer();
    }

    @Given("a PlayerRegistered event for {string}")
    public void playerRegisteredEventFor(String name) {
        PlayerRegistered event = PlayerRegistered.newBuilder()
            .setDisplayName(name)
            .setEmail(name.toLowerCase() + "@example.com")
            .setPlayerType(PlayerType.HUMAN)
            .build();
        addEvent(event);
        rehydratePlayer();
    }

    @Given("a FundsDeposited event with amount {int}")
    public void fundsDepositedEventWithAmount(int amount) {
        // Calculate new balance based on current state
        long currentBankroll = player.getBankroll();
        FundsDeposited event = FundsDeposited.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(amount).setCurrencyCode("CHIPS"))
            .setNewBalance(Currency.newBuilder().setAmount(currentBankroll + amount).setCurrencyCode("CHIPS"))
            .build();
        addEvent(event);
        rehydratePlayer();
    }

    @Given("a FundsReserved event with amount {int} for table {string}")
    public void fundsReservedEventWithAmountForTable(int amount, String tableId) {
        long currentReserved = player.getReservedFunds();
        long newReserved = currentReserved + amount;
        long newAvailable = player.getBankroll() - newReserved;

        FundsReserved event = FundsReserved.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(amount).setCurrencyCode("CHIPS"))
            .setTableRoot(ByteString.copyFrom(tableId.getBytes(StandardCharsets.UTF_8)))
            .setNewReservedBalance(Currency.newBuilder().setAmount(newReserved).setCurrencyCode("CHIPS"))
            .setNewAvailableBalance(Currency.newBuilder().setAmount(newAvailable).setCurrencyCode("CHIPS"))
            .build();
        addEvent(event);
        rehydratePlayer();
    }

    // --- When steps ---

    @When("I handle a RegisterPlayer command with name {string} and email {string}")
    public void handleRegisterPlayerCommand(String name, String email) {
        RegisterPlayer cmd = RegisterPlayer.newBuilder()
            .setDisplayName(name)
            .setEmail(email)
            .setPlayerType(PlayerType.HUMAN)
            .build();
        handleCommand(cmd);
    }

    @When("I handle a RegisterPlayer command with name {string} and email {string} as AI")
    public void handleRegisterPlayerCommandAsAI(String name, String email) {
        RegisterPlayer cmd = RegisterPlayer.newBuilder()
            .setDisplayName(name)
            .setEmail(email)
            .setPlayerType(PlayerType.AI)
            .build();
        handleCommand(cmd);
    }

    @When("I handle a DepositFunds command with amount {int}")
    public void handleDepositFundsCommand(int amount) {
        DepositFunds cmd = DepositFunds.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(amount).setCurrencyCode("CHIPS"))
            .build();
        handleCommand(cmd);
    }

    @When("I handle a WithdrawFunds command with amount {int}")
    public void handleWithdrawFundsCommand(int amount) {
        WithdrawFunds cmd = WithdrawFunds.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(amount).setCurrencyCode("CHIPS"))
            .build();
        handleCommand(cmd);
    }

    @When("I handle a ReserveFunds command with amount {int} for table {string}")
    public void handleReserveFundsCommand(int amount, String tableId) {
        ReserveFunds cmd = ReserveFunds.newBuilder()
            .setAmount(Currency.newBuilder().setAmount(amount).setCurrencyCode("CHIPS"))
            .setTableRoot(ByteString.copyFrom(tableId.getBytes(StandardCharsets.UTF_8)))
            .build();
        handleCommand(cmd);
    }

    @When("I handle a ReleaseFunds command for table {string}")
    public void handleReleaseFundsCommand(String tableId) {
        ReleaseFunds cmd = ReleaseFunds.newBuilder()
            .setTableRoot(ByteString.copyFrom(tableId.getBytes(StandardCharsets.UTF_8)))
            .build();
        handleCommand(cmd);
    }

    @When("I rebuild the player state")
    public void rebuildPlayerState() {
        rehydratePlayer();
    }

    // --- Then steps ---

    @Then("the result is a PlayerRegistered event")
    public void resultIsPlayerRegisteredEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(PlayerRegistered.class);
    }

    @Then("the result is a FundsDeposited event")
    public void resultIsFundsDepositedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(FundsDeposited.class);
    }

    @Then("the result is a FundsWithdrawn event")
    public void resultIsFundsWithdrawnEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(FundsWithdrawn.class);
    }

    @Then("the result is a FundsReserved event")
    public void resultIsFundsReservedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(FundsReserved.class);
    }

    @Then("the result is a FundsReleased event")
    public void resultIsFundsReleasedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(FundsReleased.class);
    }

    @Then("the command fails with status {string}")
    public void commandFailsWithStatus(String status) {
        assertThat(rejectedError).isNotNull();
        Status.Code expectedCode = Status.Code.valueOf(status);
        assertThat(rejectedError.getStatusCode()).isEqualTo(expectedCode);
    }

    @Then("the error message contains {string}")
    public void errorMessageContains(String substring) {
        assertThat(rejectedError).isNotNull();
        assertThat(rejectedError.getMessage().toLowerCase())
            .contains(substring.toLowerCase());
    }

    @Then("the player event has display_name {string}")
    public void playerEventHasDisplayName(String name) {
        assertThat(resultEvent).isInstanceOf(PlayerRegistered.class);
        PlayerRegistered event = (PlayerRegistered) resultEvent;
        assertThat(event.getDisplayName()).isEqualTo(name);
    }

    @Then("the player event has player_type {string}")
    public void playerEventHasPlayerType(String type) {
        assertThat(resultEvent).isInstanceOf(PlayerRegistered.class);
        PlayerRegistered event = (PlayerRegistered) resultEvent;
        PlayerType expectedType = PlayerType.valueOf(type);
        assertThat(event.getPlayerType()).isEqualTo(expectedType);
    }

    @Then("the player event has amount {int}")
    public void playerEventHasAmount(int amount) {
        long actualAmount = getEventAmount();
        assertThat(actualAmount).isEqualTo(amount);
    }

    @Then("the player event has new_balance {int}")
    public void playerEventHasNewBalance(int balance) {
        long actualBalance = getEventNewBalance();
        assertThat(actualBalance).isEqualTo(balance);
    }

    @Then("the player event has new_available_balance {int}")
    public void playerEventHasNewAvailableBalance(int balance) {
        long actualBalance = getEventNewAvailableBalance();
        assertThat(actualBalance).isEqualTo(balance);
    }

    @Then("the player state has bankroll {int}")
    public void playerStateHasBankroll(int bankroll) {
        assertThat(player.getBankroll()).isEqualTo(bankroll);
    }

    @Then("the player state has reserved_funds {int}")
    public void playerStateHasReservedFunds(int reserved) {
        assertThat(player.getReservedFunds()).isEqualTo(reserved);
    }

    @Then("the player state has available_balance {int}")
    public void playerStateHasAvailableBalance(int available) {
        assertThat(player.getAvailableBalance()).isEqualTo(available);
    }

    // --- Helper methods ---

    private void addEvent(Message event) {
        Any eventAny = Any.pack(event, "type.googleapis.com/");
        EventPage page = EventPage.newBuilder()
            .setSequence(eventPages.size())
            .setEvent(eventAny)
            .build();
        eventPages.add(page);
    }

    private void rehydratePlayer() {
        EventBook eventBook = EventBook.newBuilder()
            .setCover(Cover.newBuilder().setDomain("player"))
            .addAllPages(eventPages)
            .setNextSequence(eventPages.size())
            .build();
        player.rehydrate(eventBook);
    }

    private void handleCommand(Message command) {
        try {
            resultEvent = player.handleCommand(command);
            rejectedError = null;
        } catch (Errors.CommandRejectedError e) {
            resultEvent = null;
            rejectedError = e;
        }
    }

    private long getEventAmount() {
        if (resultEvent instanceof FundsDeposited) {
            return ((FundsDeposited) resultEvent).getAmount().getAmount();
        } else if (resultEvent instanceof FundsWithdrawn) {
            return ((FundsWithdrawn) resultEvent).getAmount().getAmount();
        } else if (resultEvent instanceof FundsReserved) {
            return ((FundsReserved) resultEvent).getAmount().getAmount();
        } else if (resultEvent instanceof FundsReleased) {
            return ((FundsReleased) resultEvent).getAmount().getAmount();
        }
        throw new IllegalStateException("Event does not have amount: " + resultEvent.getClass());
    }

    private long getEventNewBalance() {
        if (resultEvent instanceof FundsDeposited) {
            return ((FundsDeposited) resultEvent).getNewBalance().getAmount();
        } else if (resultEvent instanceof FundsWithdrawn) {
            return ((FundsWithdrawn) resultEvent).getNewBalance().getAmount();
        }
        throw new IllegalStateException("Event does not have new_balance: " + resultEvent.getClass());
    }

    private long getEventNewAvailableBalance() {
        if (resultEvent instanceof FundsReserved) {
            return ((FundsReserved) resultEvent).getNewAvailableBalance().getAmount();
        } else if (resultEvent instanceof FundsReleased) {
            return ((FundsReleased) resultEvent).getNewAvailableBalance().getAmount();
        }
        throw new IllegalStateException("Event does not have new_available_balance: " + resultEvent.getClass());
    }
}
