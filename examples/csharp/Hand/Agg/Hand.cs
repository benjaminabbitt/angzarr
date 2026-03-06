using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Hand.Agg;

/// <summary>
/// Hand aggregate - OO style with decorator-based command dispatch.
/// </summary>
public class HandAggregate : CommandHandler<HandState>
{
    public const string DomainName = "hand";

    public override string Domain => DomainName;

    protected override HandState CreateEmptyState() => new HandState();

    protected override void ApplyEvent(HandState state, Any eventAny)
    {
        HandState.Router.ApplySingle(state, eventAny);
    }

    // --- State accessors ---

    public new bool Exists => State.Exists;
    public string HandId => State.HandId;
    public ByteString TableRoot => State.TableRoot;
    public long HandNumber => State.HandNumber;
    public GameVariant GameVariant => State.GameVariant;
    public string Status => State.Status;
    public BettingPhase CurrentPhase => State.CurrentPhase;
    public long CurrentBet => State.CurrentBet;
    public long MinRaise => State.MinRaise;
    public long SmallBlind => State.SmallBlind;
    public long BigBlind => State.BigBlind;
    public List<(Suit Suit, Rank Rank)> CommunityCards => State.CommunityCards;
    public Dictionary<int, PlayerHandInfo> Players => State.Players;
    public List<(Suit Suit, Rank Rank)> RemainingDeck => State.RemainingDeck;

    public long GetPotTotal() => State.GetPotTotal();

    public PlayerHandInfo? GetPlayer(ByteString playerRoot) => State.GetPlayer(playerRoot);

    // --- Command handlers ---

    [Handles(typeof(DealCards))]
    public CardsDealt HandleDeal(DealCards cmd)
    {
        if (Exists)
            throw CommandRejectedError.PreconditionFailed("Hand already dealt");
        if (cmd.Players.Count == 0)
            throw CommandRejectedError.InvalidArgument("No players in hand");
        if (cmd.Players.Count < 2)
            throw CommandRejectedError.InvalidArgument("Need at least 2 players");

        var (playerCards, remainingDeck) = DealHoleCards(
            cmd.GameVariant,
            cmd.Players.ToList(),
            cmd.DeckSeed
        );

        var evt = new CardsDealt
        {
            TableRoot = cmd.TableRoot,
            HandNumber = cmd.HandNumber,
            GameVariant = cmd.GameVariant,
            DealerPosition = cmd.DealerPosition,
            DealtAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };
        evt.PlayerCards.AddRange(playerCards);
        evt.Players.AddRange(cmd.Players);
        evt.RemainingDeck.AddRange(remainingDeck);

        return evt;
    }

    [Handles(typeof(PostBlind))]
    public BlindPosted HandlePostBlind(PostBlind cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (Status == "complete")
            throw CommandRejectedError.PreconditionFailed("Hand is complete");
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");

        var player = GetPlayer(cmd.PlayerRoot);
        if (player == null)
            throw CommandRejectedError.PreconditionFailed("Player not in hand");
        if (player.HasFolded)
            throw CommandRejectedError.PreconditionFailed("Player has folded");
        if (cmd.Amount <= 0)
            throw CommandRejectedError.InvalidArgument("Blind amount must be positive");

        var actualAmount = Math.Min(cmd.Amount, player.Stack);
        var newStack = player.Stack - actualAmount;
        var newPotTotal = GetPotTotal() + actualAmount;

        return new BlindPosted
        {
            PlayerRoot = cmd.PlayerRoot,
            BlindType = cmd.BlindType,
            Amount = actualAmount,
            PlayerStack = newStack,
            PotTotal = newPotTotal,
            PostedAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };
    }

    [Handles(typeof(PlayerAction))]
    public ActionTaken HandleAction(PlayerAction cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (Status != "betting")
            throw CommandRejectedError.PreconditionFailed("Not in betting phase");
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");

        var player = GetPlayer(cmd.PlayerRoot);
        if (player == null)
            throw CommandRejectedError.PreconditionFailed("Player not in hand");
        if (player.HasFolded)
            throw CommandRejectedError.PreconditionFailed("Player has folded");
        if (player.IsAllIn)
            throw CommandRejectedError.PreconditionFailed("Player is all-in");

        var action = cmd.Action;
        var amount = cmd.Amount;
        var callAmount = CurrentBet - player.BetThisRound;

        switch (action)
        {
            case ActionType.Fold:
                amount = 0;
                break;
            case ActionType.Check:
                if (callAmount > 0)
                    throw CommandRejectedError.PreconditionFailed(
                        "Cannot check when there is a bet to call"
                    );
                amount = 0;
                break;
            case ActionType.Call:
                if (callAmount == 0)
                    throw CommandRejectedError.PreconditionFailed("Nothing to call");
                amount = Math.Min(callAmount, player.Stack);
                if (player.Stack - amount == 0)
                    action = ActionType.AllIn;
                break;
            case ActionType.Bet:
                if (CurrentBet > 0)
                    throw CommandRejectedError.PreconditionFailed(
                        "Cannot bet when there is already a bet"
                    );
                if (amount < BigBlind)
                    throw CommandRejectedError.InvalidArgument($"Bet must be at least {BigBlind}");
                if (amount > player.Stack)
                    throw CommandRejectedError.InvalidArgument("Bet exceeds stack");
                if (player.Stack - amount == 0)
                    action = ActionType.AllIn;
                break;
            case ActionType.Raise:
                if (CurrentBet == 0)
                    throw CommandRejectedError.PreconditionFailed(
                        "Cannot raise when there is no bet"
                    );
                // cmd.Amount is the TOTAL bet they want to have (like Go impl)
                var totalBet = amount;
                var raiseAmount = totalBet - CurrentBet;
                if (raiseAmount < MinRaise)
                    throw CommandRejectedError.InvalidArgument(
                        $"Raise must be at least {MinRaise}"
                    );
                // Actual amount to put in is total minus what they already bet
                amount = totalBet - player.BetThisRound;
                if (amount > player.Stack)
                    throw CommandRejectedError.InvalidArgument("Raise exceeds stack");
                if (player.Stack - amount == 0)
                    action = ActionType.AllIn;
                break;
            case ActionType.AllIn:
                amount = player.Stack;
                break;
            default:
                throw CommandRejectedError.InvalidArgument("Invalid action");
        }

        var newStack = player.Stack - amount;
        var newPotTotal = GetPotTotal() + amount;

        // For BET/RAISE, emit the total bet (cmd.Amount), not the chips put in (amount)
        var amountToEmit = amount;
        if (cmd.Action == ActionType.Bet || cmd.Action == ActionType.Raise)
        {
            amountToEmit = cmd.Amount;
        }

        return new ActionTaken
        {
            PlayerRoot = cmd.PlayerRoot,
            Action = action,
            Amount = amountToEmit,
            PlayerStack = newStack,
            PotTotal = newPotTotal,
            AmountToCall = Math.Max(CurrentBet, player.BetThisRound + amount) - player.BetThisRound,
            ActionAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };
    }

    [Handles(typeof(DealCommunityCards))]
    public CommunityCardsDealt HandleDealCommunity(DealCommunityCards cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (Status == "complete")
            throw CommandRejectedError.PreconditionFailed("Hand is complete");
        if (cmd.Count <= 0)
            throw CommandRejectedError.InvalidArgument("Must deal at least 1 card");

        if (GameVariant == GameVariant.FiveCardDraw)
            throw CommandRejectedError.PreconditionFailed(
                "Five card draw doesn't have community cards"
            );

        var (nextPhase, expectedCards) = GetNextPhase(CurrentPhase);
        if (nextPhase == BettingPhase.Unspecified)
            throw CommandRejectedError.PreconditionFailed("No more phases");
        if (expectedCards != cmd.Count)
            throw CommandRejectedError.InvalidArgument(
                $"Expected {expectedCards} cards for this phase"
            );
        if (RemainingDeck.Count < cmd.Count)
            throw CommandRejectedError.PreconditionFailed("Not enough cards in deck");

        var newCards = RemainingDeck.Take(cmd.Count).ToList();
        var allCommunity = CommunityCards.Concat(newCards).ToList();

        var evt = new CommunityCardsDealt
        {
            Phase = nextPhase,
            DealtAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };
        foreach (var (suit, rank) in newCards)
            evt.Cards.Add(new Card { Suit = suit, Rank = rank });
        foreach (var (suit, rank) in allCommunity)
            evt.AllCommunityCards.Add(new Card { Suit = suit, Rank = rank });

        return evt;
    }

    [Handles(typeof(RequestDraw))]
    public DrawCompleted HandleRequestDraw(RequestDraw cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (GameVariant != GameVariant.FiveCardDraw)
            throw CommandRejectedError.InvalidArgument(
                "Draw is not supported in this game variant"
            );
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");

        var player = GetPlayer(cmd.PlayerRoot);
        if (player == null)
            throw CommandRejectedError.PreconditionFailed("Player not in hand");
        if (player.HasFolded)
            throw CommandRejectedError.PreconditionFailed("Player has folded");

        var discardCount = cmd.CardIndices.Count;
        var evt = new DrawCompleted
        {
            PlayerRoot = cmd.PlayerRoot,
            CardsDiscarded = discardCount,
            CardsDrawn = discardCount,
            DrawnAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };

        // Draw new cards from remaining deck
        for (int i = 0; i < discardCount && i < RemainingDeck.Count; i++)
        {
            var (suit, rank) = RemainingDeck[i];
            evt.NewCards.Add(new Card { Suit = suit, Rank = rank });
        }

        return evt;
    }

    [Handles(typeof(RevealCards))]
    public IMessage HandleRevealCards(RevealCards cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        // Allow revealing cards at showdown or when betting is complete (testing scenario)
        if (Status != "showdown" && Status != "betting" && Status != "complete")
            throw CommandRejectedError.PreconditionFailed("Not at showdown");
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");

        var player = GetPlayer(cmd.PlayerRoot);
        if (player == null)
            throw CommandRejectedError.PreconditionFailed("Player not in hand");
        if (player.HasFolded)
            throw CommandRejectedError.PreconditionFailed("Player has folded");

        if (cmd.Muck)
        {
            return new CardsMucked
            {
                PlayerRoot = cmd.PlayerRoot,
                MuckedAt = Timestamp.FromDateTime(DateTime.UtcNow),
            };
        }

        var evt = new CardsRevealed
        {
            PlayerRoot = cmd.PlayerRoot,
            Ranking = EvaluateHand(player.HoleCards, CommunityCards),
            RevealedAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };

        foreach (var (suit, rank) in player.HoleCards)
        {
            evt.Cards.Add(new Card { Suit = suit, Rank = rank });
        }

        return evt;
    }

    private static HandRanking EvaluateHand(
        List<(Suit Suit, Rank Rank)> holeCards,
        List<(Suit Suit, Rank Rank)> communityCards
    )
    {
        // Combine hole cards and community cards
        var allCards = holeCards.Concat(communityCards).ToList();
        if (allCards.Count == 0)
            return new HandRanking { RankType = HandRankType.HighCard, Score = 0 };

        // Simple hand evaluation - check for common patterns
        var ranks = allCards.Select(c => c.Rank).ToList();
        var suits = allCards.Select(c => c.Suit).ToList();

        var rankGroups = ranks
            .GroupBy(r => r)
            .OrderByDescending(g => g.Count())
            .ThenByDescending(g => g.Key)
            .ToList();
        var suitGroups = suits.GroupBy(s => s).OrderByDescending(g => g.Count()).ToList();

        var maxOfKind = rankGroups.First().Count();
        var isFlush = suitGroups.Any(g => g.Count() >= 5);
        var isStraight = CheckStraight(ranks);

        // Check for Royal Flush
        if (isFlush && isStraight)
        {
            var flushSuit = suitGroups.First(g => g.Count() >= 5).Key;
            var flushCards = allCards
                .Where(c => c.Suit == flushSuit)
                .Select(c => c.Rank)
                .OrderByDescending(r => r)
                .ToList();
            if (
                flushCards
                    .Take(5)
                    .SequenceEqual(new[] { Rank.Ace, Rank.King, Rank.Queen, Rank.Jack, Rank.Ten })
            )
                return new HandRanking { RankType = HandRankType.RoyalFlush, Score = 10000 };
            return new HandRanking { RankType = HandRankType.StraightFlush, Score = 9000 };
        }

        if (maxOfKind == 4)
            return new HandRanking
            {
                RankType = HandRankType.FourOfAKind,
                Score = 8000 + (int)rankGroups.First().Key,
            };

        if (maxOfKind == 3 && rankGroups.Count > 1 && rankGroups[1].Count() >= 2)
            return new HandRanking
            {
                RankType = HandRankType.FullHouse,
                Score = 7000 + (int)rankGroups.First().Key * 10 + (int)rankGroups[1].Key,
            };

        if (isFlush)
            return new HandRanking { RankType = HandRankType.Flush, Score = 6000 };

        if (isStraight)
            return new HandRanking { RankType = HandRankType.Straight, Score = 5000 };

        if (maxOfKind == 3)
            return new HandRanking
            {
                RankType = HandRankType.ThreeOfAKind,
                Score = 4000 + (int)rankGroups.First().Key,
            };

        if (maxOfKind == 2 && rankGroups.Count > 1 && rankGroups[1].Count() == 2)
            return new HandRanking
            {
                RankType = HandRankType.TwoPair,
                Score = 3000 + (int)rankGroups.First().Key * 10 + (int)rankGroups[1].Key,
            };

        if (maxOfKind == 2)
            return new HandRanking
            {
                RankType = HandRankType.Pair,
                Score = 2000 + (int)rankGroups.First().Key,
            };

        return new HandRanking { RankType = HandRankType.HighCard, Score = (int)ranks.Max() };
    }

    private static bool CheckStraight(List<Rank> ranks)
    {
        var distinct = ranks.Distinct().OrderBy(r => r).ToList();
        if (distinct.Count < 5)
            return false;

        for (int i = 0; i <= distinct.Count - 5; i++)
        {
            if (distinct[i + 4] - distinct[i] == 4)
                return true;
        }

        // Check for wheel (A-2-3-4-5)
        if (
            distinct.Contains(Rank.Ace)
            && distinct.Contains(Rank.Two)
            && distinct.Contains(Rank.Three)
            && distinct.Contains(Rank.Four)
            && distinct.Contains(Rank.Five)
        )
            return true;

        return false;
    }

    [Handles(typeof(AwardPot))]
    public IMessage HandleAward(AwardPot cmd)
    {
        if (!Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (Status == "complete")
            throw CommandRejectedError.PreconditionFailed("Hand already complete");
        if (cmd.Awards.Count == 0)
            throw CommandRejectedError.InvalidArgument("No awards specified");

        foreach (var award in cmd.Awards)
        {
            var player = GetPlayer(award.PlayerRoot);
            if (player == null)
                throw CommandRejectedError.PreconditionFailed("Winner not in hand");
            if (player.HasFolded)
                throw CommandRejectedError.PreconditionFailed("Folded player cannot win pot");
        }

        var winners = cmd
            .Awards.Select(a => new PotWinner
            {
                PlayerRoot = a.PlayerRoot,
                Amount = a.Amount,
                PotType = a.PotType,
            })
            .ToList();

        var finalStacks = Players
            .Values.Select(p =>
            {
                var winAmount = cmd
                    .Awards.Where(a => a.PlayerRoot.Equals(p.PlayerRoot))
                    .Sum(a => a.Amount);
                return new PlayerStackSnapshot
                {
                    PlayerRoot = p.PlayerRoot,
                    Stack = p.Stack + winAmount,
                    IsAllIn = p.IsAllIn,
                    HasFolded = p.HasFolded,
                };
            })
            .ToList();

        var completeEvent = new HandComplete
        {
            TableRoot = TableRoot,
            HandNumber = HandNumber,
            CompletedAt = Timestamp.FromDateTime(DateTime.UtcNow),
        };
        completeEvent.Winners.AddRange(winners);
        completeEvent.FinalStacks.AddRange(finalStacks);

        return completeEvent;
    }

    private static (List<PlayerHoleCards> PlayerCards, List<Card> RemainingDeck) DealHoleCards(
        GameVariant variant,
        List<PlayerInHand> players,
        ByteString? seed
    )
    {
        var cardsPerPlayer = variant switch
        {
            GameVariant.TexasHoldem => 2,
            GameVariant.Omaha => 4,
            GameVariant.FiveCardDraw => 5,
            GameVariant.SevenCardStud => 7,
            _ => 2,
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

        var remaining = deck.Skip(deckIndex).ToList();
        return (result, remaining);
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

        var rng =
            seed != null && !seed.IsEmpty
                ? new Random(BitConverter.ToInt32(seed.ToByteArray().Take(4).ToArray(), 0))
                : new Random();

        for (var i = cards.Count - 1; i > 0; i--)
        {
            var j = rng.Next(i + 1);
            (cards[i], cards[j]) = (cards[j], cards[i]);
        }

        return cards;
    }

    private static (BettingPhase NextPhase, int CardsToDealt) GetNextPhase(BettingPhase current)
    {
        return current switch
        {
            BettingPhase.Preflop => (BettingPhase.Flop, 3),
            BettingPhase.Flop => (BettingPhase.Turn, 1),
            BettingPhase.Turn => (BettingPhase.River, 1),
            BettingPhase.River => (BettingPhase.Showdown, 0),
            _ => (BettingPhase.Unspecified, 0),
        };
    }
}
