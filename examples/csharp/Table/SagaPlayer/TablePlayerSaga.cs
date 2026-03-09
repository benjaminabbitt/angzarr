using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Table.SagaPlayer;

/// <summary>
/// Saga: Table -> Player
/// Reacts to HandEnded events from Table domain.
/// Sends ReleaseFunds commands to Player domain.
/// Sagas are stateless translators - framework handles sequence stamping.
/// </summary>
public static class TablePlayerSaga
{
    public static EventRouter Create()
    {
        return new EventRouter("saga-table-player").Domain("table").On<HandEnded>(HandleHandEnded);
    }

    private static object HandleHandEnded(HandEnded evt, List<EventBook> destinations)
    {
        // Sagas are stateless - destinations not used, framework stamps sequences
        var commands = new List<CommandBook>();

        foreach (var playerHex in evt.StackChanges.Keys)
        {
            var playerRoot = ByteString.CopyFrom(Convert.FromHexString(playerHex));

            var releaseFunds = new ReleaseFunds { TableRoot = evt.HandRoot };

            var cmdAny = EventRouter.PackCommand(releaseFunds);

            commands.Add(
                new CommandBook
                {
                    Cover = new Cover
                    {
                        Domain = "player",
                        Root = new UUID { Value = playerRoot },
                    },
                    Pages =
                    {
                        new CommandPage
                        {
                            Header = new PageHeader
                            {
                                AngzarrDeferred = new AngzarrDeferredSequence(),
                            },
                            Command = cmdAny,
                        },
                    },
                }
            );
        }

        return commands;
    }
}
