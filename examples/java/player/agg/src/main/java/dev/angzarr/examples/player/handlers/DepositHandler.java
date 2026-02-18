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

    // docs:start:deposit_guard
    static void guard(PlayerState state) {
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
        }
    }
    // docs:end:deposit_guard

    // docs:start:deposit_validate
    static long validate(DepositFunds cmd) {
        long amount = cmd.hasAmount() ? cmd.getAmount().getAmount() : 0;
        if (amount <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
        }
        return amount;
    }
    // docs:end:deposit_validate

    // docs:start:deposit_compute
    static FundsDeposited compute(DepositFunds cmd, PlayerState state, long amount) {
        long newBalance = state.getBankroll() + amount;
        return FundsDeposited.newBuilder()
            .setAmount(cmd.getAmount())
            .setNewBalance(Currency.newBuilder()
                .setAmount(newBalance)
                .setCurrencyCode("CHIPS"))
            .setDepositedAt(now())
            .build();
    }
    // docs:end:deposit_compute

    public static FundsDeposited handle(DepositFunds cmd, PlayerState state) {
        guard(state);
        long amount = validate(cmd);
        return compute(cmd, state, amount);
    }

    private static Timestamp now() {
        Instant instant = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(instant.getEpochSecond())
            .setNanos(instant.getNano())
            .build();
    }
}
