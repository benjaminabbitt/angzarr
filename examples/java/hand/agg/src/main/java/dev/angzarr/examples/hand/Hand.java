package dev.angzarr.examples.hand;

import dev.angzarr.client.CommandHandler;
import dev.angzarr.client.Errors;
import dev.angzarr.client.annotations.Applies;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.client.util.ByteUtils;
import dev.angzarr.examples.*;
import dev.angzarr.examples.hand.state.HandState;
import dev.angzarr.examples.hand.state.PlayerHandState;
import java.util.*;
import java.util.stream.Collectors;

/**
 * Hand aggregate with event sourcing (OO pattern).
 *
 * <p>Manages a single hand of poker with betting rounds.
 */
public class Hand extends CommandHandler<HandState> {

  public static final String DOMAIN = "hand";

  /** Default constructor. */
  public Hand() {
    super();
  }

  /** Constructor with prior events for state rehydration. */
  public Hand(dev.angzarr.EventBook eventBook) {
    super(eventBook);
  }

  @Override
  public String getDomain() {
    return DOMAIN;
  }

  @Override
  protected HandState createEmptyState() {
    return new HandState();
  }

  // --- Event appliers ---

  @Applies(CardsDealt.class)
  public void applyCardsDealt(HandState state, CardsDealt event) {
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
      state.getPlayers().put(ByteUtils.bytesToHex(p.getPlayerRoot().toByteArray()), pState);
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

  @Applies(CommunityCardsDealt.class)
  public void applyCommunityCardsDealt(HandState state, CommunityCardsDealt event) {
    for (Card c : event.getCardsList()) {
      state.getCommunityCards().add(cardToBytes(c));
    }
    state.setCurrentPhase(event.getPhaseValue());
  }

  @Applies(BlindPosted.class)
  public void applyBlindPosted(HandState state, BlindPosted event) {
    PlayerHandState pState = state.getPlayer(event.getPlayerRoot().toByteArray());
    if (pState != null) {
      pState.setStack(event.getPlayerStack());
      pState.setBetThisRound(event.getAmount());
      pState.setTotalInvested(pState.getTotalInvested() + event.getAmount());
    }
    state.setPotTotal(event.getPotTotal());
    state.setCurrentBet(Math.max(state.getCurrentBet(), event.getAmount()));
    // Track min_raise as the big blind (highest blind posted)
    if (event.getAmount() > state.getMinRaise()) {
      state.setMinRaise(event.getAmount());
    }
  }

  @Applies(ActionTaken.class)
  public void applyActionTaken(HandState state, ActionTaken event) {
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

  @Applies(BettingRoundComplete.class)
  public void applyBettingRoundComplete(HandState state, BettingRoundComplete event) {
    state.setPotTotal(event.getPotTotal());
    state.setCurrentBet(0);
    // Reset bets for next round
    for (PlayerHandState p : state.getPlayers().values()) {
      p.setBetThisRound(0);
      p.setHasActed(false);
    }
  }

  @Applies(HandComplete.class)
  public void applyHandComplete(HandState state, HandComplete event) {
    state.setStatus("complete");
  }

  @Applies(PotAwarded.class)
  public void applyPotAwarded(HandState state, PotAwarded event) {
    for (PotWinner winner : event.getWinnersList()) {
      PlayerHandState pState = state.getPlayer(winner.getPlayerRoot().toByteArray());
      if (pState != null) {
        pState.setStack(pState.getStack() + winner.getAmount());
      }
    }
    state.setStatus("complete");
  }

  @Applies(DrawCompleted.class)
  public void applyDrawCompleted(HandState state, DrawCompleted event) {
    PlayerHandState pState = state.getPlayer(event.getPlayerRoot().toByteArray());
    if (pState != null) {
      // Replace discarded cards with new cards
      pState.getHoleCards().clear();
      for (Card c : event.getNewCardsList()) {
        pState.getHoleCards().add(cardToBytes(c));
      }
    }
  }

  @Applies(ShowdownStarted.class)
  public void applyShowdownStarted(HandState state, ShowdownStarted event) {
    state.setStatus("showdown");
    state.setCurrentPhase(BettingPhase.SHOWDOWN_VALUE);
  }

  @Applies(CardsRevealed.class)
  public void applyCardsRevealed(HandState state, CardsRevealed event) {
    // No state change needed - just records revealed cards
  }

  @Applies(CardsMucked.class)
  public void applyCardsMucked(HandState state, CardsMucked event) {
    PlayerHandState pState = state.getPlayer(event.getPlayerRoot().toByteArray());
    if (pState != null) {
      pState.setHasFolded(true); // Muck is effectively a fold at showdown
    }
  }

  // --- State accessors ---
  public boolean exists() {
    return getState().exists();
  }

  public boolean isComplete() {
    return getState().isComplete();
  }

  public long getHandNumber() {
    return getState().getHandNumber();
  }

  public long getPotTotal() {
    return getState().getPotTotal();
  }

  public int getActivePlayerCount() {
    return getState().getActivePlayerCount();
  }

  public String getStatus() {
    return getState().getStatus();
  }

  public int getPlayerCount() {
    return getState().getPlayers().size();
  }

  public int getCommunityCardCount() {
    return getState().getCommunityCards().size();
  }

  public String getPhase() {
    int phase = getState().getCurrentPhase();
    if (phase == BettingPhase.PREFLOP_VALUE) return "PREFLOP";
    if (phase == BettingPhase.FLOP_VALUE) return "FLOP";
    if (phase == BettingPhase.TURN_VALUE) return "TURN";
    if (phase == BettingPhase.RIVER_VALUE) return "RIVER";
    if (phase == BettingPhase.SHOWDOWN_VALUE) return "SHOWDOWN";
    return "UNKNOWN";
  }

  public boolean hasPlayerFolded(String playerId) {
    PlayerHandState player = getState().getPlayers().get(playerId);
    if (player == null) {
      // Try hex-encoded lookup
      byte[] playerBytes = playerId.getBytes(java.nio.charset.StandardCharsets.UTF_8);
      player = getState().getPlayer(playerBytes);
    }
    return player != null && player.hasFolded();
  }

  public int getPlayerHoleCardCount(String playerId) {
    PlayerHandState player = getState().getPlayers().get(playerId);
    if (player == null) {
      byte[] playerBytes = playerId.getBytes(java.nio.charset.StandardCharsets.UTF_8);
      player = getState().getPlayer(playerBytes);
    }
    return player != null ? player.getHoleCards().size() : 0;
  }

  // --- Command handlers ---

  @Handles(DealCards.class)
  public CardsDealt dealCards(DealCards cmd) {
    if (exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Cards already dealt");
    }
    if (cmd.getPlayersCount() < 2) {
      throw Errors.CommandRejectedError.invalidArgument("Requires at least 2 players");
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
      playerCards.add(
          PlayerHoleCards.newBuilder()
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
        amount = cmd.getAmount();
        // Minimum bet is the big blind
        long minBet = getState().getMinRaise() > 0 ? getState().getMinRaise() : 10;
        if (amount < minBet && amount < player.getStack()) {
          throw Errors.CommandRejectedError.invalidArgument("Bet must be at least " + minBet);
        }
        if (amount > player.getStack()) {
          amount = player.getStack();
          action = ActionType.ALL_IN;
        }
        break;
      case RAISE:
        if (getState().getCurrentBet() == 0) {
          throw Errors.CommandRejectedError.invalidArgument("Cannot raise when there is no bet");
        }
        amount = cmd.getAmount();
        // Validate minimum raise (amount is total bet level)
        long raiseAmount = amount - getState().getCurrentBet();
        long minRaise = getState().getMinRaise() > 0 ? getState().getMinRaise() : 10;
        if (raiseAmount < minRaise && amount < player.getStack()) {
          throw Errors.CommandRejectedError.invalidArgument(
              "Raise must be at least " + minRaise + " above current bet");
        }
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
    if (getState().getGameVariant() == GameVariant.FIVE_CARD_DRAW_VALUE) {
      throw Errors.CommandRejectedError.invalidArgument(
          "Five Card Draw does not use community cards");
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
      winners.add(
          PotWinner.newBuilder()
              .setPlayerRoot(award.getPlayerRoot())
              .setAmount(award.getAmount())
              .setPotType(award.getPotType())
              .build());
    }

    return PotAwarded.newBuilder().addAllWinners(winners).setAwardedAt(now()).build();
  }

  @Handles(RequestDraw.class)
  public DrawCompleted requestDraw(RequestDraw cmd) {
    if (!exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
    }
    if (getState().getGameVariant() != GameVariant.FIVE_CARD_DRAW_VALUE) {
      throw Errors.CommandRejectedError.invalidArgument("Draw not supported in this game variant");
    }
    PlayerHandState player = getState().getPlayer(cmd.getPlayerRoot().toByteArray());
    if (player == null) {
      throw Errors.CommandRejectedError.preconditionFailed("Player not in hand");
    }

    int discardCount = cmd.getCardIndicesCount();
    List<byte[]> remaining = getState().getRemainingDeck();
    List<Card> newCards = new ArrayList<>();

    // Get new cards from remaining deck
    for (int i = 0; i < discardCount && i < remaining.size(); i++) {
      newCards.add(bytesToCard(remaining.get(i)));
    }

    // Build full hand with replacements
    List<Card> fullHand = new ArrayList<>();
    Set<Integer> discardSet = new HashSet<>();
    for (int idx : cmd.getCardIndicesList()) {
      discardSet.add(idx);
    }

    int newCardIndex = 0;
    for (int i = 0; i < player.getHoleCards().size(); i++) {
      if (discardSet.contains(i)) {
        if (newCardIndex < newCards.size()) {
          fullHand.add(newCards.get(newCardIndex++));
        }
      } else {
        fullHand.add(bytesToCard(player.getHoleCards().get(i)));
      }
    }

    return DrawCompleted.newBuilder()
        .setPlayerRoot(cmd.getPlayerRoot())
        .setCardsDiscarded(discardCount)
        .setCardsDrawn(newCards.size())
        .addAllNewCards(fullHand)
        .setDrawnAt(now())
        .build();
  }

  @Handles(RevealCards.class)
  public com.google.protobuf.Message revealCards(RevealCards cmd) {
    if (!exists()) {
      throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
    }
    PlayerHandState player = getState().getPlayer(cmd.getPlayerRoot().toByteArray());
    if (player == null) {
      throw Errors.CommandRejectedError.preconditionFailed("Player not in hand");
    }

    if (cmd.getMuck()) {
      return CardsMucked.newBuilder().setPlayerRoot(cmd.getPlayerRoot()).setMuckedAt(now()).build();
    }

    // Get player's hole cards
    List<Card> holeCards = new ArrayList<>();
    for (byte[] cardBytes : player.getHoleCards()) {
      holeCards.add(bytesToCard(cardBytes));
    }

    // Get community cards
    List<Card> communityCards = new ArrayList<>();
    for (byte[] cardBytes : getState().getCommunityCards()) {
      communityCards.add(bytesToCard(cardBytes));
    }

    // Evaluate hand
    HandRanking ranking = evaluateHand(holeCards, communityCards);

    return CardsRevealed.newBuilder()
        .setPlayerRoot(cmd.getPlayerRoot())
        .addAllCards(holeCards)
        .setRanking(ranking)
        .setRevealedAt(now())
        .build();
  }

  // --- Helper methods ---

  private List<Card> createShuffledDeck(byte[] seed) {
    List<Card> deck = new ArrayList<>();
    for (Suit suit : new Suit[] {Suit.CLUBS, Suit.DIAMONDS, Suit.HEARTS, Suit.SPADES}) {
      for (int rank = 2; rank <= 14; rank++) {
        deck.add(Card.newBuilder().setSuit(suit).setRank(Rank.forNumber(rank)).build());
      }
    }
    // Shuffle using seed
    Random rng = seed.length > 0 ? new Random(ByteUtils.bytesToLong(seed)) : new Random();
    Collections.shuffle(deck, rng);
    return deck;
  }

  private int getHoleCardCount(GameVariant variant) {
    switch (variant) {
      case OMAHA:
        return 4;
      case FIVE_CARD_DRAW:
        return 5;
      default:
        return 2; // Texas Hold'em, 7-card stud
    }
  }

  private BettingPhase determineNextPhase() {
    int current = getState().getCurrentPhase();
    if (current == BettingPhase.PREFLOP_VALUE) return BettingPhase.FLOP;
    if (current == BettingPhase.FLOP_VALUE) return BettingPhase.TURN;
    if (current == BettingPhase.TURN_VALUE) return BettingPhase.RIVER;
    return BettingPhase.SHOWDOWN;
  }

  private static byte[] cardToBytes(Card c) {
    return new byte[] {(byte) c.getSuitValue(), (byte) c.getRankValue()};
  }

  private static Card bytesToCard(byte[] bytes) {
    if (bytes == null || bytes.length < 2) {
      throw new IllegalArgumentException("Invalid card bytes: expected 2 bytes");
    }
    return Card.newBuilder()
        .setSuit(Suit.forNumber(bytes[0]))
        .setRank(Rank.forNumber(bytes[1]))
        .build();
  }

  // --- Hand Evaluation ---

  private HandRanking evaluateHand(List<Card> holeCards, List<Card> communityCards) {
    List<Card> allCards = new ArrayList<>();
    allCards.addAll(holeCards);
    allCards.addAll(communityCards);

    // Sort by rank descending
    allCards.sort((a, b) -> b.getRankValue() - a.getRankValue());

    // Group by suit and rank
    Map<Suit, List<Card>> bySuit = new HashMap<>();
    Map<Integer, List<Card>> byRank = new HashMap<>();
    for (Card c : allCards) {
      bySuit.computeIfAbsent(c.getSuit(), k -> new ArrayList<>()).add(c);
      byRank.computeIfAbsent(c.getRankValue(), k -> new ArrayList<>()).add(c);
    }

    // Check for flush
    List<Card> flushCards = null;
    for (List<Card> suited : bySuit.values()) {
      if (suited.size() >= 5) {
        flushCards = suited.subList(0, 5);
        break;
      }
    }

    // Check for straight
    List<Card> straightCards = findStraight(allCards);

    // Check for straight flush / royal flush
    if (flushCards != null) {
      List<Card> straightFlush = findStraight(flushCards);
      // Fall back: check if the regular straight is all one suit
      if (straightFlush == null && straightCards != null && isSameSuit(straightCards)) {
        straightFlush = straightCards;
      }
      if (straightFlush != null) {
        if (straightFlush.get(0).getRankValue() == Rank.ACE_VALUE) {
          return HandRanking.newBuilder()
              .setRankType(HandRankType.ROYAL_FLUSH)
              .setScore(1000)
              .build();
        }
        return HandRanking.newBuilder()
            .setRankType(HandRankType.STRAIGHT_FLUSH)
            .addKickers(straightFlush.get(0).getRank())
            .setScore(900 + straightFlush.get(0).getRankValue())
            .build();
      }
    }

    // Count pairs, trips, quads
    List<Integer> quads = new ArrayList<>();
    List<Integer> trips = new ArrayList<>();
    List<Integer> pairs = new ArrayList<>();
    for (Map.Entry<Integer, List<Card>> entry : byRank.entrySet()) {
      int count = entry.getValue().size();
      if (count == 4) quads.add(entry.getKey());
      else if (count == 3) trips.add(entry.getKey());
      else if (count == 2) pairs.add(entry.getKey());
    }
    quads.sort(Collections.reverseOrder());
    trips.sort(Collections.reverseOrder());
    pairs.sort(Collections.reverseOrder());

    // Four of a kind
    if (!quads.isEmpty()) {
      return HandRanking.newBuilder()
          .setRankType(HandRankType.FOUR_OF_A_KIND)
          .addKickers(Rank.forNumber(quads.get(0)))
          .setScore(800 + quads.get(0))
          .build();
    }

    // Full house
    if (!trips.isEmpty() && (!pairs.isEmpty() || trips.size() > 1)) {
      int pairRank = !pairs.isEmpty() ? pairs.get(0) : (trips.size() > 1 ? trips.get(1) : 0);
      return HandRanking.newBuilder()
          .setRankType(HandRankType.FULL_HOUSE)
          .addKickers(Rank.forNumber(trips.get(0)))
          .addKickers(Rank.forNumber(pairRank))
          .setScore(700 + trips.get(0) * 10 + pairRank)
          .build();
    }

    // Flush
    if (flushCards != null) {
      return HandRanking.newBuilder()
          .setRankType(HandRankType.FLUSH)
          .addKickers(flushCards.get(0).getRank())
          .setScore(600 + flushCards.get(0).getRankValue())
          .build();
    }

    // Straight
    if (straightCards != null) {
      return HandRanking.newBuilder()
          .setRankType(HandRankType.STRAIGHT)
          .addKickers(straightCards.get(0).getRank())
          .setScore(500 + straightCards.get(0).getRankValue())
          .build();
    }

    // Three of a kind
    if (!trips.isEmpty()) {
      return HandRanking.newBuilder()
          .setRankType(HandRankType.THREE_OF_A_KIND)
          .addKickers(Rank.forNumber(trips.get(0)))
          .setScore(400 + trips.get(0))
          .build();
    }

    // Two pair
    if (pairs.size() >= 2) {
      return HandRanking.newBuilder()
          .setRankType(HandRankType.TWO_PAIR)
          .addKickers(Rank.forNumber(pairs.get(0)))
          .addKickers(Rank.forNumber(pairs.get(1)))
          .setScore(300 + pairs.get(0) * 10 + pairs.get(1))
          .build();
    }

    // Pair
    if (!pairs.isEmpty()) {
      return HandRanking.newBuilder()
          .setRankType(HandRankType.PAIR)
          .addKickers(Rank.forNumber(pairs.get(0)))
          .setScore(200 + pairs.get(0))
          .build();
    }

    // High card
    return HandRanking.newBuilder()
        .setRankType(HandRankType.HIGH_CARD)
        .addKickers(allCards.get(0).getRank())
        .setScore(100 + allCards.get(0).getRankValue())
        .build();
  }

  private List<Card> findStraight(List<Card> cards) {
    if (cards.size() < 5) return null;

    // Get unique ranks sorted descending
    List<Integer> ranks =
        cards.stream()
            .map(Card::getRankValue)
            .distinct()
            .sorted(Collections.reverseOrder())
            .collect(Collectors.toList());

    // Check for wheel (A-2-3-4-5)
    if (ranks.contains(Rank.ACE_VALUE)
        && ranks.contains(2)
        && ranks.contains(3)
        && ranks.contains(4)
        && ranks.contains(5)) {
      return cards.stream()
          .filter(
              c ->
                  c.getRankValue() == 5
                      || c.getRankValue() == 4
                      || c.getRankValue() == 3
                      || c.getRankValue() == 2
                      || c.getRankValue() == Rank.ACE_VALUE)
          .limit(5)
          .collect(Collectors.toList());
    }

    // Check for regular straight
    for (int i = 0; i <= ranks.size() - 5; i++) {
      boolean isStraight = true;
      for (int j = 0; j < 4; j++) {
        if (ranks.get(i + j) - ranks.get(i + j + 1) != 1) {
          isStraight = false;
          break;
        }
      }
      if (isStraight) {
        int highRank = ranks.get(i);
        return cards.stream()
            .filter(c -> c.getRankValue() >= highRank - 4 && c.getRankValue() <= highRank)
            .limit(5)
            .collect(Collectors.toList());
      }
    }

    return null;
  }

  private boolean isSameSuit(List<Card> cards) {
    if (cards == null || cards.isEmpty()) return false;
    Suit first = cards.get(0).getSuit();
    return cards.stream().allMatch(c -> c.getSuit() == first);
  }
}
