package dev.angzarr.examples.player.state;

import java.util.HashMap;
import java.util.Map;

/**
 * Internal state representation for the Player aggregate.
 *
 * <p>This is a mutable state object that gets updated as events are applied.
 * It is not exposed externally - the aggregate provides read-only access
 * via properties.
 */
public class PlayerState {

    private String playerId = "";
    private String displayName = "";
    private String email = "";
    private int playerType = 0;
    private String aiModelId = "";
    private long bankroll = 0;
    private long reservedFunds = 0;
    private final Map<String, Long> tableReservations = new HashMap<>();
    private String status = "";

    public String getPlayerId() {
        return playerId;
    }

    public void setPlayerId(String playerId) {
        this.playerId = playerId;
    }

    public String getDisplayName() {
        return displayName;
    }

    public void setDisplayName(String displayName) {
        this.displayName = displayName;
    }

    public String getEmail() {
        return email;
    }

    public void setEmail(String email) {
        this.email = email;
    }

    public int getPlayerType() {
        return playerType;
    }

    public void setPlayerType(int playerType) {
        this.playerType = playerType;
    }

    public String getAiModelId() {
        return aiModelId;
    }

    public void setAiModelId(String aiModelId) {
        this.aiModelId = aiModelId;
    }

    public long getBankroll() {
        return bankroll;
    }

    public void setBankroll(long bankroll) {
        this.bankroll = bankroll;
    }

    public long getReservedFunds() {
        return reservedFunds;
    }

    public void setReservedFunds(long reservedFunds) {
        this.reservedFunds = reservedFunds;
    }

    public Map<String, Long> getTableReservations() {
        return tableReservations;
    }

    public String getStatus() {
        return status;
    }

    public void setStatus(String status) {
        this.status = status;
    }

    /**
     * Check if the player exists (has been registered).
     */
    public boolean exists() {
        return !playerId.isEmpty();
    }

    /**
     * Get available balance (bankroll minus reserved).
     */
    public long getAvailableBalance() {
        return bankroll - reservedFunds;
    }

    /**
     * Check if this is an AI player.
     */
    public boolean isAi() {
        return playerType == 1; // PlayerType.AI = 1
    }

    /**
     * Get reservation amount for a specific table.
     */
    public long getReservationForTable(String tableRootHex) {
        return tableReservations.getOrDefault(tableRootHex, 0L);
    }

    /**
     * Check if funds are reserved for a specific table.
     */
    public boolean hasReservationFor(String tableRootHex) {
        return tableReservations.containsKey(tableRootHex);
    }
}
