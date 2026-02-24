package dev.angzarr.examples.player.handlers;

import com.google.protobuf.Timestamp;
import dev.angzarr.client.Errors;
import dev.angzarr.examples.PlayerRegistered;
import dev.angzarr.examples.RegisterPlayer;
import dev.angzarr.examples.player.state.PlayerState;
import java.time.Instant;

/**
 * Functional handler for RegisterPlayer command.
 *
 * <p>Pure function following guard/validate/compute pattern:
 *
 * <ul>
 *   <li><b>guard</b>: Check state preconditions (aggregate exists, correct phase)
 *   <li><b>validate</b>: Validate command inputs
 *   <li><b>compute</b>: Build the resulting event
 * </ul>
 *
 * <h3>Why this pattern?</h3>
 *
 * <p>Each step is a pure function (state in, result out), enabling direct unit testing without
 * mocking infrastructure. You can test guard, validate, and compute logic independently by passing
 * state objects and asserting on results.
 */
public final class RegisterHandler {

  private RegisterHandler() {}

  /**
   * Handle RegisterPlayer command.
   *
   * @param cmd The command
   * @param state Current aggregate state
   * @return The resulting event
   * @throws Errors.CommandRejectedError if command is rejected
   */
  public static PlayerRegistered handle(RegisterPlayer cmd, PlayerState state) {
    // Guard
    if (state.exists()) {
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

  private static Timestamp now() {
    Instant instant = Instant.now();
    return Timestamp.newBuilder()
        .setSeconds(instant.getEpochSecond())
        .setNanos(instant.getNano())
        .build();
  }
}
