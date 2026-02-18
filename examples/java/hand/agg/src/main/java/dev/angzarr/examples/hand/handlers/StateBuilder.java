package dev.angzarr.examples.hand.handlers;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.examples.hand.state.HandState;
import dev.angzarr.examples.hand.state.PlayerHandState;
import dev.angzarr.examples.*;

/**
 * Builds HandState from an EventBook (functional style).
 *
 * <p>Used by CommandRouter for state reconstruction.
 */
public final class StateBuilder {

    private StateBuilder() {}

    /**
     * Build state from event book by replaying all events.
     */
    public static HandState fromEventBook(EventBook eventBook) {
        HandState state = new HandState();
        if (eventBook == null) {
            return state;
        }

        for (EventPage page : eventBook.getPagesList()) {
            applyEvent(state, page.getEvent());
        }
        return state;
    }

    /**
     * Apply a single event to state.
     */
    public static void applyEvent(HandState state, Any eventAny) {
        String typeUrl = eventAny.getTypeUrl();

        try {
            if (typeUrl.endsWith("CardsDealt")) {
                applyCardsDealt(state, eventAny.unpack(CardsDealt.class));
            } else if (typeUrl.endsWith("BlindPosted")) {
                applyBlindPosted(state, eventAny.unpack(BlindPosted.class));
            } else if (typeUrl.endsWith("ActionTaken")) {
                applyActionTaken(state, eventAny.unpack(ActionTaken.class));
            } else if (typeUrl.endsWith("BettingRoundComplete")) {
                applyBettingRoundComplete(state, eventAny.unpack(BettingRoundComplete.class));
            } else if (typeUrl.endsWith("CommunityCardsDealt")) {
                applyCommunityCardsDealt(state, eventAny.unpack(CommunityCardsDealt.class));
            } else if (typeUrl.endsWith("HandComplete")) {
                state.setStatus("complete");
            } else if (typeUrl.endsWith("PotAwarded")) {
                applyPotAwarded(state, eventAny.unpack(PotAwarded.class));
            }
        } catch (InvalidProtocolBufferException e) {
            throw new RuntimeException("Failed to unpack event: " + typeUrl, e);
        }
    }

    private static void applyCardsDealt(HandState state, CardsDealt event) {
        state.setHandId("hand_" + event.getHandNumber());
        state.setTableRoot(event.getTableRoot().toByteArray());
        state.setHandNumber(event.getHandNumber());
        state.setGameVariant(event.getGameVariantValue());
        state.setDealerPosition(event.getDealerPosition());
        state.setStatus("betting");
        state.setCurrentPhase(BettingPhase.PREFLOP_VALUE);

        // Initialize players
        for (PlayerInHand p : event.getPlayersList()) {
            PlayerHandState pState = new PlayerHandState();
            pState.setPlayerRoot(p.getPlayerRoot().toByteArray());
            pState.setPosition(p.getPosition());
            pState.setStack(p.getStack());
            state.getPlayers().put(bytesToHex(p.getPlayerRoot().toByteArray()), pState);
        }

        // Store hole cards
        for (PlayerHoleCards phc : event.getPlayerCardsList()) {
            PlayerHandState pState = state.getPlayer(phc.getPlayerRoot().toByteArray());
            if (pState != null) {
                for (Card c : phc.getCardsList()) {
                    pState.getHoleCards().add(cardToBytes(c));
                }
            }
        }

        // Store remaining deck
        for (Card c : event.getRemainingDeckList()) {
            state.getRemainingDeck().add(cardToBytes(c));
        }
    }

    private static void applyBlindPosted(HandState state, BlindPosted event) {
        PlayerHandState pState = state.getPlayer(event.getPlayerRoot().toByteArray());
        if (pState != null) {
            pState.setStack(event.getPlayerStack());
            pState.setBetThisRound(event.getAmount());
            pState.setTotalInvested(pState.getTotalInvested() + event.getAmount());
        }
        state.setPotTotal(event.getPotTotal());
        state.setCurrentBet(Math.max(state.getCurrentBet(), event.getAmount()));
    }

    private static void applyActionTaken(HandState state, ActionTaken event) {
        PlayerHandState pState = state.getPlayer(event.getPlayerRoot().toByteArray());
        if (pState != null) {
            pState.setStack(event.getPlayerStack());
            pState.setHasActed(true);
            if (event.getAction() == ActionType.FOLD) {
                pState.setHasFolded(true);
            } else if (event.getAction() == ActionType.ALL_IN) {
                pState.setAllIn(true);
            }
            pState.setBetThisRound(pState.getBetThisRound() + event.getAmount());
            pState.setTotalInvested(pState.getTotalInvested() + event.getAmount());
        }
        state.setPotTotal(event.getPotTotal());
        state.setCurrentBet(event.getAmountToCall());
    }

    private static void applyBettingRoundComplete(HandState state, BettingRoundComplete event) {
        state.setPotTotal(event.getPotTotal());
        state.setCurrentBet(0);
        // Reset bets for next round
        for (PlayerHandState p : state.getPlayers().values()) {
            p.setBetThisRound(0);
            p.setHasActed(false);
        }
    }

    private static void applyCommunityCardsDealt(HandState state, CommunityCardsDealt event) {
        for (Card c : event.getCardsList()) {
            state.getCommunityCards().add(cardToBytes(c));
        }
        state.setCurrentPhase(event.getPhaseValue());
    }

    private static void applyPotAwarded(HandState state, PotAwarded event) {
        for (PotWinner winner : event.getWinnersList()) {
            PlayerHandState pState = state.getPlayer(winner.getPlayerRoot().toByteArray());
            if (pState != null) {
                pState.setStack(pState.getStack() + winner.getAmount());
            }
        }
    }

    private static String bytesToHex(byte[] bytes) {
        if (bytes == null) return "";
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) sb.append(String.format("%02x", b));
        return sb.toString();
    }

    private static byte[] cardToBytes(Card c) {
        return new byte[]{(byte) c.getSuitValue(), (byte) c.getRankValue()};
    }
}
