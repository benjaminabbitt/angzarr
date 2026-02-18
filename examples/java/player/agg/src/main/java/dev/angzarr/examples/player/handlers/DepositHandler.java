package dev.angzarr.examples.player.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.player.state.PlayerState;
import dev.angzarr.examples.Currency;
import dev.angzarr.examples.DepositFunds;
import dev.angzarr.examples.FundsDeposited;

import java.time.Instant;

/**
 * Functional handler for DepositFunds command.
 */
public final class DepositHandler {

    private DepositHandler() {}

    public static FundsDeposited handle(DepositFunds cmd, PlayerState state) {
        // Guard
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }

        // Validate
        long amount = cmd.hasAmount() ? cmd.getAmount().getAmount() : 0;
        if (amount <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
        }

        // Compute
        long newBalance = state.getBankroll() + amount;
        return FundsDeposited.newBuilder()
            .setAmount(cmd.getAmount())
            .setNewBalance(Currency.newBuilder()
                .setAmount(newBalance)
                .setCurrencyCode("CHIPS"))
            .setDepositedAt(now())
            .build();
    }

    private static Timestamp now() {
        Instant instant = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(instant.getEpochSecond())
            .setNanos(instant.getNano())
            .build();
    }
}
