package dev.angzarr.examples.pmghandflow;

import dev.angzarr.client.ProcessManager;
import dev.angzarr.proto.angzarr.CommandBook;
import dev.angzarr.proto.examples.CardsDealt;
import dev.angzarr.proto.examples.DealCards;
import dev.angzarr.proto.examples.EndHand;
import dev.angzarr.proto.examples.HandComplete;
import dev.angzarr.proto.examples.HandStarted;
import dev.angzarr.proto.examples.PostBlinds;

import java.util.List;

/**
 * Hand Flow Process Manager - orchestrates poker hand phases across domains.
 *
 * This PM coordinates the workflow between table and hand domains,
 * tracking phase transitions and dispatching commands as the hand progresses.
 */

// docs:start:pm_state
enum HandPhase {
    AWAITING_DEAL, DEALING, BLINDS, BETTING, COMPLETE
}

class HandFlowState {
    private String handId = "";
    private HandPhase phase = HandPhase.AWAITING_DEAL;
    private int playerCount = 0;

    public String getHandId() { return handId; }
    public void setHandId(String handId) { this.handId = handId; }
    public HandPhase getPhase() { return phase; }
    public void setPhase(HandPhase phase) { this.phase = phase; }
    public int getPlayerCount() { return playerCount; }
    public void setPlayerCount(int playerCount) { this.playerCount = playerCount; }
}
// docs:end:pm_state

// docs:start:pm_handler
public class HandFlowPM extends ProcessManager<HandFlowState> {

    @ReactsTo(HandStarted.class)
    public List<CommandBook> handleHandStarted(HandStarted event, HandFlowState state) {
        state.setHandId(event.getHandId());
        state.setPhase(HandPhase.DEALING);
        state.setPlayerCount(event.getPlayerCount());

        return List.of(buildCommand("hand", DealCards.newBuilder()
            .setHandId(event.getHandId())
            .setPlayerCount(event.getPlayerCount())
            .build()));
    }

    @ReactsTo(CardsDealt.class)
    public List<CommandBook> handleCardsDealt(CardsDealt event, HandFlowState state) {
        state.setPhase(HandPhase.BLINDS);
        return List.of(buildCommand("hand", PostBlinds.newBuilder()
            .setHandId(state.getHandId())
            .build()));
    }

    @ReactsTo(HandComplete.class)
    public List<CommandBook> handleHandComplete(HandComplete event, HandFlowState state) {
        state.setPhase(HandPhase.COMPLETE);
        return List.of(buildCommand("table", EndHand.newBuilder()
            .setHandId(state.getHandId())
            .setWinnerId(event.getWinnerId())
            .build()));
    }
}
// docs:end:pm_handler
