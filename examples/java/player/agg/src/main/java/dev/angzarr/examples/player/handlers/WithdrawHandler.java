package dev.angzarr.examples.player.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.player.state.PlayerState;
import dev.angzarr.examples.Currency;
import dev.angzarr.examples.FundsWithdrawn;
import dev.angzarr.examples.WithdrawFunds;

import java.time.Instant;

/**
 * Functional handler for WithdrawFunds command.
 */
public final class WithdrawHandler {

    private WithdrawHandler() {}

    public static FundsWithdrawn handle(WithdrawFunds cmd, PlayerState state) {
        // Guard
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }

        // Validate
        long amount = cmd.hasAmount() ? cmd.getAmount().getAmount() : 0;
        if (amount <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
        }
        if (amount > state.getAvailableBalance()) {
            throw Errors.CommandRejectedError.preconditionFailed("Insufficient funds");
        }

        // Compute
        long newBalance = state.getBankroll() - amount;
        return FundsWithdrawn.newBuilder()
            .setAmount(cmd.getAmount())
            .setNewBalance(Currency.newBuilder()
                .setAmount(newBalance)
                .setCurrencyCode("CHIPS"))
            .setWithdrawnAt(now())
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
