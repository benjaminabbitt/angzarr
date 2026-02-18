package dev.angzarr.examples.handflow;

import java.time.Instant;
import java.util.*;

/**
 * Process manager state for a single hand.
 *
 * <p>This tracks the orchestration state separately from the
 * domain state in the hand aggregate. It coordinates:
 * - Phase transitions
 * - Action timeouts
 * - Next player to act
 * - Blind posting sequence
 */
public class HandProcess {
    private String handId = "";
    private byte[] tableRoot = new byte[0];
    private long handNumber;
    private int gameVariant;

    // State machine
    private HandPhase phase = HandPhase.WAITING_FOR_START;
    private int bettingPhase; // BettingPhase enum value

    // Player tracking
    private Map<Integer, PlayerProcessState> players = new HashMap<>();
    private List<Integer> activePositions = new ArrayList<>();

    // Position tracking
    private int dealerPosition;
    private int smallBlindPosition;
    private int bigBlindPosition;
    private int actionOn = -1;
    private int lastAggressor = -1;

    // Betting state
    private long smallBlind;
    private long bigBlind;
    private long currentBet;
    private long minRaise;
    private long potTotal;

    // Blind posting progress
    private boolean smallBlindPosted;
    private boolean bigBlindPosted;

    // Timeout handling
    private int actionTimeoutSeconds = 30;
    private Instant actionStartedAt;

    // Community cards (for phase tracking)
    private int communityCardCount;

    // Getters and setters
    public String getHandId() { return handId; }
    public void setHandId(String handId) { this.handId = handId; }
    public byte[] getTableRoot() { return tableRoot; }
    public void setTableRoot(byte[] tableRoot) { this.tableRoot = tableRoot; }
    public long getHandNumber() { return handNumber; }
    public void setHandNumber(long handNumber) { this.handNumber = handNumber; }
    public int getGameVariant() { return gameVariant; }
    public void setGameVariant(int gameVariant) { this.gameVariant = gameVariant; }
    public HandPhase getPhase() { return phase; }
    public void setPhase(HandPhase phase) { this.phase = phase; }
    public int getBettingPhase() { return bettingPhase; }
    public void setBettingPhase(int bettingPhase) { this.bettingPhase = bettingPhase; }
    public Map<Integer, PlayerProcessState> getPlayers() { return players; }
    public List<Integer> getActivePositions() { return activePositions; }
    public int getDealerPosition() { return dealerPosition; }
    public void setDealerPosition(int dealerPosition) { this.dealerPosition = dealerPosition; }
    public int getSmallBlindPosition() { return smallBlindPosition; }
    public void setSmallBlindPosition(int smallBlindPosition) { this.smallBlindPosition = smallBlindPosition; }
    public int getBigBlindPosition() { return bigBlindPosition; }
    public void setBigBlindPosition(int bigBlindPosition) { this.bigBlindPosition = bigBlindPosition; }
    public int getActionOn() { return actionOn; }
    public void setActionOn(int actionOn) { this.actionOn = actionOn; }
    public int getLastAggressor() { return lastAggressor; }
    public void setLastAggressor(int lastAggressor) { this.lastAggressor = lastAggressor; }
    public long getSmallBlind() { return smallBlind; }
    public void setSmallBlind(long smallBlind) { this.smallBlind = smallBlind; }
    public long getBigBlind() { return bigBlind; }
    public void setBigBlind(long bigBlind) { this.bigBlind = bigBlind; }
    public long getCurrentBet() { return currentBet; }
    public void setCurrentBet(long currentBet) { this.currentBet = currentBet; }
    public long getMinRaise() { return minRaise; }
    public void setMinRaise(long minRaise) { this.minRaise = minRaise; }
    public long getPotTotal() { return potTotal; }
    public void setPotTotal(long potTotal) { this.potTotal = potTotal; }
    public boolean isSmallBlindPosted() { return smallBlindPosted; }
    public void setSmallBlindPosted(boolean smallBlindPosted) { this.smallBlindPosted = smallBlindPosted; }
    public boolean isBigBlindPosted() { return bigBlindPosted; }
    public void setBigBlindPosted(boolean bigBlindPosted) { this.bigBlindPosted = bigBlindPosted; }
    public int getActionTimeoutSeconds() { return actionTimeoutSeconds; }
    public void setActionTimeoutSeconds(int actionTimeoutSeconds) { this.actionTimeoutSeconds = actionTimeoutSeconds; }
    public Instant getActionStartedAt() { return actionStartedAt; }
    public void setActionStartedAt(Instant actionStartedAt) { this.actionStartedAt = actionStartedAt; }
    public int getCommunityCardCount() { return communityCardCount; }
    public void setCommunityCardCount(int communityCardCount) { this.communityCardCount = communityCardCount; }
}
