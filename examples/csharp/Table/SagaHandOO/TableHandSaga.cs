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
///
/// Uses annotation-based handler registration with:
/// - [Prepares(typeof(EventType))] for prepare phase handlers
/// - [Handles(typeof(EventType))] for execute phase handlers
/// </summary>
public class TableHandSaga : Saga
{
    public override string Name => "saga-table-hand";
    public override string InputDomain => "table";
    public override string OutputDomain => "hand";

    /// <summary>
    /// Prepare phase: declare which destination aggregates we need to read.
    ///
    /// Called during the prepare phase of the two-phase saga protocol.
    /// Returns a list of Cover objects identifying the destination aggregates
    /// needed for the execute phase.
    /// </summary>
    [Prepares(typeof(HandStarted))]
    public List<Cover> PrepareHandStarted(HandStarted evt)
    {
        return new List<Cover>
        {
            new Cover
            {
                Domain = "hand",
                Root = new UUID { Value = evt.HandRoot },
            },
        };
    }

    /// <summary>
    /// Execute phase: translate Table.HandStarted -> Hand.DealCards.
    ///
    /// Called during the execute phase with the source event and
    /// fetched destination EventBooks. Returns the command to send.
    /// </summary>
    [Handles(typeof(HandStarted))]
    public CommandBook HandleHandStarted(HandStarted evt, List<EventBook> destinations)
    {
        var destSeq = NextSequence(destinations.Count > 0 ? destinations[0] : null);

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
                new CommandPage { Sequence = destSeq, Command = PackCommand(dealCards) },
            },
        };
    }
}
// docs:end:saga_oo
