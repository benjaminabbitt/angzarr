package dev.angzarr.examples.table.state;

import java.util.HashMap;
import java.util.Map;

/**
 * Internal state representation for the Table aggregate.
 */
public class TableState {

    private String tableId = "";
    private String tableName = "";
    private int gameVariant = 0;
    private long smallBlind = 0;
    private long bigBlind = 0;
    private long minBuyIn = 0;
    private long maxBuyIn = 0;
    private int maxPlayers = 0;
    private int actionTimeoutSeconds = 0;
    private final Map<Integer, SeatState> seats = new HashMap<>();
    private int dealerPosition = -1;
    private long handCount = 0;
    private byte[] currentHandRoot = new byte[0];
    private String status = "";

    // Getters and Setters

    public String getTableId() {
        return tableId;
    }

    public void setTableId(String tableId) {
        this.tableId = tableId;
    }

    public String getTableName() {
        return tableName;
    }

    public void setTableName(String tableName) {
        this.tableName = tableName;
    }

    public int getGameVariant() {
        return gameVariant;
    }

    public void setGameVariant(int gameVariant) {
        this.gameVariant = gameVariant;
    }

    public long getSmallBlind() {
        return smallBlind;
    }

    public void setSmallBlind(long smallBlind) {
        this.smallBlind = smallBlind;
    }

    public long getBigBlind() {
        return bigBlind;
    }

    public void setBigBlind(long bigBlind) {
        this.bigBlind = bigBlind;
    }

    public long getMinBuyIn() {
        return minBuyIn;
    }

    public void setMinBuyIn(long minBuyIn) {
        this.minBuyIn = minBuyIn;
    }

    public long getMaxBuyIn() {
        return maxBuyIn;
    }

    public void setMaxBuyIn(long maxBuyIn) {
        this.maxBuyIn = maxBuyIn;
    }

    public int getMaxPlayers() {
        return maxPlayers;
    }

    public void setMaxPlayers(int maxPlayers) {
        this.maxPlayers = maxPlayers;
    }

    public int getActionTimeoutSeconds() {
        return actionTimeoutSeconds;
    }

    public void setActionTimeoutSeconds(int actionTimeoutSeconds) {
        this.actionTimeoutSeconds = actionTimeoutSeconds;
    }

    public Map<Integer, SeatState> getSeats() {
        return seats;
    }

    public int getDealerPosition() {
        return dealerPosition;
    }

    public void setDealerPosition(int dealerPosition) {
        this.dealerPosition = dealerPosition;
    }

    public long getHandCount() {
        return handCount;
    }

    public void setHandCount(long handCount) {
        this.handCount = handCount;
    }

    public byte[] getCurrentHandRoot() {
        return currentHandRoot;
    }

    public void setCurrentHandRoot(byte[] currentHandRoot) {
        this.currentHandRoot = currentHandRoot;
    }

    public String getStatus() {
        return status;
    }

    public void setStatus(String status) {
        this.status = status;
    }

    // Helper methods

    public boolean exists() {
        return !tableId.isEmpty();
    }

    public boolean isInHand() {
        return "in_hand".equals(status);
    }

    public int getPlayerCount() {
        return (int) seats.values().stream()
            .filter(s -> s.getPlayerRoot() != null && s.getPlayerRoot().length > 0)
            .count();
    }

    public int getActivePlayerCount() {
        return (int) seats.values().stream()
            .filter(SeatState::isActive)
            .count();
    }

    public SeatState getSeat(int position) {
        return seats.get(position);
    }

    public SeatState findSeatByPlayer(byte[] playerRoot) {
        String playerHex = bytesToHex(playerRoot);
        return seats.values().stream()
            .filter(s -> playerHex.equals(bytesToHex(s.getPlayerRoot())))
            .findFirst()
            .orElse(null);
    }

    public int findAvailableSeat() {
        for (int i = 0; i < maxPlayers; i++) {
            if (!seats.containsKey(i) || seats.get(i).getPlayerRoot() == null ||
                seats.get(i).getPlayerRoot().length == 0) {
                return i;
            }
        }
        return -1;
    }

    public int advanceDealerPosition() {
        if (getActivePlayerCount() == 0) {
            return 0;
        }
        int nextPosition = (dealerPosition + 1) % maxPlayers;
        while (!seats.containsKey(nextPosition) || !seats.get(nextPosition).isActive()) {
            nextPosition = (nextPosition + 1) % maxPlayers;
        }
        return nextPosition;
    }

    private static String bytesToHex(byte[] bytes) {
        if (bytes == null) return "";
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
