using System.Security.Cryptography;
using System.Text;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.Agg;

/// <summary>
/// Table aggregate - OO style with decorator-based command dispatch.
/// </summary>
public class TableAggregate : Aggregate<TableState>
{
    public const string Domain = "table";

    protected override void ApplyEvent(TableState state, Any eventAny)
    {
        TableState.Router.ApplySingle(state, eventAny);
    }

    // --- State accessors ---

    public bool Exists => State.Exists;
    public string TableId => State.TableId;
    public string TableName => State.TableName;
    public GameVariant GameVariant => State.GameVariant;
    public long SmallBlind => State.SmallBlind;
    public long BigBlind => State.BigBlind;
    public long MinBuyIn => State.MinBuyIn;
    public long MaxBuyIn => State.MaxBuyIn;
    public int MaxPlayers => State.MaxPlayers;
    public Dictionary<int, SeatState> Seats => State.Seats;
    public int DealerPosition => State.DealerPosition;
    public long HandCount => State.HandCount;
    public ByteString CurrentHandRoot => State.CurrentHandRoot;
    public string Status => State.Status;
    public int PlayerCount => State.PlayerCount;
    public int ActivePlayerCount => State.ActivePlayerCount;
    public bool IsFull => State.IsFull;

    public string? GetSeatOccupant(int seat)
    {
        var seatState = State.GetSeat(seat);
        return seatState?.PlayerRoot.ToStringUtf8();
    }

    // --- Command handlers ---

    [Handles(typeof(CreateTable))]
    public TableCreated HandleCreate(CreateTable cmd)
    {
        if (Exists)
            throw CommandRejectedError.PreconditionFailed("Table already exists");
        if (string.IsNullOrEmpty(cmd.TableName))
            throw CommandRejectedError.InvalidArgument("table_name is required");
        if (cmd.SmallBlind <= 0)
            throw CommandRejectedError.InvalidArgument("small_blind must be positive");
        if (cmd.BigBlind <= 0)
            throw CommandRejectedError.InvalidArgument("big_blind must be positive");
        if (cmd.BigBlind < cmd.SmallBlind)
            throw CommandRejectedError.InvalidArgument("big_blind must be >= small_blind");
        if (cmd.MaxPlayers < 2 || cmd.MaxPlayers > 10)
            throw CommandRejectedError.InvalidArgument("max_players must be between 2 and 10");

        return new TableCreated
        {
            TableName = cmd.TableName,
            GameVariant = cmd.GameVariant,
            SmallBlind = cmd.SmallBlind,
            BigBlind = cmd.BigBlind,
            MinBuyIn = cmd.MinBuyIn != 0 ? cmd.MinBuyIn : cmd.BigBlind * 20,
            MaxBuyIn = cmd.MaxBuyIn != 0 ? cmd.MaxBuyIn : cmd.BigBlind * 100,
            MaxPlayers = cmd.MaxPlayers != 0 ? cmd.MaxPlayers : 9,
            ActionTimeoutSeconds = cmd.ActionTimeoutSeconds != 0 ? cmd.ActionTimeoutSeconds : 30,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(JoinTable))]
    public PlayerJoined HandleJoin(JoinTable cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");
        if (State.FindPlayerSeat(cmd.PlayerRoot) != null)
            throw CommandRejectedError.PreconditionFailed("Player already seated at table");
        if (IsFull)
            throw CommandRejectedError.PreconditionFailed("Table is full");
        if (cmd.BuyInAmount < MinBuyIn)
            throw CommandRejectedError.InvalidArgument($"Buy-in must be at least {MinBuyIn}");
        if (cmd.BuyInAmount > MaxBuyIn)
            throw CommandRejectedError.InvalidArgument($"Buy-in cannot exceed {MaxBuyIn}");
        if (cmd.PreferredSeat > 0 && State.GetSeat(cmd.PreferredSeat) != null)
            throw CommandRejectedError.PreconditionFailed("Seat is occupied");

        var seatPosition = State.FindAvailableSeat(cmd.PreferredSeat) ?? 0;

        return new PlayerJoined
        {
            PlayerRoot = cmd.PlayerRoot,
            SeatPosition = seatPosition,
            BuyInAmount = cmd.BuyInAmount,
            Stack = cmd.BuyInAmount,
            JoinedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(LeaveTable))]
    public PlayerLeft HandleLeave(LeaveTable cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");

        var seat = State.FindPlayerSeat(cmd.PlayerRoot);
        if (seat == null)
            throw CommandRejectedError.PreconditionFailed("Player is not seated at table");
        if (Status == "in_hand")
            throw CommandRejectedError.PreconditionFailed("Cannot leave table during a hand");

        return new PlayerLeft
        {
            PlayerRoot = cmd.PlayerRoot,
            SeatPosition = seat.Position,
            ChipsCashedOut = seat.Stack,
            LeftAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(SitOut))]
    public PlayerSatOut HandleSitOut(SitOut cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");

        var seat = State.FindPlayerSeat(cmd.PlayerRoot);
        if (seat == null)
            throw CommandRejectedError.PreconditionFailed("Player is not seated at table");

        return new PlayerSatOut
        {
            PlayerRoot = cmd.PlayerRoot,
            SatOutAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(SitIn))]
    public PlayerSatIn HandleSitIn(SitIn cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");

        var seat = State.FindPlayerSeat(cmd.PlayerRoot);
        if (seat == null)
            throw CommandRejectedError.PreconditionFailed("Player is not seated at table");

        return new PlayerSatIn
        {
            PlayerRoot = cmd.PlayerRoot,
            SatInAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    [Handles(typeof(StartHand))]
    public HandStarted HandleStartHand(StartHand cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");
        if (Status == "in_hand")
            throw CommandRejectedError.PreconditionFailed("Hand already in progress");
        if (ActivePlayerCount < 2)
            throw CommandRejectedError.PreconditionFailed("Not enough players to start hand");

        var handNumber = HandCount + 1;
        var handRoot = GenerateHandRoot(TableId, handNumber);
        var dealerPosition = State.NextDealerPosition();

        var activePositions = Seats.Values
            .Where(s => !s.IsSittingOut)
            .Select(s => s.Position)
            .OrderBy(p => p)
            .ToList();

        var dealerIdx = activePositions.IndexOf(dealerPosition);
        if (dealerIdx < 0) dealerIdx = 0;

        int sbPosition, bbPosition;
        if (activePositions.Count == 2)
        {
            sbPosition = activePositions[dealerIdx];
            bbPosition = activePositions[(dealerIdx + 1) % 2];
        }
        else
        {
            sbPosition = activePositions[(dealerIdx + 1) % activePositions.Count];
            bbPosition = activePositions[(dealerIdx + 2) % activePositions.Count];
        }

        var activePlayers = activePositions.Select(pos =>
        {
            var seat = Seats[pos];
            return new SeatSnapshot
            {
                Position = pos,
                PlayerRoot = seat.PlayerRoot,
                Stack = seat.Stack
            };
        }).ToList();

        var evt = new HandStarted
        {
            HandRoot = ByteString.CopyFrom(handRoot),
            HandNumber = handNumber,
            DealerPosition = dealerPosition,
            SmallBlindPosition = sbPosition,
            BigBlindPosition = bbPosition,
            GameVariant = GameVariant,
            SmallBlind = SmallBlind,
            BigBlind = BigBlind,
            StartedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.ActivePlayers.AddRange(activePlayers);

        return evt;
    }

    [Handles(typeof(EndHand))]
    public HandEnded HandleEndHand(EndHand cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");
        if (Status != "in_hand")
            throw CommandRejectedError.PreconditionFailed("No hand in progress");
        if (!cmd.HandRoot.Equals(CurrentHandRoot))
            throw CommandRejectedError.PreconditionFailed("Hand root mismatch");

        var stackChanges = new Dictionary<string, long>();
        foreach (var result in cmd.Results)
        {
            var playerHex = Convert.ToHexString(result.WinnerRoot.ToByteArray()).ToLowerInvariant();
            if (!stackChanges.ContainsKey(playerHex))
                stackChanges[playerHex] = 0;
            stackChanges[playerHex] += result.Amount;
        }

        var evt = new HandEnded
        {
            HandRoot = cmd.HandRoot,
            EndedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.Results.AddRange(cmd.Results);
        foreach (var kvp in stackChanges)
        {
            evt.StackChanges[kvp.Key] = kvp.Value;
        }

        return evt;
    }

    [Handles(typeof(AddChips))]
    public ChipsAdded HandleAddChips(AddChips cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");

        var seat = State.FindPlayerSeat(cmd.PlayerRoot);
        if (seat == null)
            throw CommandRejectedError.PreconditionFailed("Player is not seated at table");

        var newStack = seat.Stack + cmd.Amount;
        return new ChipsAdded
        {
            PlayerRoot = cmd.PlayerRoot,
            Amount = cmd.Amount,
            NewStack = newStack,
            AddedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }

    private static byte[] GenerateHandRoot(string tableId, long handNumber)
    {
        using var sha = SHA256.Create();
        var input = $"angzarr.poker.hand.{tableId}.{handNumber}";
        var hash = sha.ComputeHash(Encoding.UTF8.GetBytes(input));
        return hash.Take(16).ToArray();
    }
}
