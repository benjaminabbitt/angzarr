using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.Agg;

/// <summary>
/// State for a player in the hand.
/// </summary>
public class PlayerHandInfo
{
    public ByteString PlayerRoot { get; set; } = ByteString.Empty;
    public int Position { get; set; }
    public List<(Suit Suit, Rank Rank)> HoleCards { get; } = new();
    public long Stack { get; set; }
    public long BetThisRound { get; set; }
    public long TotalInvested { get; set; }
    public bool HasActed { get; set; }
    public bool HasFolded { get; set; }
    public bool IsAllIn { get; set; }
}

/// <summary>
/// State for a pot.
/// </summary>
public class PotInfo
{
    public long Amount { get; set; }
    public List<ByteString> EligiblePlayers { get; } = new();
    public string PotType { get; set; } = "main";
}

/// <summary>
/// Hand aggregate state.
/// </summary>
public class HandState
{
    public string HandId { get; set; } = "";
    public ByteString TableRoot { get; set; } = ByteString.Empty;
    public long HandNumber { get; set; }
    public GameVariant GameVariant { get; set; } = GameVariant.Unspecified;
    public List<(Suit Suit, Rank Rank)> RemainingDeck { get; } = new();
    public Dictionary<int, PlayerHandInfo> Players { get; } = new();
    public List<(Suit Suit, Rank Rank)> CommunityCards { get; } = new();
    public BettingPhase CurrentPhase { get; set; } = BettingPhase.Unspecified;
    public int ActionOnPosition { get; set; } = -1;
    public long CurrentBet { get; set; }
    public long MinRaise { get; set; }
    public List<PotInfo> Pots { get; } = new();
    public int DealerPosition { get; set; }
    public int SmallBlindPosition { get; set; }
    public int BigBlindPosition { get; set; }
    public long SmallBlind { get; set; }
    public long BigBlind { get; set; }
    public string Status { get; set; } = "";

    public bool Exists => !string.IsNullOrEmpty(Status);

    public long GetPotTotal() => Pots.Sum(p => p.Amount);

    public PlayerHandInfo? GetPlayer(ByteString playerRoot)
    {
        return Players.Values.FirstOrDefault(p => p.PlayerRoot.Equals(playerRoot));
    }

    public List<PlayerHandInfo> GetActivePlayers()
    {
        return Players.Values.Where(p => !p.HasFolded && !p.IsAllIn).ToList();
    }

    public List<PlayerHandInfo> GetPlayersInHand()
    {
        return Players.Values.Where(p => !p.HasFolded).ToList();
    }

    /// <summary>
    /// StateRouter for fluent state reconstruction.
    /// </summary>
    public static readonly StateRouter<HandState> Router = new StateRouter<HandState>(() =>
        {
            var s = new HandState();
            s.Pots.Add(new PotInfo { PotType = "main" });
            return s;
        })
        .On<CardsDealt>((state, evt) =>
        {
            state.HandId = $"{Convert.ToHexString(evt.TableRoot.ToByteArray()).ToLowerInvariant()}_{evt.HandNumber}";
            state.TableRoot = evt.TableRoot;
            state.HandNumber = evt.HandNumber;
            state.GameVariant = evt.GameVariant;
            state.DealerPosition = evt.DealerPosition;
            state.Status = "betting";
            state.CurrentPhase = BettingPhase.Preflop;

            foreach (var player in evt.Players)
            {
                state.Players[player.Position] = new PlayerHandInfo
                {
                    PlayerRoot = player.PlayerRoot,
                    Position = player.Position,
                    Stack = player.Stack
                };
            }

            var dealtCards = new HashSet<(Suit, Rank)>();
            foreach (var pc in evt.PlayerCards)
            {
                var playerInfo = state.Players.Values.FirstOrDefault(p => p.PlayerRoot.Equals(pc.PlayerRoot));
                if (playerInfo != null)
                {
                    playerInfo.HoleCards.Clear();
                    foreach (var c in pc.Cards)
                    {
                        playerInfo.HoleCards.Add((c.Suit, c.Rank));
                        dealtCards.Add((c.Suit, c.Rank));
                    }
                }
            }

            // Build remaining deck
            state.RemainingDeck.Clear();
            foreach (Suit suit in new[] { Suit.Clubs, Suit.Diamonds, Suit.Hearts, Suit.Spades })
            {
                for (var rank = Rank.Two; rank <= Rank.Ace; rank++)
                {
                    if (!dealtCards.Contains((suit, rank)))
                        state.RemainingDeck.Add((suit, rank));
                }
            }

            state.Pots.Clear();
            state.Pots.Add(new PotInfo { Amount = 0, PotType = "main" });
            foreach (var p in state.Players.Values)
                state.Pots[0].EligiblePlayers.Add(p.PlayerRoot);
        })
        .On<BlindPosted>((state, evt) =>
        {
            var player = state.GetPlayer(evt.PlayerRoot);
            if (player != null)
            {
                player.Stack = evt.PlayerStack;
                player.BetThisRound = evt.Amount;
                player.TotalInvested += evt.Amount;
                if (evt.BlindType == "small")
                {
                    state.SmallBlindPosition = player.Position;
                    state.SmallBlind = evt.Amount;
                }
                else if (evt.BlindType == "big")
                {
                    state.BigBlindPosition = player.Position;
                    state.BigBlind = evt.Amount;
                    state.CurrentBet = evt.Amount;
                    state.MinRaise = evt.Amount;
                }
            }
            if (state.Pots.Count > 0)
                state.Pots[0].Amount = evt.PotTotal;
            state.Status = "betting";
        })
        .On<ActionTaken>((state, evt) =>
        {
            var player = state.GetPlayer(evt.PlayerRoot);
            if (player != null)
            {
                player.Stack = evt.PlayerStack;
                player.HasActed = true;
                if (evt.Action == ActionType.Fold)
                {
                    player.HasFolded = true;
                }
                else if (evt.Action == ActionType.Call || evt.Action == ActionType.Bet || evt.Action == ActionType.Raise)
                {
                    player.BetThisRound += evt.Amount;
                    player.TotalInvested += evt.Amount;
                }
                else if (evt.Action == ActionType.AllIn)
                {
                    player.IsAllIn = true;
                    player.BetThisRound += evt.Amount;
                    player.TotalInvested += evt.Amount;
                }
                if (evt.Action == ActionType.Bet || evt.Action == ActionType.Raise || evt.Action == ActionType.AllIn)
                {
                    if (player.BetThisRound > state.CurrentBet)
                    {
                        var raiseAmount = player.BetThisRound - state.CurrentBet;
                        state.CurrentBet = player.BetThisRound;
                        state.MinRaise = Math.Max(state.MinRaise, raiseAmount);
                    }
                }
            }
            if (state.Pots.Count > 0)
                state.Pots[0].Amount = evt.PotTotal;
            state.ActionOnPosition = -1;
        })
        .On<CommunityCardsDealt>((state, evt) =>
        {
            foreach (var card in evt.Cards)
            {
                var cardTuple = (card.Suit, card.Rank);
                state.CommunityCards.Add(cardTuple);
                state.RemainingDeck.Remove(cardTuple);
            }
            state.CurrentPhase = evt.Phase;
            state.Status = "betting";
            foreach (var p in state.Players.Values)
            {
                p.BetThisRound = 0;
                p.HasActed = false;
            }
            state.CurrentBet = 0;
        })
        .On<ShowdownStarted>((state, _) =>
        {
            state.Status = "showdown";
        })
        .On<PotAwarded>((state, evt) =>
        {
            foreach (var winner in evt.Winners)
            {
                var player = state.GetPlayer(winner.PlayerRoot);
                if (player != null)
                    player.Stack += winner.Amount;
            }
        })
        .On<HandComplete>((state, _) =>
        {
            state.Status = "complete";
        });

    /// <summary>
    /// Build state from an EventBook by applying all events.
    /// </summary>
    public static HandState FromEventBook(EventBook eventBook)
    {
        return Router.WithEventBook(eventBook);
    }
}
