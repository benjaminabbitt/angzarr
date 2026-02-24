using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf.WellKnownTypes;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for RegisterPlayer command.
///
/// Follows the guard/validate/compute pattern:
/// - Guard: Check state preconditions (aggregate exists, correct phase)
/// - Validate: Validate command inputs
/// - Compute: Build the resulting event
///
/// Why this pattern? Each step is a pure function (state in, result out),
/// enabling direct unit testing without mocking infrastructure. You can test
/// each step independently by passing state objects and asserting on results.
/// </summary>
public static class RegisterHandler
{
    public static PlayerRegistered Handle(RegisterPlayer cmd, PlayerState state)
    {
        // Guard
        if (state.Exists)
            throw CommandRejectedError.PreconditionFailed("Player already exists");

        // Validate
        if (string.IsNullOrEmpty(cmd.DisplayName))
            throw CommandRejectedError.InvalidArgument("display_name is required");
        if (string.IsNullOrEmpty(cmd.Email))
            throw CommandRejectedError.InvalidArgument("email is required");

        // Compute
        return new PlayerRegistered
        {
            DisplayName = cmd.DisplayName,
            Email = cmd.Email,
            PlayerType = cmd.PlayerType,
            AiModelId = cmd.AiModelId,
            RegisteredAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };
    }
}
