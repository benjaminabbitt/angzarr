package dev.angzarr.examples.handflowoo;

import com.google.protobuf.Struct;
import dev.angzarr.*;
import dev.angzarr.client.ProcessManager;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.client.annotations.ReactsTo;
import dev.angzarr.examples.*;

import java.util.List;

/**
 * Hand Flow Process Manager using OO-style annotations.
 *
 * <p>This PM orchestrates poker hand flow by:
 * - Tracking when hands start and complete
 * - Coordinating between table and hand domains
 *
 * <p>Uses Struct as a minimal protobuf state type for demonstration.
 * In production, you would define a proper protobuf state message.
 */
public class HandFlowPM extends ProcessManager<Struct> {

    public HandFlowPM() {
        super("hand-flow");
    }

    @Override
    protected Struct createEmptyState() {
        return Struct.getDefaultInstance();
    }

    /**
     * Declare the hand destination needed when a hand starts.
     */
    @Prepares(HandStarted.class)
    public List<Cover> prepareHandStarted(HandStarted event) {
        return List.of(Cover.newBuilder()
            .setDomain("hand")
            .setRoot(dev.angzarr.UUID.newBuilder().setValue(event.getHandRoot()))
            .build());
    }

    /**
     * Process the HandStarted event.
     *
     * <p>Initialize hand process (not persisted in this simplified version).
     * The saga-table-hand will send DealCards, so we don't emit commands here.
     */
    @ReactsTo(HandStarted.class)
    public List<CommandBook> handleHandStarted(HandStarted event) {
        return List.of();
    }

    /**
     * Process the CardsDealt event.
     *
     * <p>Post small blind command. In a real implementation, we'd track state
     * to know which blind to post.
     */
    @ReactsTo(CardsDealt.class)
    public List<CommandBook> handleCardsDealt(CardsDealt event) {
        return List.of();
    }

    /**
     * Process the BlindPosted event.
     *
     * <p>In a full implementation, we'd check if both blinds are posted
     * and then start the betting round.
     */
    @ReactsTo(BlindPosted.class)
    public List<CommandBook> handleBlindPosted(BlindPosted event) {
        return List.of();
    }

    /**
     * Process the ActionTaken event.
     *
     * <p>In a full implementation, we'd check if betting is complete
     * and advance to the next phase.
     */
    @ReactsTo(ActionTaken.class)
    public List<CommandBook> handleActionTaken(ActionTaken event) {
        return List.of();
    }

    /**
     * Process the CommunityCardsDealt event.
     *
     * <p>Start new betting round after community cards.
     */
    @ReactsTo(CommunityCardsDealt.class)
    public List<CommandBook> handleCommunityDealt(CommunityCardsDealt event) {
        return List.of();
    }

    /**
     * Process the PotAwarded event.
     *
     * <p>Hand is complete. Clean up.
     */
    @ReactsTo(PotAwarded.class)
    public List<CommandBook> handlePotAwarded(PotAwarded event) {
        return List.of();
    }
}
