using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for RegisterPlayer command.
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
            RegisteredAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
