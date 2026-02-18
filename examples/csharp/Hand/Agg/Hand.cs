using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.Agg;

/// <summary>
/// Hand aggregate - OO style with decorator-based command dispatch.
/// </summary>
public class HandAggregate : Aggregate<HandState>
{
    public const string Domain = "hand";

    protected override void ApplyEvent(HandState state, Any eventAny)
    {
        HandState.Router.ApplySingle(state, eventAny);
    }

    // --- State accessors ---

    public bool Exists => State.Exists;
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
            PostedAt = Timestamp.FromDateTime(DateTime.UtcNow)
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
                    throw CommandRejectedError.PreconditionFailed("Cannot check when there is a bet to call");
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
                    throw CommandRejectedError.PreconditionFailed("Cannot bet when there is already a bet");
                if (amount < BigBlind)
                    throw CommandRejectedError.InvalidArgument($"Bet must be at least {BigBlind}");
                if (amount > player.Stack)
                    throw CommandRejectedError.InvalidArgument("Bet exceeds stack");
                if (player.Stack - amount == 0)
                    action = ActionType.AllIn;
                break;
            case ActionType.Raise:
                if (CurrentBet == 0)
                    throw CommandRejectedError.PreconditionFailed("Cannot raise when there is no bet");
                var totalBet = player.BetThisRound + amount;
                var raiseAmount = totalBet - CurrentBet;
                if (raiseAmount < MinRaise && amount < player.Stack)
                    throw CommandRejectedError.InvalidArgument($"Raise must be at least {MinRaise}");
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

        return new ActionTaken
        {
            PlayerRoot = cmd.PlayerRoot,
            Action = action,
            Amount = amount,
            PlayerStack = newStack,
            PotTotal = newPotTotal,
            AmountToCall = Math.Max(CurrentBet, player.BetThisRound + amount) - player.BetThisRound,
            ActionAt = Timestamp.FromDateTime(DateTime.UtcNow)
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
            throw CommandRejectedError.PreconditionFailed("Five card draw doesn't have community cards");

        var (nextPhase, expectedCards) = GetNextPhase(CurrentPhase);
        if (nextPhase == BettingPhase.Unspecified)
            throw CommandRejectedError.PreconditionFailed("No more phases");
        if (expectedCards != cmd.Count)
            throw CommandRejectedError.InvalidArgument($"Expected {expectedCards} cards for this phase");
        if (RemainingDeck.Count < cmd.Count)
            throw CommandRejectedError.PreconditionFailed("Not enough cards in deck");

        var newCards = RemainingDeck.Take(cmd.Count).ToList();
        var allCommunity = CommunityCards.Concat(newCards).ToList();

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

        var winners = cmd.Awards.Select(a => new PotWinner
        {
            PlayerRoot = a.PlayerRoot,
            Amount = a.Amount,
            PotType = a.PotType
        }).ToList();

        var finalStacks = Players.Values.Select(p =>
        {
            var winAmount = cmd.Awards.Where(a => a.PlayerRoot.Equals(p.PlayerRoot)).Sum(a => a.Amount);
            return new PlayerStackSnapshot
            {
                PlayerRoot = p.PlayerRoot,
                Stack = p.Stack + winAmount,
                IsAllIn = p.IsAllIn,
                HasFolded = p.HasFolded
            };
        }).ToList();

        var completeEvent = new HandComplete
        {
            TableRoot = TableRoot,
            HandNumber = HandNumber,
            CompletedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        completeEvent.Winners.AddRange(winners);
        completeEvent.FinalStacks.AddRange(finalStacks);

        return completeEvent;
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
