package dev.angzarr.examples.hand.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.hand.state.HandState;
import dev.angzarr.examples.hand.state.PlayerHandState;
import dev.angzarr.examples.*;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;

/**
 * Functional handler for AwardPot command.
 *
 * <p>Pure function following guard/validate/compute pattern.
 */
public final class AwardPotHandler {

    private AwardPotHandler() {}

    /**
     * Handle AwardPot command.
     *
     * @param cmd The command
     * @param state Current aggregate state
     * @return The resulting event
     * @throws Errors.CommandRejectedError if command is rejected
     */
    public static PotAwarded handle(AwardPot cmd, HandState state) {
        // Guard
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
        }
        if (state.isComplete()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand already complete");
        }

        // Validate
        if (cmd.getAwardsList().isEmpty()) {
            throw Errors.CommandRejectedError.invalidArgument("No awards specified");
        }

        for (PotAward award : cmd.getAwardsList()) {
            PlayerHandState player = state.getPlayer(award.getPlayerRoot().toByteArray());
            if (player == null) {
                throw Errors.CommandRejectedError.invalidArgument("Winner not in hand");
            }
            if (player.hasFolded()) {
                throw Errors.CommandRejectedError.invalidArgument("Folded player cannot win pot");
            }
        }

        // Compute
        List<PotWinner> winners = new ArrayList<>();
        for (PotAward award : cmd.getAwardsList()) {
            winners.add(PotWinner.newBuilder()
                .setPlayerRoot(award.getPlayerRoot())
                .setAmount(award.getAmount())
                .setPotType(award.getPotType())
                .build());
        }

        return PotAwarded.newBuilder()
            .addAllWinners(winners)
            .setAwardedAt(now())
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
