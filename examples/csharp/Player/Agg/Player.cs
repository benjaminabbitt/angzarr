// DOC: This file is referenced in docs/docs/examples/aggregates.mdx
//      Update documentation when making changes to handler patterns.

using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg;

/// <summary>
/// Player aggregate - OO style with decorator-based command dispatch.
/// </summary>
public class PlayerAggregate : Aggregate<PlayerState>
{
    public const string Domain = "player";

    // --- Event appliers ---

    [Applies(typeof(PlayerRegistered))]
    public void ApplyRegistered(PlayerState state, PlayerRegistered evt)
    {
        state.PlayerId = $"player_{evt.Email}";
        state.DisplayName = evt.DisplayName;
        state.Email = evt.Email;
        state.PlayerType = evt.PlayerType;
        state.AiModelId = evt.AiModelId;
        state.Status = "active";
        state.Bankroll = 0;
        state.ReservedFunds = 0;
    }

    [Applies(typeof(FundsDeposited))]
    public void ApplyDeposited(PlayerState state, FundsDeposited evt)
    {
        if (evt.NewBalance != null)
        {
            state.Bankroll = evt.NewBalance.Amount;
        }
    }

    [Applies(typeof(FundsWithdrawn))]
    public void ApplyWithdrawn(PlayerState state, FundsWithdrawn evt)
    {
        if (evt.NewBalance != null)
        {
            state.Bankroll = evt.NewBalance.Amount;
        }
    }

    [Applies(typeof(FundsReserved))]
    public void ApplyReserved(PlayerState state, FundsReserved evt)
    {
        if (evt.NewReservedBalance != null)
        {
            state.ReservedFunds = evt.NewReservedBalance.Amount;
        }
        var tableKey = Convert.ToHexString(evt.TableRoot.ToByteArray()).ToLowerInvariant();
        if (evt.Amount != null)
        {
            state.TableReservations[tableKey] = evt.Amount.Amount;
        }
    }

    [Applies(typeof(FundsReleased))]
    public void ApplyReleased(PlayerState state, FundsReleased evt)
    {
        if (evt.NewReservedBalance != null)
        {
            state.ReservedFunds = evt.NewReservedBalance.Amount;
        }
        var tableKey = Convert.ToHexString(evt.TableRoot.ToByteArray()).ToLowerInvariant();
        state.TableReservations.Remove(tableKey);
    }

    [Applies(typeof(FundsTransferred))]
    public void ApplyTransferred(PlayerState state, FundsTransferred evt)
    {
        if (evt.NewBalance != null)
        {
            state.Bankroll = evt.NewBalance.Amount;
        }
    }

    // --- State accessors ---

    public bool Exists => State.Exists;
    public string PlayerId => State.PlayerId;
    public string DisplayName => State.DisplayName;
    public string Email => State.Email;
    public PlayerType PlayerType => State.PlayerType;
    public string AiModelId => State.AiModelId;
    public long Bankroll => State.Bankroll;
    public long ReservedFunds => State.ReservedFunds;
    public Dictionary<string, long> TableReservations => State.TableReservations;
    public string Status => State.Status;
    public long AvailableBalance => State.AvailableBalance;
    public bool IsAi => State.IsAi;

    // --- Command handlers ---

    // docs:start:annotation_handlers
    [Handles(typeof(RegisterPlayer))]
    public PlayerRegistered HandleRegister(RegisterPlayer cmd)
    {
        if (Exists)
            throw CommandRejectedError.PreconditionFailed("Player already exists");
        if (string.IsNullOrEmpty(cmd.DisplayName))
            throw CommandRejectedError.InvalidArgument("display_name is required");
        if (string.IsNullOrEmpty(cmd.Email))
            throw CommandRejectedError.InvalidArgument("email is required");

        return new PlayerRegistered
        {
            DisplayName = cmd.DisplayName,
            Email = cmd.Email,
            PlayerType = cmd.PlayerType,
            AiModelId = cmd.AiModelId,
            RegisteredAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(DepositFunds))]
    public FundsDeposited HandleDeposit(DepositFunds cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        var amount = cmd.Amount?.Amount ?? 0;
        if (amount <= 0)
            throw CommandRejectedError.InvalidArgument("amount must be positive");

        var newBalance = Bankroll + amount;
        return new FundsDeposited
        {
            Amount = cmd.Amount,
            NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
            DepositedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(WithdrawFunds))]
    public FundsWithdrawn HandleWithdraw(WithdrawFunds cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        var amount = cmd.Amount?.Amount ?? 0;
        if (amount <= 0)
            throw CommandRejectedError.InvalidArgument("amount must be positive");
        if (amount > AvailableBalance)
            throw CommandRejectedError.PreconditionFailed("Insufficient funds");

        var newBalance = Bankroll - amount;
        return new FundsWithdrawn
        {
            Amount = cmd.Amount,
            NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
            WithdrawnAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(ReserveFunds))]
    public FundsReserved HandleReserve(ReserveFunds cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        var amount = cmd.Amount?.Amount ?? 0;
        if (amount <= 0)
            throw CommandRejectedError.InvalidArgument("amount must be positive");

        var tableKey = Convert.ToHexString(cmd.TableRoot.ToByteArray()).ToLowerInvariant();
        if (TableReservations.ContainsKey(tableKey))
            throw CommandRejectedError.PreconditionFailed("Funds already reserved for this table");
        if (amount > AvailableBalance)
            throw CommandRejectedError.PreconditionFailed("Insufficient funds");

        var newReserved = ReservedFunds + amount;
        var newAvailable = Bankroll - newReserved;
        return new FundsReserved
        {
            Amount = cmd.Amount,
            TableRoot = cmd.TableRoot,
            NewAvailableBalance = new Currency { Amount = newAvailable, CurrencyCode = "CHIPS" },
            NewReservedBalance = new Currency { Amount = newReserved, CurrencyCode = "CHIPS" },
            ReservedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(ReleaseFunds))]
    public FundsReleased HandleRelease(ReleaseFunds cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        var tableKey = Convert.ToHexString(cmd.TableRoot.ToByteArray()).ToLowerInvariant();
        if (!TableReservations.TryGetValue(tableKey, out var reservedForTable) || reservedForTable == 0)
            throw CommandRejectedError.PreconditionFailed("No funds reserved for this table");

        var newReserved = ReservedFunds - reservedForTable;
        var newAvailable = Bankroll - newReserved;
        return new FundsReleased
        {
            Amount = new Currency { Amount = reservedForTable, CurrencyCode = "CHIPS" },
            TableRoot = cmd.TableRoot,
            NewAvailableBalance = new Currency { Amount = newAvailable, CurrencyCode = "CHIPS" },
            NewReservedBalance = new Currency { Amount = newReserved, CurrencyCode = "CHIPS" },
            ReleasedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(TransferFunds))]
    public FundsTransferred HandleTransfer(TransferFunds cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Player does not exist");

        var amount = cmd.Amount?.Amount ?? 0;
        var newBalance = Bankroll + amount;
        return new FundsTransferred
        {
            FromPlayerRoot = cmd.FromPlayerRoot,
            ToPlayerRoot = Google.Protobuf.ByteString.CopyFromUtf8(PlayerId),
            Amount = cmd.Amount,
            HandRoot = cmd.HandRoot,
            Reason = cmd.Reason,
            NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
            TransferredAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
    // docs:end:annotation_handlers

    // --- Rejection handler ---

    [Rejected("table", "JoinTable")]
    public IMessage? HandleTableJoinRejected(Notification notification)
    {
        // Default: delegate to framework
        return null;
    }
}
