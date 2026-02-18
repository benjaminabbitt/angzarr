package dev.angzarr.examples.hand.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.hand.state.HandState;
import dev.angzarr.examples.hand.state.PlayerHandState;
import dev.angzarr.examples.*;

import java.time.Instant;

/**
 * Functional handler for PlayerAction command.
 *
 * <p>Pure function following guard/validate/compute pattern.
 */
public final class PlayerActionHandler {

    private PlayerActionHandler() {}

    /**
     * Handle PlayerAction command.
     *
     * @param cmd The command
     * @param state Current aggregate state
     * @return The resulting event
     * @throws Errors.CommandRejectedError if command is rejected
     */
    public static ActionTaken handle(PlayerAction cmd, HandState state) {
        // Guard
        if (!state.exists()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand does not exist");
        }
        if (state.isComplete()) {
            throw Errors.CommandRejectedError.preconditionFailed("Hand is complete");
        }

        // Validate
        PlayerHandState player = state.getPlayer(cmd.getPlayerRoot().toByteArray());
        if (player == null) {
            throw Errors.CommandRejectedError.preconditionFailed("Player not in hand");
        }
        if (player.hasFolded()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player has folded");
        }
        if (player.isAllIn()) {
            throw Errors.CommandRejectedError.preconditionFailed("Player is all-in");
        }

        // Compute action amount
        long amount = 0;
        ActionType action = cmd.getAction();
        long callAmount = state.getCurrentBet() - player.getBetThisRound();

        switch (action) {
            case FOLD:
                break;
            case CHECK:
                if (callAmount > 0) {
                    throw Errors.CommandRejectedError.invalidArgument("Cannot check, must call or fold");
                }
                break;
            case CALL:
                if (callAmount == 0) {
                    throw Errors.CommandRejectedError.invalidArgument("Nothing to call");
                }
                amount = Math.min(callAmount, player.getStack());
                if (player.getStack() - amount == 0) {
                    action = ActionType.ALL_IN;
                }
                break;
            case BET:
                if (state.getCurrentBet() > 0) {
                    throw Errors.CommandRejectedError.invalidArgument("Cannot bet when there is already a bet");
                }
                amount = cmd.getAmount();
                if (amount > player.getStack()) {
                    throw Errors.CommandRejectedError.invalidArgument("Bet exceeds stack");
                }
                if (player.getStack() - amount == 0) {
                    action = ActionType.ALL_IN;
                }
                break;
            case RAISE:
                if (state.getCurrentBet() == 0) {
                    throw Errors.CommandRejectedError.invalidArgument("Cannot raise when there is no bet");
                }
                amount = cmd.getAmount();
                if (amount > player.getStack()) {
                    throw Errors.CommandRejectedError.invalidArgument("Raise exceeds stack");
                }
                if (player.getStack() - amount == 0) {
                    action = ActionType.ALL_IN;
                }
                break;
            case ALL_IN:
                amount = player.getStack();
                break;
            default:
                throw Errors.CommandRejectedError.invalidArgument("Invalid action");
        }

        long newStack = player.getStack() - amount;
        long newPot = state.getPotTotal() + amount;
        long amountToCall = Math.max(state.getCurrentBet(), player.getBetThisRound() + amount);

        return ActionTaken.newBuilder()
            .setPlayerRoot(cmd.getPlayerRoot())
            .setAction(action)
            .setAmount(amount)
            .setPlayerStack(newStack)
            .setPotTotal(newPot)
            .setAmountToCall(amountToCall)
            .setActionAt(now())
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
