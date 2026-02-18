package dev.angzarr.examples.handflow;

/**
 * Tracks a player's state within the process manager.
 */
public class PlayerProcessState {
    private byte[] playerRoot;
    private int position;
    private long stack;
    private long betThisRound;
    private long totalInvested;
    private boolean hasActed;
    private boolean hasFolded;
    private boolean isAllIn;

    public PlayerProcessState(byte[] playerRoot, int position, long stack) {
        this.playerRoot = playerRoot;
        this.position = position;
        this.stack = stack;
    }

    // Getters and setters
    public byte[] getPlayerRoot() { return playerRoot; }
    public int getPosition() { return position; }
    public long getStack() { return stack; }
    public void setStack(long stack) { this.stack = stack; }
    public long getBetThisRound() { return betThisRound; }
    public void setBetThisRound(long betThisRound) { this.betThisRound = betThisRound; }
    public long getTotalInvested() { return totalInvested; }
    public void setTotalInvested(long totalInvested) { this.totalInvested = totalInvested; }
    public boolean hasActed() { return hasActed; }
    public void setHasActed(boolean hasActed) { this.hasActed = hasActed; }
    public boolean hasFolded() { return hasFolded; }
    public void setHasFolded(boolean hasFolded) { this.hasFolded = hasFolded; }
    public boolean isAllIn() { return isAllIn; }
    public void setAllIn(boolean allIn) { isAllIn = allIn; }
}
