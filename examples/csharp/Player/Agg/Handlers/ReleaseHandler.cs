using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for ReleaseFunds command.
/// </summary>
public static class ReleaseHandler
{
    public static FundsReleased Handle(ReleaseFunds cmd, PlayerState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        // Validate
        var tableKey = Convert.ToHexString(cmd.TableRoot.ToByteArray()).ToLowerInvariant();
        if (!state.TableReservations.TryGetValue(tableKey, out var reservedForTable) || reservedForTable == 0)
            throw CommandRejectedError.PreconditionFailed("No funds reserved for this table");

        // Compute
        var newReserved = state.ReservedFunds - reservedForTable;
        var newAvailable = state.Bankroll - newReserved;
        return new FundsReleased
        {
            Amount = new Currency { Amount = reservedForTable, CurrencyCode = "CHIPS" },
            TableRoot = cmd.TableRoot,
            NewAvailableBalance = new Currency { Amount = newAvailable, CurrencyCode = "CHIPS" },
            NewReservedBalance = new Currency { Amount = newReserved, CurrencyCode = "CHIPS" },
            ReleasedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
