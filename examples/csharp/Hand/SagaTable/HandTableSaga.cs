using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Hand.SagaTable;

/// <summary>
/// Saga: Hand -> Table
/// Reacts to HandComplete events from Hand domain.
/// Sends EndHand commands to Table domain.
/// Sagas are stateless translators - framework handles sequence stamping.
/// </summary>
public static class HandTableSaga
{
    public static EventRouter Create()
    {
        return new EventRouter("saga-hand-table")
            .Domain("hand")
            .On<HandComplete>(HandleHandComplete);
    }

    private static object HandleHandComplete(HandComplete evt, List<EventBook> destinations)
    {
        // Sagas are stateless - destinations not used, framework stamps sequences
        var results = evt
            .Winners.Select(winner => new PotResult
            {
                WinnerRoot = winner.PlayerRoot,
                Amount = winner.Amount,
                PotType = winner.PotType,
                WinningHand = winner.WinningHand,
            })
            .ToList();

        var endHand = new EndHand();
        endHand.Results.AddRange(results);

        var cmdAny = EventRouter.PackCommand(endHand);

        return new CommandBook
        {
            Cover = new Cover
            {
                Domain = "table",
                Root = new UUID { Value = evt.TableRoot },
            },
            Pages =
            {
                new CommandPage
                {
                    Header = new PageHeader { AngzarrDeferred = new AngzarrDeferredSequence() },
                    Command = cmdAny,
                },
            },
        };
    }
}
