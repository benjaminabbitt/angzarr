using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for rejection compensation.
///
/// Handles JoinTable rejection by releasing reserved funds.
/// </summary>
public static class RejectedHandler
{
    /// <summary>
    /// Handle JoinTable rejection by releasing reserved funds.
    /// </summary>
    public static FundsReleased HandleJoinRejected(Notification notification, PlayerState state)
    {
        var ctx = CompensationContext.From(notification);

        Console.WriteLine(
            $"Player compensation for JoinTable rejection: reason={ctx.RejectionReason}"
        );

        // Extract table_root from the rejected command
        var tableRoot = ctx.RejectedCommand?.Cover?.Root?.Value ?? ByteString.Empty;

        // Release the funds that were reserved for this table
        var tableKey = Convert.ToHexString(tableRoot.ToByteArray()).ToLowerInvariant();
        state.TableReservations.TryGetValue(tableKey, out var reservedAmount);
        var newReserved = state.ReservedFunds - reservedAmount;
        var newAvailable = state.Bankroll - newReserved;

        return new FundsReleased
        {
            Amount = new Currency { Amount = reservedAmount, CurrencyCode = "CHIPS" },
            TableRoot = tableRoot,
            NewAvailableBalance = new Currency { Amount = newAvailable, CurrencyCode = "CHIPS" },
            NewReservedBalance = new Currency { Amount = newReserved, CurrencyCode = "CHIPS" },
            ReleasedAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };
    }
}
