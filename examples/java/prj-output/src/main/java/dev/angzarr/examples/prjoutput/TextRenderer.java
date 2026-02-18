package dev.angzarr.examples.prjoutput;

import com.google.protobuf.ByteString;
import dev.angzarr.examples.*;

import java.util.HashMap;
import java.util.Map;

/**
 * Renders poker events as human-readable text.
 */
public class TextRenderer {

    private final Map<String, String> playerNames = new HashMap<>();

    /**
     * Set display name for a player root.
     */
    public void setPlayerName(byte[] playerRoot, String name) {
        playerNames.put(bytesToHex(playerRoot), name);
    }

    /**
     * Get display name for a player (or shortened root if unknown).
     */
    private String getPlayerName(ByteString playerRoot) {
        if (playerRoot == null || playerRoot.isEmpty()) {
            return "Unknown";
        }
        String hex = bytesToHex(playerRoot.toByteArray());
        return playerNames.getOrDefault(hex, hex.substring(0, Math.min(8, hex.length())));
    }

    /**
     * Render an event to text.
     */
    public String render(String eventType, Object event) {
        switch (eventType) {
            // Player events
            case "PlayerRegistered":
                return renderPlayerRegistered((PlayerRegistered) event);
            case "FundsDeposited":
                return renderFundsDeposited((FundsDeposited) event);
            case "FundsWithdrawn":
                return renderFundsWithdrawn((FundsWithdrawn) event);
            case "FundsReserved":
                return renderFundsReserved((FundsReserved) event);
            case "FundsReleased":
                return renderFundsReleased((FundsReleased) event);

            // Table events
            case "TableCreated":
                return renderTableCreated((TableCreated) event);
            case "PlayerJoined":
                return renderPlayerJoined((PlayerJoined) event);
            case "PlayerLeft":
                return renderPlayerLeft((PlayerLeft) event);
            case "HandStarted":
                return renderHandStarted((HandStarted) event);
            case "HandEnded":
                return renderHandEnded((HandEnded) event);

            // Hand events
            case "CardsDealt":
                return renderCardsDealt((CardsDealt) event);
            case "BlindPosted":
                return renderBlindPosted((BlindPosted) event);
            case "ActionTaken":
                return renderActionTaken((ActionTaken) event);
            case "CommunityCardsDealt":
                return renderCommunityCardsDealt((CommunityCardsDealt) event);
            case "PotAwarded":
                return renderPotAwarded((PotAwarded) event);
            case "HandComplete":
                return renderHandComplete((HandComplete) event);

            default:
                return "[" + eventType + "]";
        }
    }

    private String renderPlayerRegistered(PlayerRegistered event) {
        return String.format("Player registered: %s (%s)",
            event.getDisplayName(), event.getEmail());
    }

    private String renderFundsDeposited(FundsDeposited event) {
        long amount = event.hasAmount() ? event.getAmount().getAmount() : 0;
        long balance = event.hasNewBalance() ? event.getNewBalance().getAmount() : 0;
        return String.format("Deposit: %d -> balance %d", amount, balance);
    }

    private String renderFundsWithdrawn(FundsWithdrawn event) {
        long amount = event.hasAmount() ? event.getAmount().getAmount() : 0;
        long balance = event.hasNewBalance() ? event.getNewBalance().getAmount() : 0;
        return String.format("Withdrawal: %d -> balance %d", amount, balance);
    }

    private String renderFundsReserved(FundsReserved event) {
        long amount = event.hasAmount() ? event.getAmount().getAmount() : 0;
        return String.format("Funds reserved: %d", amount);
    }

    private String renderFundsReleased(FundsReleased event) {
        long amount = event.hasAmount() ? event.getAmount().getAmount() : 0;
        return String.format("Funds released: %d", amount);
    }

    private String renderTableCreated(TableCreated event) {
        return String.format("Table created: %s (%d seats, blinds %d/%d)",
            event.getTableName(),
            event.getMaxPlayers(),
            event.getSmallBlind(),
            event.getBigBlind());
    }

    private String renderPlayerJoined(PlayerJoined event) {
        return String.format("Player joined seat %d with %d chips",
            event.getSeatPosition(), event.getBuyInAmount());
    }

    private String renderPlayerLeft(PlayerLeft event) {
        return String.format("Player left seat %d with %d chips",
            event.getSeatPosition(), event.getChipsCashedOut());
    }

    private String renderHandStarted(HandStarted event) {
        return String.format("=== Hand #%d started (dealer: seat %d, blinds: %d/%d) ===",
            event.getHandNumber(),
            event.getDealerPosition(),
            event.getSmallBlind(),
            event.getBigBlind());
    }

    private String renderHandEnded(HandEnded event) {
        String handId = bytesToHex(event.getHandRoot().toByteArray()).substring(0, 8);
        return String.format("=== Hand %s ended ===", handId);
    }

    private String renderCardsDealt(CardsDealt event) {
        return String.format("Cards dealt to %d players", event.getPlayersCount());
    }

    private String renderBlindPosted(BlindPosted event) {
        String player = getPlayerName(event.getPlayerRoot());
        return String.format("%s posts %s blind: %d",
            player, event.getBlindType(), event.getAmount());
    }

    private String renderActionTaken(ActionTaken event) {
        String player = getPlayerName(event.getPlayerRoot());
        String action = event.getAction().name().toLowerCase();
        if (event.getAmount() > 0) {
            return String.format("%s %s %d (pot: %d)",
                player, action, event.getAmount(), event.getPotTotal());
        }
        return String.format("%s %s (pot: %d)",
            player, action, event.getPotTotal());
    }

    private String renderCommunityCardsDealt(CommunityCardsDealt event) {
        StringBuilder cards = new StringBuilder();
        for (Card card : event.getCardsList()) {
            if (cards.length() > 0) cards.append(" ");
            cards.append(renderCard(card));
        }
        String phase = event.getPhase().name().toLowerCase();
        return String.format("%s: %s", phase, cards);
    }

    private String renderPotAwarded(PotAwarded event) {
        StringBuilder sb = new StringBuilder("Pot awarded: ");
        boolean first = true;
        for (PotWinner winner : event.getWinnersList()) {
            if (!first) sb.append(", ");
            String player = getPlayerName(winner.getPlayerRoot());
            sb.append(String.format("%s wins %d", player, winner.getAmount()));
            first = false;
        }
        return sb.toString();
    }

    private String renderHandComplete(HandComplete event) {
        return String.format("Hand #%d complete", event.getHandNumber());
    }

    private String renderCard(Card card) {
        String rank = switch (card.getRank()) {
            case TWO -> "2";
            case THREE -> "3";
            case FOUR -> "4";
            case FIVE -> "5";
            case SIX -> "6";
            case SEVEN -> "7";
            case EIGHT -> "8";
            case NINE -> "9";
            case TEN -> "T";
            case JACK -> "J";
            case QUEEN -> "Q";
            case KING -> "K";
            case ACE -> "A";
            default -> "?";
        };
        String suit = switch (card.getSuit()) {
            case CLUBS -> "c";
            case DIAMONDS -> "d";
            case HEARTS -> "h";
            case SPADES -> "s";
            default -> "?";
        };
        return rank + suit;
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
