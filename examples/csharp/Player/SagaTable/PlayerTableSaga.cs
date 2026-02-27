using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Player.SagaTable;

/// <summary>
/// Saga: Player -> Table
///
/// Propagates player sit-out/sit-in intent as facts to the table domain.
///
/// Flow:
/// - PlayerSittingOut -> PlayerSatOut fact to table
/// - PlayerReturningToPlay -> PlayerSatIn fact to table
/// </summary>
public static class PlayerTableSaga
{
    /// <summary>
    /// Stored source root during dispatch for handler access.
    /// </summary>
    private static ByteString _currentSourceRoot = ByteString.Empty;

    public static EventRouter Create()
    {
        return new EventRouter("saga-player-table")
            .Domain("player")
            .Prepare<PlayerSittingOut>(PrepareSittingOut)
            .Prepare<PlayerReturningToPlay>(PrepareReturningToPlay)
            .On<PlayerSittingOut>(HandleSittingOut)
            .On<PlayerReturningToPlay>(HandleReturningToPlay);
    }

    /// <summary>
    /// Set source root from the event book before processing.
    /// </summary>
    public static void SetSourceRoot(EventBook? source)
    {
        if (source?.Cover?.Root != null)
        {
            _currentSourceRoot = source.Cover.Root.Value;
        }
        else
        {
            _currentSourceRoot = ByteString.Empty;
        }
    }

    /// <summary>
    /// Prepare phase: no destinations needed (emits facts, not commands).
    /// </summary>
    private static List<Cover> PrepareSittingOut(PlayerSittingOut evt)
    {
        return new List<Cover>();
    }

    /// <summary>
    /// Prepare phase: no destinations needed (emits facts, not commands).
    /// </summary>
    private static List<Cover> PrepareReturningToPlay(PlayerReturningToPlay evt)
    {
        return new List<Cover>();
    }

    /// <summary>
    /// Execute phase: translate PlayerSittingOut -> PlayerSatOut fact for table.
    /// </summary>
    private static object HandleSittingOut(PlayerSittingOut evt, List<EventBook> destinations)
    {
        var satOut = new PlayerSatOut { PlayerRoot = _currentSourceRoot, SatOutAt = evt.SatOutAt };

        var factAny = Any.Pack(satOut, "type.googleapis.com/");

        return new EventBook
        {
            Cover = new Cover
            {
                Domain = "table",
                Root = new UUID { Value = evt.TableRoot },
            },
            Pages = { new EventPage { Event = factAny } },
        };
    }

    /// <summary>
    /// Execute phase: translate PlayerReturningToPlay -> PlayerSatIn fact for table.
    /// </summary>
    private static object HandleReturningToPlay(
        PlayerReturningToPlay evt,
        List<EventBook> destinations
    )
    {
        var satIn = new PlayerSatIn { PlayerRoot = _currentSourceRoot, SatInAt = evt.SatInAt };

        var factAny = Any.Pack(satIn, "type.googleapis.com/");

        return new EventBook
        {
            Cover = new Cover
            {
                Domain = "table",
                Root = new UUID { Value = evt.TableRoot },
            },
            Pages = { new EventPage { Event = factAny } },
        };
    }
}
