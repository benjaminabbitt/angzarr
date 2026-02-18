using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.SagaHand;

/// <summary>
/// Saga: Table -> Hand
/// Reacts to HandStarted events from Table domain.
/// Sends DealCards commands to Hand domain.
/// </summary>
public static class TableHandSaga
{
    public static EventRouter Create()
    {
        return new EventRouter("saga-table-hand", "table")
            .Sends("hand", "DealCards")
            .Prepare<HandStarted>(PrepareHandStarted)
            .On<HandStarted>(HandleHandStarted);
    }

    private static List<Cover> PrepareHandStarted(HandStarted evt)
    {
        return new List<Cover>
        {
            new Cover
            {
                Domain = "hand",
                Root = new UUID { Value = evt.HandRoot }
            }
        };
    }

    private static object HandleHandStarted(HandStarted evt, List<EventBook> destinations)
    {
        var destSeq = EventRouter.NextSequence(destinations.FirstOrDefault());

        var players = evt.ActivePlayers.Select(seat => new PlayerInHand
        {
            PlayerRoot = seat.PlayerRoot,
            Position = seat.Position,
            Stack = seat.Stack
        }).ToList();

        var dealCards = new DealCards
        {
            TableRoot = evt.HandRoot,
            HandNumber = evt.HandNumber,
            GameVariant = evt.GameVariant,
            DealerPosition = evt.DealerPosition,
            SmallBlind = evt.SmallBlind,
            BigBlind = evt.BigBlind
        };
        dealCards.Players.AddRange(players);

        var cmdAny = EventRouter.PackCommand(dealCards);

        return new CommandBook
        {
            Cover = new Cover
            {
                Domain = "hand",
                Root = new UUID { Value = evt.HandRoot }
            },
            Pages = { new CommandPage { Sequence = destSeq, Command = cmdAny } }
        };
    }
}
