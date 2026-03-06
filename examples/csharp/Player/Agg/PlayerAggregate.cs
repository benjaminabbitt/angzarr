using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf.WellKnownTypes;
using Player.Agg.Handlers;

namespace Player.Agg;

/// <summary>
/// Player aggregate - OO style with decorator-based command dispatch.
/// </summary>
public class PlayerAggregate : CommandHandler<PlayerState>
{
    public const string DomainName = "player";

    public override string Domain => DomainName;

    protected override PlayerState CreateEmptyState() => new PlayerState();

    protected override void ApplyEvent(PlayerState state, Any eventAny)
    {
        PlayerState.Router.ApplySingle(state, eventAny);
    }

    // --- State accessors ---

    public new bool Exists => State.Exists;
    public string PlayerId => State.PlayerId;
    public string DisplayName => State.DisplayName;
    public string Email => State.Email;
    public PlayerType PlayerType => State.PlayerType;
    public long Bankroll => State.Bankroll;
    public long ReservedFunds => State.ReservedFunds;
    public long AvailableBalance => State.AvailableBalance;
    public string Status => State.Status;

    // --- Command handlers ---

    [Handles(typeof(RegisterPlayer))]
    public PlayerRegistered HandleRegister(RegisterPlayer cmd)
    {
        return RegisterHandler.Handle(cmd, State);
    }

    [Handles(typeof(DepositFunds))]
    public FundsDeposited HandleDeposit(DepositFunds cmd)
    {
        return DepositHandler.Handle(cmd, State);
    }

    [Handles(typeof(WithdrawFunds))]
    public FundsWithdrawn HandleWithdraw(WithdrawFunds cmd)
    {
        return WithdrawHandler.Handle(cmd, State);
    }

    [Handles(typeof(ReserveFunds))]
    public FundsReserved HandleReserve(ReserveFunds cmd)
    {
        return ReserveHandler.Handle(cmd, State);
    }

    [Handles(typeof(ReleaseFunds))]
    public FundsReleased HandleRelease(ReleaseFunds cmd)
    {
        return ReleaseHandler.Handle(cmd, State);
    }

    [Handles(typeof(TransferFunds))]
    public FundsTransferred HandleTransfer(TransferFunds cmd)
    {
        return TransferHandler.Handle(cmd, State);
    }
}
