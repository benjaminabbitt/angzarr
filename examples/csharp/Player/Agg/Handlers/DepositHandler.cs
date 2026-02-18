using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for DepositFunds command.
/// </summary>
public static class DepositHandler
{
    public static FundsDeposited Handle(DepositFunds cmd, PlayerState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        // Validate
        var amount = cmd.Amount?.Amount ?? 0;
        if (amount <= 0)
            throw CommandRejectedError.InvalidArgument("amount must be positive");

        // Compute
        var newBalance = state.Bankroll + amount;
        return new FundsDeposited
        {
            Amount = cmd.Amount,
            NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
            DepositedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
