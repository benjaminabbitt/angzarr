package dev.angzarr.examples.hand.state;

import java.util.ArrayList;
import java.util.List;

/**
 * Player state within a hand.
 */
public class PlayerHandState {

    private byte[] playerRoot;
    private int position;
    private List<byte[]> holeCards = new ArrayList<>();
    private long stack;
    private long betThisRound;
    private long totalInvested;
    private boolean hasActed;
    private boolean hasFolded;
    private boolean isAllIn;

    public byte[] getPlayerRoot() { return playerRoot; }
    public void setPlayerRoot(byte[] playerRoot) { this.playerRoot = playerRoot; }

    public int getPosition() { return position; }
    public void setPosition(int position) { this.position = position; }

    public List<byte[]> getHoleCards() { return holeCards; }

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
