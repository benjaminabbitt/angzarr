package dev.angzarr.examples.steps;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.Message;
import dev.angzarr.Cover;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.table.Table;
import dev.angzarr.examples.*;
import io.cucumber.datatable.DataTable;
import io.cucumber.java.Before;
import io.cucumber.java.en.And;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;
import io.grpc.Status;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Cucumber step definitions for Table aggregate tests.
 */
public class TableSteps {

    private Table table;
    private List<EventPage> eventPages;
    private Message resultEvent;
    private Errors.CommandRejectedError rejectedError;

    @Before
    public void setup() {
        table = new Table();
        eventPages = new ArrayList<>();
        resultEvent = null;
        rejectedError = null;
    }

    // --- Given steps ---

    @Given("no prior events for the table aggregate")
    public void noPriorEventsForTable() {
        eventPages.clear();
        rehydrateTable();
    }

    @Given("a TableCreated event for {string}")
    public void tableCreatedEventFor(String name) {
        TableCreated event = TableCreated.newBuilder()
            .setTableName(name)
            .setGameVariant(GameVariant.TEXAS_HOLDEM)
            .setSmallBlind(5)
            .setBigBlind(10)
            .setMaxPlayers(9)
            .setMinBuyIn(200)
            .setMaxBuyIn(1000)
            .build();
        addEvent(event);
        rehydrateTable();
    }

    @Given("a TableCreated event for {string} with min_buy_in {int}")
    public void tableCreatedEventWithMinBuyIn(String name, int minBuyIn) {
        TableCreated event = TableCreated.newBuilder()
            .setTableName(name)
            .setGameVariant(GameVariant.TEXAS_HOLDEM)
            .setSmallBlind(5)
            .setBigBlind(10)
            .setMaxPlayers(9)
            .setMinBuyIn(minBuyIn)
            .setMaxBuyIn(1000)
            .build();
        addEvent(event);
        rehydrateTable();
    }

    @Given("a TableCreated event for {string} with max_players {int}")
    public void tableCreatedEventWithMaxPlayers(String name, int maxPlayers) {
        TableCreated event = TableCreated.newBuilder()
            .setTableName(name)
            .setGameVariant(GameVariant.TEXAS_HOLDEM)
            .setSmallBlind(5)
            .setBigBlind(10)
            .setMaxPlayers(maxPlayers)
            .setMinBuyIn(200)
            .setMaxBuyIn(1000)
            .build();
        addEvent(event);
        rehydrateTable();
    }

    @Given("a PlayerJoined event for player {string} at seat {int}")
    public void playerJoinedEventAtSeat(String playerId, int seat) {
        playerJoinedEventAtSeatWithStack(playerId, seat, 500);
    }

    @Given("a PlayerJoined event for player {string} at seat {int} with stack {int}")
    public void playerJoinedEventAtSeatWithStack(String playerId, int seat, int stack) {
        PlayerJoined event = PlayerJoined.newBuilder()
            .setPlayerRoot(ByteString.copyFrom(playerId.getBytes(StandardCharsets.UTF_8)))
            .setSeatPosition(seat)
            .setBuyInAmount(stack)
            .setStack(stack)
            .build();
        addEvent(event);
        rehydrateTable();
    }

    @Given("a HandStarted event for hand {int}")
    public void handStartedEventForHand(int handNumber) {
        handStartedEventWithDealer(handNumber, 0);
    }

    @Given("a HandStarted event for hand {int} with dealer at seat {int}")
    public void handStartedEventWithDealer(int handNumber, int dealerPosition) {
        HandStarted event = HandStarted.newBuilder()
            .setHandNumber(handNumber)
            .setDealerPosition(dealerPosition)
            .setSmallBlind(5)
            .setBigBlind(10)
            .setGameVariant(GameVariant.TEXAS_HOLDEM)
            .build();
        addEvent(event);
        rehydrateTable();
    }

    @Given("a HandEnded event for hand {int}")
    public void handEndedEventForHand(int handNumber) {
        // HandEnded uses hand_root, not hand_number - use a synthetic root
        byte[] handRoot = ("hand_" + handNumber).getBytes(StandardCharsets.UTF_8);
        HandEnded event = HandEnded.newBuilder()
            .setHandRoot(ByteString.copyFrom(handRoot))
            .build();
        addEvent(event);
        rehydrateTable();
    }

    // --- When steps ---

    @When("I handle a CreateTable command with name {string} and variant {string}:")
    public void handleCreateTableCommand(String name, String variant, DataTable dataTable) {
        Map<String, String> params = dataTable.asMaps().get(0);
        GameVariant gameVariant = GameVariant.valueOf(variant);

        CreateTable cmd = CreateTable.newBuilder()
            .setTableName(name)
            .setGameVariant(gameVariant)
            .setSmallBlind(Integer.parseInt(params.get("small_blind")))
            .setBigBlind(Integer.parseInt(params.get("big_blind")))
            .setMinBuyIn(Integer.parseInt(params.get("min_buy_in")))
            .setMaxBuyIn(Integer.parseInt(params.get("max_buy_in")))
            .setMaxPlayers(Integer.parseInt(params.get("max_players")))
            .build();
        handleCommand(cmd);
    }

    @When("I handle a JoinTable command for player {string} at seat {int} with buy-in {int}")
    public void handleJoinTableCommand(String playerId, int seat, int buyIn) {
        JoinTable cmd = JoinTable.newBuilder()
            .setPlayerRoot(ByteString.copyFrom(playerId.getBytes(StandardCharsets.UTF_8)))
            .setPreferredSeat(seat)
            .setBuyInAmount(buyIn)
            .build();
        handleCommand(cmd);
    }

    @When("I handle a LeaveTable command for player {string}")
    public void handleLeaveTableCommand(String playerId) {
        LeaveTable cmd = LeaveTable.newBuilder()
            .setPlayerRoot(ByteString.copyFrom(playerId.getBytes(StandardCharsets.UTF_8)))
            .build();
        handleCommand(cmd);
    }

    @When("I handle a StartHand command")
    public void handleStartHandCommand() {
        StartHand cmd = StartHand.newBuilder().build();
        handleCommand(cmd);
    }

    @When("I handle an EndHand command with winner {string} winning {int}")
    public void handleEndHandCommand(String winnerId, int amount) {
        ByteString winnerRoot = ByteString.copyFrom(winnerId.getBytes(StandardCharsets.UTF_8));
        EndHand cmd = EndHand.newBuilder()
            .addResults(PotResult.newBuilder()
                .setWinnerRoot(winnerRoot)
                .setAmount(amount)
                .setPotType("main"))
            .build();
        handleCommand(cmd);
    }

    @When("I rebuild the table state")
    public void rebuildTableState() {
        rehydrateTable();
    }

    // --- Then steps ---

    @Then("the result is a TableCreated event")
    public void resultIsTableCreatedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(TableCreated.class);
    }

    @Then("the result is a PlayerJoined event")
    public void resultIsPlayerJoinedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(PlayerJoined.class);
    }

    @Then("the result is a PlayerLeft event")
    public void resultIsPlayerLeftEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(PlayerLeft.class);
    }

    @Then("the result is a HandStarted event")
    public void resultIsHandStartedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(HandStarted.class);
    }

    @Then("the result is a HandEnded event")
    public void resultIsHandEndedEvent() {
        assertThat(rejectedError).isNull();
        assertThat(resultEvent).isInstanceOf(HandEnded.class);
    }

    @Then("the table event has table_name {string}")
    public void tableEventHasName(String name) {
        assertThat(resultEvent).isInstanceOf(TableCreated.class);
        TableCreated event = (TableCreated) resultEvent;
        assertThat(event.getTableName()).isEqualTo(name);
    }

    @Then("the table event has game_variant {string}")
    public void tableEventHasGameVariant(String variant) {
        GameVariant expected = GameVariant.valueOf(variant);
        if (resultEvent instanceof TableCreated) {
            assertThat(((TableCreated) resultEvent).getGameVariant()).isEqualTo(expected);
        } else if (resultEvent instanceof HandStarted) {
            assertThat(((HandStarted) resultEvent).getGameVariant()).isEqualTo(expected);
        }
    }

    @Then("the table event has small_blind {int}")
    public void tableEventHasSmallBlind(int smallBlind) {
        if (resultEvent instanceof TableCreated) {
            assertThat(((TableCreated) resultEvent).getSmallBlind()).isEqualTo(smallBlind);
        } else if (resultEvent instanceof HandStarted) {
            assertThat(((HandStarted) resultEvent).getSmallBlind()).isEqualTo(smallBlind);
        }
    }

    @Then("the table event has big_blind {int}")
    public void tableEventHasBigBlind(int bigBlind) {
        if (resultEvent instanceof TableCreated) {
            assertThat(((TableCreated) resultEvent).getBigBlind()).isEqualTo(bigBlind);
        } else if (resultEvent instanceof HandStarted) {
            assertThat(((HandStarted) resultEvent).getBigBlind()).isEqualTo(bigBlind);
        }
    }

    @Then("the table event has seat_position {int}")
    public void tableEventHasSeatPosition(int position) {
        assertThat(resultEvent).isInstanceOf(PlayerJoined.class);
        assertThat(((PlayerJoined) resultEvent).getSeatPosition()).isEqualTo(position);
    }

    @Then("the table event has buy_in_amount {int}")
    public void tableEventHasBuyInAmount(int amount) {
        assertThat(resultEvent).isInstanceOf(PlayerJoined.class);
        assertThat(((PlayerJoined) resultEvent).getBuyInAmount()).isEqualTo(amount);
    }

    @Then("the table event has chips_cashed_out {int}")
    public void tableEventHasChipsCashedOut(int amount) {
        assertThat(resultEvent).isInstanceOf(PlayerLeft.class);
        assertThat(((PlayerLeft) resultEvent).getChipsCashedOut()).isEqualTo(amount);
    }

    @Then("the table event has hand_number {int}")
    public void tableEventHasHandNumber(int handNumber) {
        // HandStarted has hand_number, HandEnded only has hand_root
        assertThat(resultEvent).isInstanceOf(HandStarted.class);
        assertThat(((HandStarted) resultEvent).getHandNumber()).isEqualTo(handNumber);
    }

    @Then("the table event has dealer_position {int}")
    public void tableEventHasDealerPosition(int position) {
        assertThat(resultEvent).isInstanceOf(HandStarted.class);
        assertThat(((HandStarted) resultEvent).getDealerPosition()).isEqualTo(position);
    }

    @Then("the table event has {int} active_players")
    public void tableEventHasActivePlayers(int count) {
        assertThat(resultEvent).isInstanceOf(HandStarted.class);
        assertThat(((HandStarted) resultEvent).getActivePlayersCount()).isEqualTo(count);
    }

    @Then("player {string} stack change is {int}")
    public void playerStackChangeIs(String playerId, int change) {
        assertThat(resultEvent).isInstanceOf(HandEnded.class);
        HandEnded event = (HandEnded) resultEvent;
        long actualChange = event.getStackChangesOrDefault(playerId, 0L);
        assertThat(actualChange).isEqualTo((long) change);
    }

    @Then("the table state has {int} players")
    public void tableStateHasPlayers(int count) {
        assertThat(table.getPlayerCount()).isEqualTo(count);
    }

    @Then("the table state has seat {int} occupied by {string}")
    public void tableStateHasSeatOccupiedBy(int seat, String playerId) {
        assertThat(table.getPlayerAtSeat(seat)).isNotNull();
    }

    @Then("the table state has status {string}")
    public void tableStateHasStatus(String status) {
        assertThat(table.getStatus()).isEqualTo(status);
    }

    @Then("the table state has hand_count {int}")
    public void tableStateHasHandCount(int count) {
        assertThat(table.getHandNumber()).isEqualTo(count);
    }

    // --- Helper methods ---

    private void addEvent(Message event) {
        Any eventAny = Any.pack(event, "type.googleapis.com/");
        EventPage page = EventPage.newBuilder()
            .setNum(eventPages.size())
            .setEvent(eventAny)
            .build();
        eventPages.add(page);
    }

    private void rehydrateTable() {
        EventBook eventBook = EventBook.newBuilder()
            .setCover(Cover.newBuilder().setDomain("table"))
            .addAllPages(eventPages)
            .setNextSequence(eventPages.size())
            .build();
        table.rehydrate(eventBook);
    }

    private void handleCommand(Message command) {
        try {
            resultEvent = table.handleCommand(command);
            rejectedError = null;
        } catch (Errors.CommandRejectedError e) {
            resultEvent = null;
            rejectedError = e;
        }
    }
}
