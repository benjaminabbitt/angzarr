package dev.angzarr.examples.hand.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.hand.state.HandState;
import dev.angzarr.examples.hand.state.PlayerHandState;
import dev.angzarr.examples.*;

import java.time.Instant;

/**
 * Functional handler for PostBlind command.
 *
 * <p>Pure function following guard/validate/compute pattern.
 */
public final class PostBlindHandler {

    private PostBlindHandler() {}

    /**
     * Handle PostBlind command.
     *
     * @param cmd The command
     * @param state Current aggregate state
     * @return The resulting event
     * @throws Errors.CommandRejectedError if command is rejected
     */
    public static BlindPosted handle(PostBlind cmd, HandState state) {
        // Guard
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
        }

        // Validate
        PlayerHandState player = state.getPlayer(cmd.getPlayerRoot().toByteArray());
        if (player == null) {
            throw Errors.CommandRejectedError.preconditionFailed("Player not in hand");
        }
        if (player.hasFolded()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player has folded");
        }
        if (cmd.getAmount() <= 0) {
            throw Errors.CommandRejectedError.invalidArgument("Blind amount must be positive");
        }

        // Compute
        long amount = Math.min(cmd.getAmount(), player.getStack());
        long newStack = player.getStack() - amount;
        long newPot = state.getPotTotal() + amount;

        return BlindPosted.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setBlindType(cmd.getBlindType())
            .setAmount(amount)
            .setPlayerStack(newStack)
            .setPotTotal(newPot)
            .setPostedAt(now())
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
