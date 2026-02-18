using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.Agg;

/// <summary>
/// State of a single seat at the table.
/// </summary>
public class SeatState
{
    public int Position { get; set; }
    public ByteString PlayerRoot { get; set; } = ByteString.Empty;
    public long Stack { get; set; }
    public bool IsActive { get; set; } = true;
    public bool IsSittingOut { get; set; }
}

/// <summary>
/// Table aggregate state.
/// </summary>
public class TableState
{
    public string TableId { get; set; } = "";
    public string TableName { get; set; } = "";
    public GameVariant GameVariant { get; set; } = GameVariant.Unspecified;
    public long SmallBlind { get; set; }
    public long BigBlind { get; set; }
    public long MinBuyIn { get; set; }
    public long MaxBuyIn { get; set; }
    public int MaxPlayers { get; set; } = 9;
    public int ActionTimeoutSeconds { get; set; } = 30;
    public Dictionary<int, SeatState> Seats { get; } = new();
    public int DealerPosition { get; set; }
    public long HandCount { get; set; }
    public ByteString CurrentHandRoot { get; set; } = ByteString.Empty;
    public string Status { get; set; } = "";

    public bool Exists => !string.IsNullOrEmpty(TableId);
    public int PlayerCount => Seats.Count;
    public int ActivePlayerCount => Seats.Values.Count(s => !s.IsSittingOut);
    public bool IsFull => Seats.Count >= MaxPlayers;

    public SeatState? GetSeat(int position) => Seats.GetValueOrDefault(position);

    public SeatState? FindPlayerSeat(ByteString playerRoot)
    {
        return Seats.Values.FirstOrDefault(s => s.PlayerRoot.Equals(playerRoot));
    }

    public int? FindAvailableSeat(int preferred = -1)
    {
        if (preferred > 0 && preferred < MaxPlayers && !Seats.ContainsKey(preferred))
            return preferred;
        for (var pos = 0; pos < MaxPlayers; pos++)
        {
            if (!Seats.ContainsKey(pos))
                return pos;
        }
        return null;
    }

    public int NextDealerPosition()
    {
        if (Seats.Count == 0) return 0;
        var positions = Seats.Keys.OrderBy(p => p).ToList();
        var currentIdx = 0;
        for (var i = 0; i < positions.Count; i++)
        {
            if (positions[i] == DealerPosition)
            {
                currentIdx = i;
                break;
            }
        }
        var nextIdx = (currentIdx + 1) % positions.Count;
        return positions[nextIdx];
    }

    /// <summary>
    /// StateRouter for fluent state reconstruction.
    /// </summary>
    public static readonly StateRouter<TableState> Router = new StateRouter<TableState>()
        .On<TableCreated>((state, evt) =>
        {
            state.TableId = $"table_{evt.TableName}";
            state.TableName = evt.TableName;
            state.GameVariant = evt.GameVariant;
            state.SmallBlind = evt.SmallBlind;
            state.BigBlind = evt.BigBlind;
            state.MinBuyIn = evt.MinBuyIn;
            state.MaxBuyIn = evt.MaxBuyIn;
            state.MaxPlayers = evt.MaxPlayers;
            state.ActionTimeoutSeconds = evt.ActionTimeoutSeconds;
            state.Status = "waiting";
        })
        .On<PlayerJoined>((state, evt) =>
        {
            state.Seats[evt.SeatPosition] = new SeatState
            {
                Position = evt.SeatPosition,
                PlayerRoot = evt.PlayerRoot,
                Stack = evt.Stack
            };
        })
        .On<PlayerLeft>((state, evt) =>
        {
            state.Seats.Remove(evt.SeatPosition);
        })
        .On<PlayerSatOut>((state, evt) =>
        {
            var seat = state.Seats.Values.FirstOrDefault(s => s.PlayerRoot.Equals(evt.PlayerRoot));
            if (seat != null) seat.IsSittingOut = true;
        })
        .On<PlayerSatIn>((state, evt) =>
        {
            var seat = state.Seats.Values.FirstOrDefault(s => s.PlayerRoot.Equals(evt.PlayerRoot));
            if (seat != null) seat.IsSittingOut = false;
        })
        .On<HandStarted>((state, evt) =>
        {
            state.HandCount = evt.HandNumber;
            state.CurrentHandRoot = evt.HandRoot;
            state.DealerPosition = evt.DealerPosition;
            state.Status = "in_hand";
        })
        .On<HandEnded>((state, evt) =>
        {
            state.CurrentHandRoot = ByteString.Empty;
            state.Status = "waiting";
            foreach (var kvp in evt.StackChanges)
            {
                var playerRoot = ByteString.CopyFrom(Convert.FromHexString(kvp.Key));
                var seat = state.Seats.Values.FirstOrDefault(s => s.PlayerRoot.Equals(playerRoot));
                if (seat != null) seat.Stack += kvp.Value;
            }
        })
        .On<ChipsAdded>((state, evt) =>
        {
            var seat = state.Seats.Values.FirstOrDefault(s => s.PlayerRoot.Equals(evt.PlayerRoot));
            if (seat != null) seat.Stack = evt.NewStack;
        });

    /// <summary>
    /// Build state from an EventBook by applying all events.
    /// </summary>
    public static TableState FromEventBook(EventBook eventBook)
    {
        return Router.WithEventBook(eventBook);
    }
}
