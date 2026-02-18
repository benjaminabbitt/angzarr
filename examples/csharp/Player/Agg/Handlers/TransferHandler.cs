using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg.Handlers;

/// <summary>
/// Handler for TransferFunds command.
/// </summary>
public static class TransferHandler
{
    public static FundsTransferred Handle(TransferFunds cmd, PlayerState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        // Compute
        var amount = cmd.Amount?.Amount ?? 0;
        var newBalance = state.Bankroll + amount;
        return new FundsTransferred
        {
            FromPlayerRoot = cmd.FromPlayerRoot,
            ToPlayerRoot = Google.Protobuf.ByteString.CopyFromUtf8(state.PlayerId),
            Amount = cmd.Amount,
            HandRoot = cmd.HandRoot,
            Reason = cmd.Reason,
            NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
            TransferredAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
