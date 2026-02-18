using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Player.Agg;

/// <summary>
/// Player aggregate state.
/// </summary>
public class PlayerState
{
    public string PlayerId { get; set; } = "";
    public string DisplayName { get; set; } = "";
    public string Email { get; set; } = "";
    public PlayerType PlayerType { get; set; } = PlayerType.Unspecified;
    public string AiModelId { get; set; } = "";
    public long Bankroll { get; set; }
    public long ReservedFunds { get; set; }
    public Dictionary<string, long> TableReservations { get; } = new();
    public string Status { get; set; } = "";

    public bool Exists => !string.IsNullOrEmpty(PlayerId);
    public long AvailableBalance => Bankroll - ReservedFunds;
    public bool IsAi => PlayerType == PlayerType.Ai;

    /// <summary>
    /// StateRouter for fluent state reconstruction.
    /// </summary>
    public static readonly StateRouter<PlayerState> Router = new StateRouter<PlayerState>()
        .On<PlayerRegistered>((state, evt) =>
        {
            state.PlayerId = $"player_{evt.Email}";
            state.DisplayName = evt.DisplayName;
            state.Email = evt.Email;
            state.PlayerType = evt.PlayerType;
            state.AiModelId = evt.AiModelId;
            state.Status = "active";
            state.Bankroll = 0;
            state.ReservedFunds = 0;
        })
        .On<FundsDeposited>((state, evt) =>
        {
            if (evt.NewBalance != null)
            {
                state.Bankroll = evt.NewBalance.Amount;
            }
        })
        .On<FundsWithdrawn>((state, evt) =>
        {
            if (evt.NewBalance != null)
            {
                state.Bankroll = evt.NewBalance.Amount;
            }
        })
        .On<FundsReserved>((state, evt) =>
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
        })
        .On<FundsReleased>((state, evt) =>
        {
            if (evt.NewReservedBalance != null)
            {
                state.ReservedFunds = evt.NewReservedBalance.Amount;
            }
            var tableKey = Convert.ToHexString(evt.TableRoot.ToByteArray()).ToLowerInvariant();
            state.TableReservations.Remove(tableKey);
        })
        .On<FundsTransferred>((state, evt) =>
        {
            if (evt.NewBalance != null)
            {
                state.Bankroll = evt.NewBalance.Amount;
            }
        });

    /// <summary>
    /// Build state from an EventBook by applying all events.
    /// </summary>
    public static PlayerState FromEventBook(EventBook eventBook)
    {
        return Router.WithEventBook(eventBook);
    }
}
