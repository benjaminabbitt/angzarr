using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for DepositFunds command.
/// </summary>
public static class DepositHandler
{
    // docs:start:deposit_guard
    internal static void Guard(PlayerState state)
    {
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");
    }
    // docs:end:deposit_guard

    // docs:start:deposit_validate
    internal static long Validate(DepositFunds cmd)
    {
        var amount = cmd.Amount?.Amount ?? 0;
        if (amount <= 0)
            throw CommandRejectedError.InvalidArgument("amount must be positive");
        return amount;
    }
    // docs:end:deposit_validate

    // docs:start:deposit_compute
    internal static FundsDeposited Compute(DepositFunds cmd, PlayerState state, long amount)
    {
        var newBalance = state.Bankroll + amount;
        return new FundsDeposited
        {
            Amount = cmd.Amount,
            NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
            DepositedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
    // docs:end:deposit_compute

    public static FundsDeposited Handle(DepositFunds cmd, PlayerState state)
    {
        Guard(state);
        var amount = Validate(cmd);
        return Compute(cmd, state, amount);
    }
}
