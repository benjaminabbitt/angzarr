using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.Agg.Handlers;

/// <summary>
/// Handler for LeaveTable command.
/// </summary>
public static class LeaveHandler
{
    public static PlayerLeft Handle(LeaveTable cmd, TableState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");

        // Validate
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");

        var seat = state.FindPlayerSeat(cmd.PlayerRoot);
        if (seat == null)
            throw CommandRejectedError.PreconditionFailed("Player is not seated at table");
        if (state.Status == "in_hand")
            throw CommandRejectedError.PreconditionFailed("Cannot leave table during a hand");

        // Compute
        return new PlayerLeft
        {
            PlayerRoot = cmd.PlayerRoot,
            SeatPosition = seat.Position,
            ChipsCashedOut = seat.Stack,
            LeftAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
