package dev.angzarr.examples.player.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.player.state.PlayerState;
import dev.angzarr.examples.Currency;
import dev.angzarr.examples.FundsReleased;
import dev.angzarr.examples.ReleaseFunds;

import java.time.Instant;

/**
 * Functional handler for ReleaseFunds command.
 */
public final class ReleaseHandler {

    private ReleaseHandler() {}

    public static FundsReleased handle(ReleaseFunds cmd, PlayerState state) {
        // Guard
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }

        // Validate
        String tableKey = bytesToHex(cmd.getTableRoot().toByteArray());
        long reservedForTable = state.getReservationForTable(tableKey);
        if (reservedForTable == 0) {
            throw Errors.CommandRejectedError.preconditionFailed("No funds reserved for this table");
        }

        // Compute
        long newReserved = state.getReservedFunds() - reservedForTable;
        long newAvailable = state.getBankroll() - newReserved;
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

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }

    private static Timestamp now() {
        Instant instant = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(instant.getEpochSecond())
            .setNanos(instant.getNano())
            .build();
    }
}
