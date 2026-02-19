using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.SagaTable;

/// <summary>
/// Saga: Hand -> Table
/// Reacts to HandComplete events from Hand domain.
/// Sends EndHand commands to Table domain.
/// </summary>
public static class HandTableSaga
{
    public static EventRouter Create()
    {
        return new EventRouter("saga-hand-table")
            .Domain("hand")
            .Prepare<HandComplete>(PrepareHandComplete)
            .On<HandComplete>(HandleHandComplete);
    }

    private static List<Cover> PrepareHandComplete(HandComplete evt)
    {
        return new List<Cover>
        {
            new Cover
            {
                Domain = "table",
                Root = new UUID { Value = evt.TableRoot }
            }
        };
    }

    private static object HandleHandComplete(HandComplete evt, List<EventBook> destinations)
    {
        var destSeq = EventRouter.NextSequence(destinations.FirstOrDefault());

        var results = evt.Winners.Select(winner => new PotResult
        {
            WinnerRoot = winner.PlayerRoot,
            Amount = winner.Amount,
            PotType = winner.PotType,
            WinningHand = winner.WinningHand
        }).ToList();

        var endHand = new EndHand();
        endHand.Results.AddRange(results);

        var cmdAny = EventRouter.PackCommand(endHand);

        return new CommandBook
        {
            Cover = new Cover
            {
                Domain = "table",
                Root = new UUID { Value = evt.TableRoot }
            },
            Pages = { new CommandPage { Sequence = destSeq, Command = cmdAny } }
        };
    }
}
