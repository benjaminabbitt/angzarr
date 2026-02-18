package dev.angzarr.examples.hand.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.hand.state.HandState;
import dev.angzarr.examples.*;

import java.time.Instant;
import java.util.*;

/**
 * Functional handler for DealCards command.
 *
 * <p>Pure function following guard/validate/compute pattern.
 */
public final class DealCardsHandler {

    private DealCardsHandler() {}

    /**
     * Handle DealCards command.
     *
     * @param cmd The command
     * @param state Current aggregate state
     * @return The resulting event
     * @throws Errors.CommandRejectedError if command is rejected
     */
    public static CardsDealt handle(DealCards cmd, HandState state) {
        // Guard
        if (state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand already exists");
        }

        // Validate
        if (cmd.getPlayersList().isEmpty()) {
            throw Errors.CommandRejectedError.invalidArgument("players required");
        }
        if (cmd.getPlayersCount() < 2) {
            throw Errors.CommandRejectedError.invalidArgument("Need at least 2 players");
        }

        // Compute
        List<Card> deck = createShuffledDeck(cmd.getDeckSeed().toByteArray());
        int cardsPerPlayer = getHoleCardCount(cmd.getGameVariant());

        List<PlayerHoleCards> playerCards = new ArrayList<>();
        int deckIndex = 0;
        for (PlayerInHand player : cmd.getPlayersList()) {
            List<Card> holeCards = new ArrayList<>();
            for (int i = 0; i < cardsPerPlayer; i++) {
                holeCards.add(deck.get(deckIndex++));
            }
            playerCards.add(PlayerHoleCards.newBuilder()
                .setPlayerRoot(player.getPlayerRoot())
                .addAllCards(holeCards)
                .build());
        }

        return CardsDealt.newBuilder()
            .setTableRoot(cmd.getTableRoot())
            .setHandNumber(cmd.getHandNumber())
            .setGameVariant(cmd.getGameVariant())
            .addAllPlayerCards(playerCards)
            .setDealerPosition(cmd.getDealerPosition())
            .addAllPlayers(cmd.getPlayersList())
            .addAllRemainingDeck(deck.subList(deckIndex, deck.size()))
            .setDealtAt(now())
            .build();
    }

    private static List<Card> createShuffledDeck(byte[] seed) {
        List<Card> deck = new ArrayList<>();
        for (Suit suit : new Suit[]{Suit.CLUBS, Suit.DIAMONDS, Suit.HEARTS, Suit.SPADES}) {
            for (int rank = 2; rank <= 14; rank++) {
                deck.add(Card.newBuilder().setSuit(suit).setRank(Rank.forNumber(rank)).build());
            }
        }
        Random rng = seed.length > 0 ? new Random(bytesToLong(seed)) : new Random();
        Collections.shuffle(deck, rng);
        return deck;
    }

    private static int getHoleCardCount(GameVariant variant) {
        switch (variant) {
            case OMAHA: return 4;
            case FIVE_CARD_DRAW: return 5;
            default: return 2;
        }
    }

    private static long bytesToLong(byte[] bytes) {
        long result = 0;
        for (int i = 0; i < Math.min(8, bytes.length); i++) {
            result = (result << 8) | (bytes[i] & 0xFF);
        }
        return result;
    }

    private static Timestamp now() {
        Instant instant = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(instant.getEpochSecond())
            .setNanos(instant.getNano())
            .build();
    }
}
