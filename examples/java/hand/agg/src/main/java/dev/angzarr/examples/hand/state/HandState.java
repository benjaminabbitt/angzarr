package dev.angzarr.examples.hand.state;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Internal state for Hand aggregate.
 */
public class HandState {

    private String handId = "";
    private byte[] tableRoot = new byte[0];
    private long handNumber = 0;
    private int gameVariant = 0;
    private List<byte[]> remainingDeck = new ArrayList<>();
    private List<byte[]> communityCards = new ArrayList<>();
    private Map<String, PlayerHandState> players = new HashMap<>();
    private int currentPhase = 0; // BettingPhase enum value
    private int actionOnPosition = -1;
    private long currentBet = 0;
    private long minRaise = 0;
    private long potTotal = 0;
    private int dealerPosition = 0;
    private int smallBlindPosition = 0;
    private int bigBlindPosition = 0;
    private String status = "";

    // Getters and setters
    public String getHandId() { return handId; }
    public void setHandId(String handId) { this.handId = handId; }

    public byte[] getTableRoot() { return tableRoot; }
    public void setTableRoot(byte[] tableRoot) { this.tableRoot = tableRoot; }

    public long getHandNumber() { return handNumber; }
    public void setHandNumber(long handNumber) { this.handNumber = handNumber; }

    public int getGameVariant() { return gameVariant; }
    public void setGameVariant(int gameVariant) { this.gameVariant = gameVariant; }

    public List<byte[]> getRemainingDeck() { return remainingDeck; }
    public List<byte[]> getCommunityCards() { return communityCards; }

    public Map<String, PlayerHandState> getPlayers() { return players; }

    public int getCurrentPhase() { return currentPhase; }
    public void setCurrentPhase(int currentPhase) { this.currentPhase = currentPhase; }

    public int getActionOnPosition() { return actionOnPosition; }
    public void setActionOnPosition(int actionOnPosition) { this.actionOnPosition = actionOnPosition; }

    public long getCurrentBet() { return currentBet; }
    public void setCurrentBet(long currentBet) { this.currentBet = currentBet; }

    public long getMinRaise() { return minRaise; }
    public void setMinRaise(long minRaise) { this.minRaise = minRaise; }

    public long getPotTotal() { return potTotal; }
    public void setPotTotal(long potTotal) { this.potTotal = potTotal; }

    public int getDealerPosition() { return dealerPosition; }
    public void setDealerPosition(int dealerPosition) { this.dealerPosition = dealerPosition; }

    public int getSmallBlindPosition() { return smallBlindPosition; }
    public void setSmallBlindPosition(int smallBlindPosition) { this.smallBlindPosition = smallBlindPosition; }

    public int getBigBlindPosition() { return bigBlindPosition; }
    public void setBigBlindPosition(int bigBlindPosition) { this.bigBlindPosition = bigBlindPosition; }

    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }

    public boolean exists() { return !handId.isEmpty(); }
    public boolean isComplete() { return "complete".equals(status); }

    public PlayerHandState getPlayer(byte[] playerRoot) {
        return players.get(bytesToHex(playerRoot));
    }

    public int getActivePlayerCount() {
        return (int) players.values().stream()
            .filter(p -> !p.hasFolded() && !p.isAllIn())
            .count();
    }

    private static String bytesToHex(byte[] bytes) {
        if (bytes == null) return "";
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) sb.append(String.format("%02x", b));
        return sb.toString();
    }
}
