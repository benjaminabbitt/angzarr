package dev.angzarr.examples.hand.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.hand.state.HandState;
import dev.angzarr.examples.*;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;

/**
 * Functional handler for DealCommunityCards command.
 *
 * <p>Pure function following guard/validate/compute pattern.
 */
public final class DealCommunityCardsHandler {

    private DealCommunityCardsHandler() {}

    /**
     * Handle DealCommunityCards command.
     *
     * @param cmd The command
     * @param state Current aggregate state
     * @return The resulting event
     * @throws Errors.CommandRejectedError if command is rejected
     */
    public static CommunityCardsDealt handle(DealCommunityCards cmd, HandState state) {
        // Guard
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
        }
        if (state.isComplete()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand is complete");
        }

        // Validate
        if (cmd.getCount() <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("Must deal at least 1 card");
        }

        List<byte[]> remaining = state.getRemainingDeck();
        if (remaining.size() < cmd.getCount()) {
            throw Errors.CommandRejectedError.invalidArgument("Not enough cards in deck");
        }

        // Compute
        BettingPhase nextPhase = determineNextPhase(state.getCurrentPhase());

        List<Card> newCards = new ArrayList<>();
        for (int i = 0; i < cmd.getCount() && i < remaining.size(); i++) {
            newCards.add(bytesToCard(remaining.get(i)));
        }

        List<Card> allCommunity = new ArrayList<>();
        for (byte[] c : state.getCommunityCards()) {
            allCommunity.add(bytesToCard(c));
        }
        allCommunity.addAll(newCards);

        return CommunityCardsDealt.newBuilder()
            .addAllCards(newCards)
            .setPhase(nextPhase)
            .addAllAllCommunityCards(allCommunity)
            .setDealtAt(now())
            .build();
    }

    private static BettingPhase determineNextPhase(int currentPhase) {
        if (currentPhase == BettingPhase.PREFLOP_VALUE) return BettingPhase.FLOP;
        if (currentPhase == BettingPhase.FLOP_VALUE) return BettingPhase.TURN;
        if (currentPhase == BettingPhase.TURN_VALUE) return BettingPhase.RIVER;
        return BettingPhase.SHOWDOWN;
    }

    private static Card bytesToCard(byte[] bytes) {
        return Card.newBuilder()
            .setSuit(Suit.forNumber(bytes[0]))
            .setRank(Rank.forNumber(bytes[1]))
            .build();
    }

    private static Timestamp now() {
        Instant instant = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(instant.getEpochSecond())
            .setNanos(instant.getNano())
            .build();
    }
}
