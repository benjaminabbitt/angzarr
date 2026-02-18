package dev.angzarr.examples.table.state;

/**
 * Represents a seat at the table.
 */
public class SeatState {

    private int position;
    private byte[] playerRoot;
    private long stack;
    private boolean active;
    private boolean sittingOut;

    public SeatState() {}

    public SeatState(int position) {
        this.position = position;
    }

    public int getPosition() {
        return position;
    }

    public void setPosition(int position) {
        this.position = position;
    }

    public byte[] getPlayerRoot() {
        return playerRoot;
    }

    public void setPlayerRoot(byte[] playerRoot) {
        this.playerRoot = playerRoot;
    }

    public long getStack() {
        return stack;
    }

    public void setStack(long stack) {
        this.stack = stack;
    }

    public boolean isActive() {
        return active && !sittingOut && playerRoot != null && playerRoot.length > 0;
    }

    public void setActive(boolean active) {
        this.active = active;
    }

    public boolean isSittingOut() {
        return sittingOut;
    }

    public void setSittingOut(boolean sittingOut) {
        this.sittingOut = sittingOut;
    }

    public boolean isOccupied() {
        return playerRoot != null && playerRoot.length > 0;
    }
}
