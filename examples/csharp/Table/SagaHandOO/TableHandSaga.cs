// DOC: This file is referenced in docs/docs/examples/sagas.mdx
//      Update documentation when making changes to saga patterns.
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Table.SagaHandOO;

// docs:start:saga_oo
/// <summary>
/// Saga: Table -> Hand (OO Pattern)
///
/// Reacts to HandStarted events from Table domain.
/// Sends DealCards commands to Hand domain.
/// Sagas are stateless translators - framework handles sequence stamping.
///
/// Uses annotation-based handler registration with:
/// - [Handles(typeof(EventType))] for handle phase handlers
/// </summary>
public class TableHandSaga : Saga
{
    public override string Name => "saga-table-hand";
    public override string InputDomain => "table";
    public override string OutputDomain => "hand";

    /// <summary>
    /// Handle phase: translate Table.HandStarted -> Hand.DealCards.
    ///
    /// Called with the source event. Framework handles sequence stamping.
    /// </summary>
    [Handles(typeof(HandStarted))]
    public CommandBook HandleHandStarted(HandStarted evt, List<EventBook> destinations)
    {
        // Sagas are stateless - destinations not used, framework stamps sequences

        // Convert SeatSnapshot to PlayerInHand
        var players = evt
            .ActivePlayers.Select(seat => new PlayerInHand
            {
                PlayerRoot = seat.PlayerRoot,
                Position = seat.Position,
                Stack = seat.Stack,
            })
            .ToList();

        // Build DealCards command
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
                    Command = PackCommand(dealCards),
                },
            },
        };
    }
}
// docs:end:saga_oo
