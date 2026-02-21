package dev.angzarr.examples.steps;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.Message;
import dev.angzarr.Cover;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.hand.Hand;
import dev.angzarr.examples.*;
import io.cucumber.java.Before;
import io.cucumber.java.en.And;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Cucumber step definitions for Hand aggregate tests.
 */
public class HandSteps {

    private Hand hand;
    private List<EventPage> eventPages;
    private Message resultEvent;
    private Errors.CommandRejectedError rejectedError;

    @Before
    public void setup() {
        hand = new Hand();
        eventPages = new ArrayList<>();
        resultEvent = null;
        rejectedError = null;
    }

    // --- Given steps ---

    @Given("no prior events for the hand aggregate")
    public void noPriorEventsForHand() {
        eventPages.clear();
        rehydrateHand();
    }

    @Given("a CardsDealt event for hand {int} with {int} players")
    public void cardsDealtEventWithPlayers(int handNumber, int playerCount) {
        CardsDealt.Builder builder = CardsDealt.newBuilder()
            .setHandNumber(handNumber)
            .setGameVariant(GameVariant.TEXAS_HOLDEM)
            .setDealerPosition(0);

        for (int i = 0; i < playerCount; i++) {
            String playerId = "player-" + (i + 1);
            builder.addPlayers(PlayerInHand.newBuilder()
                .setPlayerRoot(ByteString.copyFrom(playerId.getBytes(StandardCharsets.UTF_8)))
                .setPosition(i)
                .setStack(500)
                .build());
        }

        addEvent(builder.build());
        rehydrateHand();
    }

    @Given("a BlindPosted event for player {string} with amount {int}")
    public void blindPostedEventForPlayer(String playerId, int amount) {
        BlindPosted event = BlindPosted.newBuilder()
            .setPlayerRoot(ByteString.copyFrom(playerId.getBytes(StandardCharsets.UTF_8)))
            .setBlindType(amount <= 5 ? "small" : "big")
            .setAmount(amount)
            .setPlayerStack(500 - amount)
            .setPotTotal(hand.getPotTotal() + amount)
            .build();
        addEvent(event);
        rehydrateHand();
    }

    // --- When steps ---

    @When("I handle a DealCards command for hand {int} with players:")
    public void handleDealCardsCommand(int handNumber, io.cucumber.datatable.DataTable dataTable) {
        List<String> playerIds = dataTable.asList();

        DealCards.Builder builder = DealCards.newBuilder()
            .setHandNumber(handNumber)
            .setGameVariant(GameVariant.TEXAS_HOLDEM)
            .setDealerPosition(0);

        for (int i = 0; i < playerIds.size(); i++) {
            builder.addPlayers(PlayerInHand.newBuilder()
                .setPlayerRoot(ByteString.copyFrom(playerIds.get(i).getBytes(StandardCharsets.UTF_8)))
                .setPosition(i)
                .setStack(500)
                .build());
        }

        handleCommand(builder.build());
    }

    @When("I handle a PostBlind command for player {string} with {string} blind {int}")
    public void handlePostBlindCommand(String playerId, String blindType, int amount) {
        PostBlind cmd = PostBlind.newBuilder()
            .setPlayerRoot(ByteString.copyFrom(playerId.getBytes(StandardCharsets.UTF_8)))
            .setBlindType(blindType)
            .setAmount(amount)
            .build();
        handleCommand(cmd);
    }

    @When("I handle a PlayerAction command for player {string} with action {string}")
    public void handlePlayerActionCommand(String playerId, String action) {
        handlePlayerActionCommandWithAmount(playerId, action, 0);
    }

    @When("I handle a PlayerAction command for player {string} with action {string} and amount {int}")
    public void handlePlayerActionCommandWithAmount(String playerId, String action, int amount) {
        ActionType actionType = ActionType.valueOf(action.toUpperCase());
        PlayerAction cmd = PlayerAction.newBuilder()
            .setPlayerRoot(ByteString.copyFrom(playerId.getBytes(StandardCharsets.UTF_8)))
            .setAction(actionType)
            .setAmount(amount)
            .build();
        handleCommand(cmd);
    }

    @When("I handle a DealCommunityCards command with count {int}")
    public void handleDealCommunityCardsCommand(int count) {
        DealCommunityCards cmd = DealCommunityCards.newBuilder()
            .setCount(count)
            .build();
        handleCommand(cmd);
    }

    @When("I handle an AwardPot command with winner {string} winning {int}")
    public void handleAwardPotCommand(String playerId, int amount) {
        AwardPot cmd = AwardPot.newBuilder()
            .addAwards(PotAward.newBuilder()
                .setPlayerRoot(ByteString.copyFrom(playerId.getBytes(StandardCharsets.UTF_8)))
                .setAmount(amount)
                .setPotType("main"))
            .build();
        handleCommand(cmd);
    }

    @When("I rebuild the hand state")
    public void rebuildHandState() {
        rehydrateHand();
    }

    // --- Then steps ---

    @Then("the result is a CardsDealt event")
    public void resultIsCardsDealtEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(CardsDealt.class);
    }

    @Then("the result is a BlindPosted event")
    public void resultIsBlindPostedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(BlindPosted.class);
    }

    @Then("the result is an ActionTaken event")
    public void resultIsActionTakenEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(ActionTaken.class);
    }

    @Then("the result is a CommunityCardsDealt event")
    public void resultIsCommunityCardsDealtEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(CommunityCardsDealt.class);
    }

    @Then("the result is a PotAwarded event")
    public void resultIsPotAwardedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(PotAwarded.class);
    }

    @Then("the hand event has hand_number {int}")
    public void handEventHasHandNumber(int number) {
        assertThat(resultEvent).isInstanceOf(CardsDealt.class);
        assertThat(((CardsDealt) resultEvent).getHandNumber()).isEqualTo(number);
    }

    @Then("the hand event has {int} players")
    public void handEventHasPlayers(int count) {
        assertThat(resultEvent).isInstanceOf(CardsDealt.class);
        assertThat(((CardsDealt) resultEvent).getPlayersCount()).isEqualTo(count);
    }

    @Then("the hand event has pot_total {int}")
    public void handEventHasPotTotal(int potTotal) {
        if (resultEvent instanceof BlindPosted) {
            assertThat(((BlindPosted) resultEvent).getPotTotal()).isEqualTo(potTotal);
        } else if (resultEvent instanceof ActionTaken) {
            assertThat(((ActionTaken) resultEvent).getPotTotal()).isEqualTo(potTotal);
        }
    }

    @Then("the hand event has action {string}")
    public void handEventHasAction(String action) {
        assertThat(resultEvent).isInstanceOf(ActionTaken.class);
        ActionType expected = ActionType.valueOf(action.toUpperCase());
        assertThat(((ActionTaken) resultEvent).getAction()).isEqualTo(expected);
    }

    @Then("the hand state has pot_total {int}")
    public void handStateHasPotTotal(int potTotal) {
        assertThat(hand.getPotTotal()).isEqualTo(potTotal);
    }

    @Then("the hand state has {int} active_players")
    public void handStateHasActivePlayers(int count) {
        assertThat(hand.getActivePlayerCount()).isEqualTo(count);
    }

    @Then("the hand state is complete")
    public void handStateIsComplete() {
        assertThat(hand.isComplete()).isTrue();
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

    private void rehydrateHand() {
        EventBook eventBook = EventBook.newBuilder()
            .setCover(Cover.newBuilder().setDomain("hand"))
            .addAllPages(eventPages)
            .setNextSequence(eventPages.size())
            .build();
        hand.rehydrate(eventBook);
    }

    private void handleCommand(Message command) {
        try {
            resultEvent = hand.handleCommand(command);
            rejectedError = null;
        } catch (Errors.CommandRejectedError e) {
            resultEvent = null;
            rejectedError = e;
        }
    }
}
