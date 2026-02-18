using Google.Protobuf;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Player.Agg.Handlers;

namespace Player.Agg;

/// <summary>
/// Functional router for Player aggregate.
/// </summary>
public static class PlayerRouter
{
    public static CommandRouter Create()
    {
        return new CommandRouter("player", eb => PlayerState.FromEventBook(eb))
            .On<RegisterPlayer>((cmd, state) => RegisterHandler.Handle(cmd, (PlayerState)state))
            .On<DepositFunds>((cmd, state) => DepositHandler.Handle(cmd, (PlayerState)state))
            .On<WithdrawFunds>((cmd, state) => WithdrawHandler.Handle(cmd, (PlayerState)state))
            .On<ReserveFunds>((cmd, state) => ReserveHandler.Handle(cmd, (PlayerState)state))
            .On<ReleaseFunds>((cmd, state) => ReleaseHandler.Handle(cmd, (PlayerState)state))
            .On<TransferFunds>((cmd, state) => TransferHandler.Handle(cmd, (PlayerState)state));
    }
}
