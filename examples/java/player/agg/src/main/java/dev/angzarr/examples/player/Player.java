package dev.angzarr.examples.player;

import dev.angzarr.client.Aggregate;
import dev.angzarr.client.Errors;
import dev.angzarr.client.annotations.Applies;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.client.annotations.Rejected;
import dev.angzarr.examples.player.state.PlayerState;
import dev.angzarr.examples.Currency;
import dev.angzarr.examples.DepositFunds;
import dev.angzarr.examples.FundsDeposited;
import dev.angzarr.examples.FundsReleased;
import dev.angzarr.examples.FundsReserved;
import dev.angzarr.examples.FundsWithdrawn;
import dev.angzarr.examples.PlayerRegistered;
import dev.angzarr.examples.RegisterPlayer;
import dev.angzarr.examples.ReleaseFunds;
import dev.angzarr.examples.ReserveFunds;
import dev.angzarr.examples.WithdrawFunds;

/**
 * Player aggregate with event sourcing (OO pattern).
 *
 * <p>Manages player identity, bankroll, and table reservations.
 * Uses the guard/validate/compute pattern in command handlers.
 */
public class Player extends Aggregate<PlayerState> {

    public static final String DOMAIN = "player";

    @Override
    public String getDomain() {
        return DOMAIN;
    }

    @Override
    protected PlayerState createEmptyState() {
        return new PlayerState();
    }

    // --- Event appliers ---

    @Applies(PlayerRegistered.class)
    public void applyRegistered(PlayerState state, PlayerRegistered event) {
        state.setPlayerId("player_" + event.getEmail());
        state.setDisplayName(event.getDisplayName());
        state.setEmail(event.getEmail());
        state.setPlayerType(event.getPlayerTypeValue());
        state.setAiModelId(event.getAiModelId());
        state.setStatus("active");
        state.setBankroll(0);
        state.setReservedFunds(0);
    }

    @Applies(FundsDeposited.class)
    public void applyDeposited(PlayerState state, FundsDeposited event) {
        if (event.hasNewBalance()) {
            state.setBankroll(event.getNewBalance().getAmount());
        }
    }

    @Applies(FundsWithdrawn.class)
    public void applyWithdrawn(PlayerState state, FundsWithdrawn event) {
        if (event.hasNewBalance()) {
            state.setBankroll(event.getNewBalance().getAmount());
        }
    }

    @Applies(FundsReserved.class)
    public void applyReserved(PlayerState state, FundsReserved event) {
        if (event.hasNewReservedBalance()) {
            state.setReservedFunds(event.getNewReservedBalance().getAmount());
        }
        String tableKey = bytesToHex(event.getTableRoot().toByteArray());
        if (event.hasAmount()) {
            state.getTableReservations().put(tableKey, event.getAmount().getAmount());
        }
    }

    @Applies(FundsReleased.class)
    public void applyReleased(PlayerState state, FundsReleased event) {
        if (event.hasNewReservedBalance()) {
            state.setReservedFunds(event.getNewReservedBalance().getAmount());
        }
        String tableKey = bytesToHex(event.getTableRoot().toByteArray());
        state.getTableReservations().remove(tableKey);
    }

    // --- State accessors ---

    public boolean exists() {
        return getState().exists();
    }

    public String getPlayerId() {
        return getState().getPlayerId();
    }

    public String getDisplayName() {
        return getState().getDisplayName();
    }

    public String getEmail() {
        return getState().getEmail();
    }

    public int getPlayerType() {
        return getState().getPlayerType();
    }

    public String getAiModelId() {
        return getState().getAiModelId();
    }

    public long getBankroll() {
        return getState().getBankroll();
    }

    public long getReservedFunds() {
        return getState().getReservedFunds();
    }

    public long getAvailableBalance() {
        return getState().getAvailableBalance();
    }

    public boolean isAi() {
        return getState().isAi();
    }

    // --- Command handlers ---

    @Handles(RegisterPlayer.class)
    public PlayerRegistered register(RegisterPlayer cmd) {
        // Guard
        if (exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player already exists");
        }

        // Validate
        if (cmd.getDisplayName().isEmpty()) {
            throw Errors.CommandRejectedError.invalidArgument("display_name is required");
        }
        if (cmd.getEmail().isEmpty()) {
            throw Errors.CommandRejectedError.invalidArgument("email is required");
        }

        // Compute
        return PlayerRegistered.newBuilder()
            .setDisplayName(cmd.getDisplayName())
            .setEmail(cmd.getEmail())
            .setPlayerType(cmd.getPlayerType())
            .setAiModelId(cmd.getAiModelId())
            .setRegisteredAt(now())
            .build();
    }

    @Handles(DepositFunds.class)
    public FundsDeposited deposit(DepositFunds cmd) {
        // Guard
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }

        // Validate
        long amount = cmd.hasAmount() ? cmd.getAmount().getAmount() : 0;
        if (amount <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
        }

        // Compute
        long newBalance = getBankroll() + amount;
        return FundsDeposited.newBuilder()
            .setAmount(cmd.getAmount())
            .setNewBalance(Currency.newBuilder()
                .setAmount(newBalance)
                .setCurrencyCode("CHIPS"))
            .setDepositedAt(now())
            .build();
    }

    @Handles(WithdrawFunds.class)
    public FundsWithdrawn withdraw(WithdrawFunds cmd) {
        // Guard
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }

        // Validate
        long amount = cmd.hasAmount() ? cmd.getAmount().getAmount() : 0;
        if (amount <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
        }
        if (amount > getAvailableBalance()) {
            throw Errors.CommandRejectedError.preconditionFailed("Insufficient funds");
        }

        // Compute
        long newBalance = getBankroll() - amount;
        return FundsWithdrawn.newBuilder()
            .setAmount(cmd.getAmount())
            .setNewBalance(Currency.newBuilder()
                .setAmount(newBalance)
                .setCurrencyCode("CHIPS"))
            .setWithdrawnAt(now())
            .build();
    }

    @Handles(ReserveFunds.class)
    public FundsReserved reserve(ReserveFunds cmd) {
        // Guard
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }

        // Validate
        long amount = cmd.hasAmount() ? cmd.getAmount().getAmount() : 0;
        if (amount <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
        }

        String tableKey = bytesToHex(cmd.getTableRoot().toByteArray());
        if (getState().hasReservationFor(tableKey)) {
            throw Errors.CommandRejectedError.preconditionFailed("Funds already reserved for this table");
        }

        if (amount > getAvailableBalance()) {
            throw Errors.CommandRejectedError.preconditionFailed("Insufficient funds");
        }

        // Compute
        long newReserved = getReservedFunds() + amount;
        long newAvailable = getBankroll() - newReserved;
        return FundsReserved.newBuilder()
            .setAmount(cmd.getAmount())
            .setTableRoot(cmd.getTableRoot())
            .setNewAvailableBalance(Currency.newBuilder()
                .setAmount(newAvailable)
                .setCurrencyCode("CHIPS"))
            .setNewReservedBalance(Currency.newBuilder()
                .setAmount(newReserved)
                .setCurrencyCode("CHIPS"))
            .setReservedAt(now())
            .build();
    }

    @Handles(ReleaseFunds.class)
    public FundsReleased release(ReleaseFunds cmd) {
        // Guard
        if (!exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }

        // Validate
        String tableKey = bytesToHex(cmd.getTableRoot().toByteArray());
        long reservedForTable = getState().getReservationForTable(tableKey);
        if (reservedForTable == 0) {
            throw Errors.CommandRejectedError.preconditionFailed("No funds reserved for this table");
        }

        // Compute
        long newReserved = getReservedFunds() - reservedForTable;
        long newAvailable = getBankroll() - newReserved;
        return FundsReleased.newBuilder()
            .setAmount(Currency.newBuilder()
                .setAmount(reservedForTable)
                .setCurrencyCode("CHIPS"))
            .setTableRoot(cmd.getTableRoot())
            .setNewAvailableBalance(Currency.newBuilder()
                .setAmount(newAvailable)
                .setCurrencyCode("CHIPS"))
            .setNewReservedBalance(Currency.newBuilder()
                .setAmount(newReserved)
                .setCurrencyCode("CHIPS"))
            .setReleasedAt(now())
            .build();
    }

    // --- Utility methods ---

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
