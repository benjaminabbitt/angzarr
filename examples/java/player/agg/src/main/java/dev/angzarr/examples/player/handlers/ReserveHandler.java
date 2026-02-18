// DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//      Update documentation when making changes to handler patterns.
package dev.angzarr.examples.player.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.player.state.PlayerState;
import dev.angzarr.examples.Currency;
import dev.angzarr.examples.FundsReserved;
import dev.angzarr.examples.ReserveFunds;

import java.time.Instant;

// docs:start:reserve_funds_imp
/**
 * Functional handler for ReserveFunds command.
 */
public final class ReserveHandler {

    private ReserveHandler() {}

    public static FundsReserved handle(ReserveFunds cmd, PlayerState state) {
        // Guard
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }

        // Validate
        long amount = cmd.hasAmount() ? cmd.getAmount().getAmount() : 0;
        if (amount <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
        }

        String tableKey = bytesToHex(cmd.getTableRoot().toByteArray());
        if (state.hasReservationFor(tableKey)) {
            throw Errors.CommandRejectedError.preconditionFailed("Funds already reserved for this table");
        }

        if (amount > state.getAvailableBalance()) {
            throw Errors.CommandRejectedError.preconditionFailed("Insufficient funds");
        }

        // Compute
        long newReserved = state.getReservedFunds() + amount;
        long newAvailable = state.getBankroll() - newReserved;
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
// docs:end:reserve_funds_imp
