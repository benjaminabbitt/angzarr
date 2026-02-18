using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.Agg.Handlers;

/// <summary>
/// Handler for JoinTable command.
/// </summary>
public static class JoinHandler
{
    public static PlayerJoined Handle(JoinTable cmd, TableState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");

        // Validate
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");
        if (state.FindPlayerSeat(cmd.PlayerRoot) != null)
            throw CommandRejectedError.PreconditionFailed("Player already seated at table");
        if (state.IsFull)
            throw CommandRejectedError.PreconditionFailed("Table is full");
        if (cmd.BuyInAmount < state.MinBuyIn)
            throw CommandRejectedError.InvalidArgument($"Buy-in must be at least {state.MinBuyIn}");
        if (cmd.BuyInAmount > state.MaxBuyIn)
            throw CommandRejectedError.InvalidArgument($"Buy-in cannot exceed {state.MaxBuyIn}");
        if (cmd.PreferredSeat > 0 && state.GetSeat(cmd.PreferredSeat) != null)
            throw CommandRejectedError.PreconditionFailed("Seat is occupied");

        // Compute
        var seatPosition = state.FindAvailableSeat(cmd.PreferredSeat) ?? 0;

        return new PlayerJoined
        {
            PlayerRoot = cmd.PlayerRoot,
            SeatPosition = seatPosition,
            BuyInAmount = cmd.BuyInAmount,
            Stack = cmd.BuyInAmount,
            JoinedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
