using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for ReserveFunds command.
/// </summary>
public static class ReserveHandler
{
    public static FundsReserved Handle(ReserveFunds cmd, PlayerState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        // Validate
        var amount = cmd.Amount?.Amount ?? 0;
        if (amount <= 0)
            throw CommandRejectedError.InvalidArgument("amount must be positive");

        var tableKey = Convert.ToHexString(cmd.TableRoot.ToByteArray()).ToLowerInvariant();
        if (state.TableReservations.ContainsKey(tableKey))
            throw CommandRejectedError.PreconditionFailed("Funds already reserved for this table");
        if (amount > state.AvailableBalance)
            throw CommandRejectedError.PreconditionFailed("Insufficient funds");

        // Compute
        var newReserved = state.ReservedFunds + amount;
        var newAvailable = state.Bankroll - newReserved;
        return new FundsReserved
        {
            Amount = cmd.Amount,
            TableRoot = cmd.TableRoot,
            NewAvailableBalance = new Currency { Amount = newAvailable, CurrencyCode = "CHIPS" },
            NewReservedBalance = new Currency { Amount = newReserved, CurrencyCode = "CHIPS" },
            ReservedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
