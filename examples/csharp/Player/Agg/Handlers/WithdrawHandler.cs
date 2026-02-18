using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for WithdrawFunds command.
/// </summary>
public static class WithdrawHandler
{
    public static FundsWithdrawn Handle(WithdrawFunds cmd, PlayerState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        // Validate
        var amount = cmd.Amount?.Amount ?? 0;
        if (amount <= 0)
            throw CommandRejectedError.InvalidArgument("amount must be positive");
        if (amount > state.AvailableBalance)
            throw CommandRejectedError.PreconditionFailed("Insufficient funds");

        // Compute
        var newBalance = state.Bankroll - amount;
        return new FundsWithdrawn
        {
            Amount = cmd.Amount,
            NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
            WithdrawnAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
