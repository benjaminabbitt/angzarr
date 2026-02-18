using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.SagaPlayer;

/// <summary>
/// Saga: Table -> Player
/// Reacts to HandEnded events from Table domain.
/// Sends ReleaseFunds commands to Player domain.
/// </summary>
public static class TablePlayerSaga
{
    public static EventRouter Create()
    {
        return new EventRouter("saga-table-player", "table")
            .Sends("player", "ReleaseFunds")
            .Prepare<HandEnded>(PrepareHandEnded)
            .On<HandEnded>(HandleHandEnded);
    }

    private static List<Cover> PrepareHandEnded(HandEnded evt)
    {
        return evt.StackChanges.Keys.Select(playerHex =>
        {
            var playerRoot = ByteString.CopyFrom(Convert.FromHexString(playerHex));
            return new Cover
            {
                Domain = "player",
                Root = new UUID { Value = playerRoot }
            };
        }).ToList();
    }

    private static object HandleHandEnded(HandEnded evt, List<EventBook> destinations)
    {
        var destMap = destinations
            .Where(d => d.Cover?.Root != null)
            .ToDictionary(
                d => Convert.ToHexString(d.Cover.Root.Value.ToByteArray()).ToLowerInvariant(),
                d => d
            );

        var commands = new List<CommandBook>();

        foreach (var playerHex in evt.StackChanges.Keys)
        {
            var playerRoot = ByteString.CopyFrom(Convert.FromHexString(playerHex));
            var destSeq = destMap.TryGetValue(playerHex.ToLowerInvariant(), out var dest)
                ? EventRouter.NextSequence(dest)
                : 0;

            var releaseFunds = new ReleaseFunds
            {
                TableRoot = evt.HandRoot
            };

            var cmdAny = EventRouter.PackCommand(releaseFunds);

            commands.Add(new CommandBook
            {
                Cover = new Cover
                {
                    Domain = "player",
                    Root = new UUID { Value = playerRoot }
                },
                Pages = { new CommandPage { Sequence = destSeq, Command = cmdAny } }
            });
        }

        return commands;
    }
}
