using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.Agg.Handlers;

/// <summary>
/// Handler for DealCards command.
/// </summary>
public static class DealHandler
{
    public static CardsDealt Handle(DealCards cmd, HandState state)
    {
        // Guard
        if (state.Exists)
            throw CommandRejectedError.PreconditionFailed("Hand already dealt");

        // Validate
        if (cmd.Players.Count == 0)
            throw CommandRejectedError.InvalidArgument("No players in hand");
        if (cmd.Players.Count < 2)
            throw CommandRejectedError.InvalidArgument("Need at least 2 players");

        // Compute
        var playerCards = DealHoleCards(cmd.GameVariant, cmd.Players.ToList(), cmd.DeckSeed);

        var evt = new CardsDealt
        {
            TableRoot = cmd.TableRoot,
            HandNumber = cmd.HandNumber,
            GameVariant = cmd.GameVariant,
            DealerPosition = cmd.DealerPosition,
            DealtAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.PlayerCards.AddRange(playerCards);
        evt.Players.AddRange(cmd.Players);

        return evt;
    }

    private static List<PlayerHoleCards> DealHoleCards(GameVariant variant, List<PlayerInHand> players, ByteString? seed)
    {
        var cardsPerPlayer = variant switch
        {
            GameVariant.TexasHoldem => 2,
            GameVariant.Omaha => 4,
            GameVariant.FiveCardDraw => 5,
            GameVariant.SevenCardStud => 7,
            _ => 2
        };

        var deck = BuildDeck(seed);
        var result = new List<PlayerHoleCards>();
        var deckIndex = 0;

        foreach (var player in players)
        {
            var pc = new PlayerHoleCards { PlayerRoot = player.PlayerRoot };
            for (var i = 0; i < cardsPerPlayer && deckIndex < deck.Count; i++)
            {
                pc.Cards.Add(deck[deckIndex++]);
            }
            result.Add(pc);
        }

        return result;
    }

    private static List<Card> BuildDeck(ByteString? seed)
    {
        var cards = new List<Card>();
        foreach (Suit suit in new[] { Suit.Clubs, Suit.Diamonds, Suit.Hearts, Suit.Spades })
        {
            for (var rank = Rank.Two; rank <= Rank.Ace; rank++)
            {
                cards.Add(new Card { Suit = suit, Rank = rank });
            }
        }

        var rng = seed != null && !seed.IsEmpty
            ? new Random(BitConverter.ToInt32(seed.ToByteArray().Take(4).ToArray(), 0))
            : new Random();

        for (var i = cards.Count - 1; i > 0; i--)
        {
            var j = rng.Next(i + 1);
            (cards[i], cards[j]) = (cards[j], cards[i]);
        }

        return cards;
    }
}
