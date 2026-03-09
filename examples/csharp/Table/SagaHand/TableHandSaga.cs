// DOC: This file is referenced in docs/docs/examples/sagas.mdx
//      Update documentation when making changes to saga patterns.

using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Table.SagaHand;

/// <summary>
/// Saga: Table -> Hand
/// Reacts to HandStarted events from Table domain.
/// Sends DealCards commands to Hand domain.
/// Sagas are stateless translators - framework handles sequence stamping.
/// </summary>
public static class TableHandSaga
{
    // docs:start:event_router
    public static EventRouter Create()
    {
        return new EventRouter("saga-table-hand")
            .Domain("table")
            .On<HandStarted>(HandleHandStarted);
    }

    // docs:end:event_router

    // docs:start:saga_handler
    private static object HandleHandStarted(HandStarted evt, List<EventBook> destinations)
    {
        // Sagas are stateless - destinations not used, framework stamps sequences
        var players = evt
            .ActivePlayers.Select(seat => new PlayerInHand
            {
                PlayerRoot = seat.PlayerRoot,
                Position = seat.Position,
                Stack = seat.Stack,
            })
            .ToList();

        var dealCards = new DealCards
        {
            TableRoot = evt.HandRoot,
            HandNumber = evt.HandNumber,
            GameVariant = evt.GameVariant,
            DealerPosition = evt.DealerPosition,
            SmallBlind = evt.SmallBlind,
            BigBlind = evt.BigBlind,
        };
        dealCards.Players.AddRange(players);

        var cmdAny = EventRouter.PackCommand(dealCards);

        return new CommandBook
        {
            Cover = new Cover
            {
                Domain = "hand",
                Root = new UUID { Value = evt.HandRoot },
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
    // docs:end:saga_handler
}
