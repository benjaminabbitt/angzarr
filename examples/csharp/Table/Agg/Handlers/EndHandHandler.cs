using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.Agg.Handlers;

/// <summary>
/// Handler for EndHand command.
/// </summary>
public static class EndHandHandler
{
    public static HandEnded Handle(EndHand cmd, TableState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");
        if (state.Status != "in_hand")
            throw CommandRejectedError.PreconditionFailed("No hand in progress");
        if (!cmd.HandRoot.Equals(state.CurrentHandRoot))
            throw CommandRejectedError.PreconditionFailed("Hand root mismatch");

        // Compute
        var stackChanges = new Dictionary<string, long>();
        foreach (var result in cmd.Results)
        {
            var playerHex = Convert.ToHexString(result.WinnerRoot.ToByteArray()).ToLowerInvariant();
            if (!stackChanges.ContainsKey(playerHex))
                stackChanges[playerHex] = 0;
            stackChanges[playerHex] += result.Amount;
        }

        var evt = new HandEnded
        {
            HandRoot = cmd.HandRoot,
            EndedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.Results.AddRange(cmd.Results);
        foreach (var kvp in stackChanges)
        {
            evt.StackChanges[kvp.Key] = kvp.Value;
        }

        return evt;
    }
}
