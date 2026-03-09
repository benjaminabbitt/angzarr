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
/// Sagas are stateless translators - framework handles sequence stamping.
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
    /// Handle phase: translate PlayerSittingOut -> PlayerSatOut fact for table.
    /// </summary>
    private static object HandleSittingOut(PlayerSittingOut evt, List<EventBook> destinations)
    {
        // Sagas are stateless - destinations not used, framework stamps sequences
        var satOut = new PlayerSatOut { PlayerRoot = _currentSourceRoot, SatOutAt = evt.SatOutAt };

        var factAny = Any.Pack(satOut, "type.googleapis.com/");

        return new EventBook
        {
            Cover = new Cover
            {
                Domain = "table",
                Root = new UUID { Value = evt.TableRoot },
            },
            Pages =
            {
                new EventPage
                {
                    Header = new PageHeader { AngzarrDeferred = new AngzarrDeferredSequence() },
                    Event = factAny,
                },
            },
        };
    }

    /// <summary>
    /// Handle phase: translate PlayerReturningToPlay -> PlayerSatIn fact for table.
    /// </summary>
    private static object HandleReturningToPlay(
        PlayerReturningToPlay evt,
        List<EventBook> destinations
    )
    {
        // Sagas are stateless - destinations not used, framework stamps sequences
        var satIn = new PlayerSatIn { PlayerRoot = _currentSourceRoot, SatInAt = evt.SatInAt };

        var factAny = Any.Pack(satIn, "type.googleapis.com/");

        return new EventBook
        {
            Cover = new Cover
            {
                Domain = "table",
                Root = new UUID { Value = evt.TableRoot },
            },
            Pages =
            {
                new EventPage
                {
                    Header = new PageHeader { AngzarrDeferred = new AngzarrDeferredSequence() },
                    Event = factAny,
                },
            },
        };
    }
}
