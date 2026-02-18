package dev.angzarr.examples.hand;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.client.Aggregate;
import dev.angzarr.client.Errors;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.examples.hand.state.HandState;
import dev.angzarr.examples.hand.state.PlayerHandState;
import dev.angzarr.examples.*;

import java.util.*;

/**
 * Hand aggregate with event sourcing (OO pattern).
 *
 * <p>Manages a single hand of poker with betting rounds.
 */
public class Hand extends Aggregate<HandState> {

    public static final String DOMAIN = "hand";

    @Override
    public String getDomain() {
        return DOMAIN;
    }

    @Override
    protected HandState createEmptyState() {
        return new HandState();
    }

    @Override
    protected void applyEvent(HandState state, Any eventAny) {
        String typeUrl = eventAny.getTypeUrl();

        try {
            if (typeUrl.endsWith("CardsDealt")) {
                CardsDealt event = eventAny.unpack(CardsDealt.class);
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

            } else if (typeUrl.endsWith("BlindPosted")) {
                BlindPosted event = eventAny.unpack(BlindPosted.class);
                PlayerHandState pState = state.getPlayer(event.getPlayerRoot().toByteArray());
                if (pState != null) {
                    pState.setStack(event.getPlayerStack());
                    pState.setBetThisRound(event.getAmount());
                    pState.setTotalInvested(pState.getTotalInvested() + event.getAmount());
                }
                state.setPotTotal(event.getPotTotal());
                state.setCurrentBet(Math.max(state.getCurrentBet(), event.getAmount()));

            } else if (typeUrl.endsWith("ActionTaken")) {
                ActionTaken event = eventAny.unpack(ActionTaken.class);
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

            } else if (typeUrl.endsWith("BettingRoundComplete")) {
                BettingRoundComplete event = eventAny.unpack(BettingRoundComplete.class);
                state.setPotTotal(event.getPotTotal());
                state.setCurrentBet(0);
                // Reset bets for next round
                for (PlayerHandState p : state.getPlayers().values()) {
                    p.setBetThisRound(0);
                    p.setHasActed(false);
                }

            } else if (typeUrl.endsWith("CommunityCardsDealt")) {
                CommunityCardsDealt event = eventAny.unpack(CommunityCardsDealt.class);
                for (Card c : event.getCardsList()) {
                    state.getCommunityCards().add(cardToBytes(c));
                }
                state.setCurrentPhase(event.getPhaseValue());

            } else if (typeUrl.endsWith("HandComplete")) {
                state.setStatus("complete");

            } else if (typeUrl.endsWith("PotAwarded")) {
                PotAwarded event = eventAny.unpack(PotAwarded.class);
                for (PotWinner winner : event.getWinnersList()) {
                    PlayerHandState pState = state.getPlayer(winner.getPlayerRoot().toByteArray());
                    if (pState != null) {
                        pState.setStack(pState.getStack() + winner.getAmount());
                    }
                }
            }
        } catch (InvalidProtocolBufferException e) {
            throw new RuntimeException("Failed to unpack event: " + typeUrl, e);
        }
    }

    // --- State accessors ---
    public boolean exists() { return getState().exists(); }
    public boolean isComplete() { return getState().isComplete(); }
    public long getHandNumber() { return getState().getHandNumber(); }
    public long getPotTotal() { return getState().getPotTotal(); }
    public int getActivePlayerCount() { return getState().getActivePlayerCount(); }

    // --- Command handlers ---

    @Handles(DealCards.class)
    public CardsDealt dealCards(DealCards cmd) {
        if (exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand already exists");
        }
        if (cmd.getPlayersList().isEmpty()) {
            throw Errors.CommandRejectedError.invalidArgument("players required");
        }

        // Generate hole cards for each player
        List<Card> deck = createShuffledDeck(cmd.getDeckSeed().toByteArray());
        List<PlayerHoleCards> playerCards = new ArrayList<>();
        int cardsPerPlayer = getHoleCardCount(cmd.getGameVariant());

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

    @Handles(PostBlind.class)
    public BlindPosted postBlind(PostBlind cmd) {
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
        }
        PlayerHandState player = getState().getPlayer(cmd.getPlayerRoot().toByteArray());
        if (player == null) {
            throw Errors.CommandRejectedError.preconditionFailed("Player not in hand");
        }

        long amount = Math.min(cmd.getAmount(), player.getStack());
        long newStack = player.getStack() - amount;
        long newPot = getState().getPotTotal() + amount;

        return BlindPosted.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setBlindType(cmd.getBlindType())
            .setAmount(amount)
            .setPlayerStack(newStack)
            .setPotTotal(newPot)
            .setPostedAt(now())
            .build();
    }

    @Handles(PlayerAction.class)
    public ActionTaken playerAction(PlayerAction cmd) {
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
        }
        if (isComplete()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand is complete");
        }
        PlayerHandState player = getState().getPlayer(cmd.getPlayerRoot().toByteArray());
        if (player == null) {
            throw Errors.CommandRejectedError.preconditionFailed("Player not in hand");
        }
        if (player.hasFolded()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player has folded");
        }

        long amount = 0;
        ActionType action = cmd.getAction();

        switch (action) {
            case FOLD:
                break;
            case CHECK:
                if (getState().getCurrentBet() > player.getBetThisRound()) {
                    throw Errors.CommandRejectedError.invalidArgument("Cannot check, must call or fold");
                }
                break;
            case CALL:
                amount = getState().getCurrentBet() - player.getBetThisRound();
                amount = Math.min(amount, player.getStack());
                break;
            case BET:
            case RAISE:
                amount = cmd.getAmount();
                if (amount > player.getStack()) {
                    amount = player.getStack();
                    action = ActionType.ALL_IN;
                }
                break;
            case ALL_IN:
                amount = player.getStack();
                break;
            default:
                throw Errors.CommandRejectedError.invalidArgument("Invalid action");
        }

        long newStack = player.getStack() - amount;
        long newPot = getState().getPotTotal() + amount;
        long amountToCall = Math.max(getState().getCurrentBet(), player.getBetThisRound() + amount);

        return ActionTaken.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setAction(action)
            .setAmount(amount)
            .setPlayerStack(newStack)
            .setPotTotal(newPot)
            .setAmountToCall(amountToCall)
            .setActionAt(now())
            .build();
    }

    @Handles(DealCommunityCards.class)
    public CommunityCardsDealt dealCommunityCards(DealCommunityCards cmd) {
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
        }

        List<byte[]> remaining = getState().getRemainingDeck();
        List<Card> newCards = new ArrayList<>();
        for (int i = 0; i < cmd.getCount() && i < remaining.size(); i++) {
            newCards.add(bytesToCard(remaining.get(i)));
        }

        BettingPhase nextPhase = determineNextPhase();

        List<Card> allCommunity = new ArrayList<>();
        for (byte[] c : getState().getCommunityCards()) {
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

    @Handles(AwardPot.class)
    public PotAwarded awardPot(AwardPot cmd) {
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
        }

        List<PotWinner> winners = new ArrayList<>();
        for (PotAward award : cmd.getAwardsList()) {
            winners.add(PotWinner.newBuilder()
                .setPlayerRoot(award.getPlayerRoot())
                .setAmount(award.getAmount())
                .setPotType(award.getPotType())
                .build());
        }

        return PotAwarded.newBuilder()
            .addAllWinners(winners)
            .setAwardedAt(now())
            .build();
    }

    // --- Helper methods ---

    private List<Card> createShuffledDeck(byte[] seed) {
        List<Card> deck = new ArrayList<>();
        for (Suit suit : new Suit[]{Suit.CLUBS, Suit.DIAMONDS, Suit.HEARTS, Suit.SPADES}) {
            for (int rank = 2; rank <= 14; rank++) {
                deck.add(Card.newBuilder().setSuit(suit).setRank(Rank.forNumber(rank)).build());
            }
        }
        // Shuffle using seed
        Random rng = seed.length > 0 ? new Random(bytesToLong(seed)) : new Random();
        Collections.shuffle(deck, rng);
        return deck;
    }

    private int getHoleCardCount(GameVariant variant) {
        switch (variant) {
            case OMAHA: return 4;
            case FIVE_CARD_DRAW: return 5;
            default: return 2; // Texas Hold'em, 7-card stud
        }
    }

    private BettingPhase determineNextPhase() {
        int current = getState().getCurrentPhase();
        if (current == BettingPhase.PREFLOP_VALUE) return BettingPhase.FLOP;
        if (current == BettingPhase.FLOP_VALUE) return BettingPhase.TURN;
        if (current == BettingPhase.TURN_VALUE) return BettingPhase.RIVER;
        return BettingPhase.SHOWDOWN;
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

    private static Card bytesToCard(byte[] bytes) {
        return Card.newBuilder()
            .setSuit(Suit.forNumber(bytes[0]))
            .setRank(Rank.forNumber(bytes[1]))
            .build();
    }

    private static long bytesToLong(byte[] bytes) {
        long result = 0;
        for (int i = 0; i < Math.min(8, bytes.length); i++) {
            result = (result << 8) | (bytes[i] & 0xFF);
        }
        return result;
    }
}
