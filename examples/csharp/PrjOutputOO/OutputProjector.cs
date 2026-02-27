using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;

namespace PrjOutputOO;

// docs:start:projector_oo
/// <summary>
/// Projector: Output (OO Pattern)
///
/// Subscribes to player, table, and hand domain events.
/// Writes formatted game logs to a file.
///
/// This is the OO-style implementation using Projector base class with
/// [Projects(typeof(EventType))] annotated methods.
/// </summary>
public class OutputProjector : Projector
{
    private static readonly string LogFile =
        Environment.GetEnvironmentVariable("HAND_LOG_FILE") ?? "hand_log_oo.txt";

    private static StreamWriter? _logWriter;

    public override string Name => "output";
    public override IReadOnlyList<string> InputDomains => new[] { "player", "table", "hand" };

    private static StreamWriter GetLogWriter()
    {
        if (_logWriter == null)
        {
            _logWriter = new StreamWriter(LogFile, append: true);
        }
        return _logWriter;
    }

    private void WriteLog(string msg)
    {
        var writer = GetLogWriter();
        var timestamp = DateTime.UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ");
        writer.WriteLine($"[{timestamp}] {msg}");
        writer.Flush();
    }

    private static string TruncateId(ByteString playerRoot)
    {
        var bytes = playerRoot.ToByteArray();
        if (bytes.Length >= 4)
        {
            return $"{bytes[0]:x2}{bytes[1]:x2}{bytes[2]:x2}{bytes[3]:x2}";
        }
        return Convert.ToHexString(bytes).ToLowerInvariant();
    }

    [Projects(typeof(PlayerRegistered))]
    public Projection ProjectRegistered(PlayerRegistered evt)
    {
        WriteLog($"PLAYER registered: {evt.DisplayName} ({evt.Email})");
        return new Projection { Projector = Name };
    }

    [Projects(typeof(FundsDeposited))]
    public Projection ProjectDeposited(FundsDeposited evt)
    {
        var amount = evt.Amount?.Amount ?? 0;
        var newBalance = evt.NewBalance?.Amount ?? 0;
        WriteLog($"PLAYER deposited {amount}, balance: {newBalance}");
        return new Projection { Projector = Name };
    }

    [Projects(typeof(TableCreated))]
    public Projection ProjectTableCreated(TableCreated evt)
    {
        WriteLog($"TABLE created: {evt.TableName} ({evt.GameVariant})");
        return new Projection { Projector = Name };
    }

    [Projects(typeof(PlayerJoined))]
    public Projection ProjectPlayerJoined(PlayerJoined evt)
    {
        var playerId = TruncateId(evt.PlayerRoot);
        WriteLog($"TABLE player {playerId} joined with {evt.Stack} chips");
        return new Projection { Projector = Name };
    }

    [Projects(typeof(HandStarted))]
    public Projection ProjectHandStarted(HandStarted evt)
    {
        WriteLog(
            $"TABLE hand #{evt.HandNumber} started, {evt.ActivePlayers.Count} players, dealer at position {evt.DealerPosition}"
        );
        return new Projection { Projector = Name };
    }

    [Projects(typeof(CardsDealt))]
    public Projection ProjectCardsDealt(CardsDealt evt)
    {
        WriteLog($"HAND cards dealt to {evt.PlayerCards.Count} players");
        return new Projection { Projector = Name };
    }

    [Projects(typeof(BlindPosted))]
    public Projection ProjectBlindPosted(BlindPosted evt)
    {
        var playerId = TruncateId(evt.PlayerRoot);
        WriteLog($"HAND player {playerId} posted {evt.BlindType} blind: {evt.Amount}");
        return new Projection { Projector = Name };
    }

    [Projects(typeof(ActionTaken))]
    public Projection ProjectActionTaken(ActionTaken evt)
    {
        var playerId = TruncateId(evt.PlayerRoot);
        WriteLog($"HAND player {playerId}: {evt.Action} {evt.Amount}");
        return new Projection { Projector = Name };
    }

    [Projects(typeof(PotAwarded))]
    public Projection ProjectPotAwarded(PotAwarded evt)
    {
        var winners = string.Join(
            ", ",
            evt.Winners.Select(w => $"{TruncateId(w.PlayerRoot)} wins {w.Amount}")
        );
        WriteLog($"HAND pot awarded: {winners}");
        return new Projection { Projector = Name };
    }

    [Projects(typeof(HandComplete))]
    public Projection ProjectHandComplete(HandComplete evt)
    {
        WriteLog($"HAND #{evt.HandNumber} complete");
        return new Projection { Projector = Name };
    }
}
// docs:end:projector_oo
