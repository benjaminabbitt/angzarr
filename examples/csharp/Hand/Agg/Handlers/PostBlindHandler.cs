using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.Agg.Handlers;

/// <summary>
/// Handler for PostBlind command.
/// </summary>
public static class PostBlindHandler
{
    public static BlindPosted Handle(PostBlind cmd, HandState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (state.Status == "complete")
            throw CommandRejectedError.PreconditionFailed("Hand is complete");

        // Validate
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");

        var player = state.GetPlayer(cmd.PlayerRoot);
        if (player == null)
            throw CommandRejectedError.PreconditionFailed("Player not in hand");
        if (player.HasFolded)
            throw CommandRejectedError.PreconditionFailed("Player has folded");
        if (cmd.Amount <= 0)
            throw CommandRejectedError.InvalidArgument("Blind amount must be positive");

        // Compute
        var actualAmount = Math.Min(cmd.Amount, player.Stack);
        var newStack = player.Stack - actualAmount;
        var newPotTotal = state.GetPotTotal() + actualAmount;

        return new BlindPosted
        {
            PlayerRoot = cmd.PlayerRoot,
            BlindType = cmd.BlindType,
            Amount = actualAmount,
            PlayerStack = newStack,
            PotTotal = newPotTotal,
            PostedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
