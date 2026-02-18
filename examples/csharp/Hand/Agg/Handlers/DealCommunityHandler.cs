using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.Agg.Handlers;

/// <summary>
/// Handler for DealCommunityCards command.
/// </summary>
public static class DealCommunityHandler
{
    public static CommunityCardsDealt Handle(DealCommunityCards cmd, HandState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (state.Status == "complete")
            throw CommandRejectedError.PreconditionFailed("Hand is complete");

        // Validate
        if (cmd.Count <= 0)
            throw CommandRejectedError.InvalidArgument("Must deal at least 1 card");
        if (state.GameVariant == GameVariant.FiveCardDraw)
            throw CommandRejectedError.PreconditionFailed("Five card draw doesn't have community cards");

        var (nextPhase, expectedCards) = GetNextPhase(state.CurrentPhase);
        if (nextPhase == BettingPhase.Unspecified)
            throw CommandRejectedError.PreconditionFailed("No more phases");
        if (expectedCards != cmd.Count)
            throw CommandRejectedError.InvalidArgument($"Expected {expectedCards} cards for this phase");
        if (state.RemainingDeck.Count < cmd.Count)
            throw CommandRejectedError.PreconditionFailed("Not enough cards in deck");

        // Compute
        var newCards = state.RemainingDeck.Take(cmd.Count).ToList();
        var allCommunity = state.CommunityCards.Concat(newCards).ToList();

        var evt = new CommunityCardsDealt
        {
            Phase = nextPhase,
            DealtAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        foreach (var (suit, rank) in newCards)
            evt.Cards.Add(new Card { Suit = suit, Rank = rank });
        foreach (var (suit, rank) in allCommunity)
            evt.AllCommunityCards.Add(new Card { Suit = suit, Rank = rank });

        return evt;
    }

    private static (BettingPhase NextPhase, int CardsToDealt) GetNextPhase(BettingPhase current)
    {
        return current switch
        {
            BettingPhase.Preflop => (BettingPhase.Flop, 3),
            BettingPhase.Flop => (BettingPhase.Turn, 1),
            BettingPhase.Turn => (BettingPhase.River, 1),
            BettingPhase.River => (BettingPhase.Showdown, 0),
            _ => (BettingPhase.Unspecified, 0)
        };
    }
}
