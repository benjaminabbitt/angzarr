using Google.Protobuf;
using Angzarr.Examples;

namespace PrjOutput;

/// <summary>
/// Renders events as human-readable text.
/// </summary>
public class TextRenderer
{
    private readonly Dictionary<string, string> _playerNames = new();

    public void SetPlayerName(ByteString playerRoot, string name)
    {
        var key = Convert.ToHexString(playerRoot.ToByteArray()).ToLowerInvariant();
        _playerNames[key] = name;
    }

    public string GetPlayerName(ByteString playerRoot)
    {
        var key = Convert.ToHexString(playerRoot.ToByteArray()).ToLowerInvariant();
        return _playerNames.TryGetValue(key, out var name) ? name : key[..8];
    }

    public string Render(string eventType, object evt)
    {
        return eventType switch
        {
            "PlayerRegistered" => RenderPlayerRegistered((PlayerRegistered)evt),
            "FundsDeposited" => RenderFundsDeposited((FundsDeposited)evt),
            "FundsWithdrawn" => RenderFundsWithdrawn((FundsWithdrawn)evt),
            "FundsReserved" => RenderFundsReserved((FundsReserved)evt),
            "FundsReleased" => RenderFundsReleased((FundsReleased)evt),
            "TableCreated" => RenderTableCreated((TableCreated)evt),
            "PlayerJoined" => RenderPlayerJoined((PlayerJoined)evt),
            "PlayerLeft" => RenderPlayerLeft((PlayerLeft)evt),
            "HandStarted" => RenderHandStarted((HandStarted)evt),
            "HandEnded" => RenderHandEnded((HandEnded)evt),
            "CardsDealt" => RenderCardsDealt((CardsDealt)evt),
            "BlindPosted" => RenderBlindPosted((BlindPosted)evt),
            "ActionTaken" => RenderActionTaken((ActionTaken)evt),
            "CommunityCardsDealt" => RenderCommunityCardsDealt((CommunityCardsDealt)evt),
            "PotAwarded" => RenderPotAwarded((PotAwarded)evt),
            "HandComplete" => RenderHandComplete((HandComplete)evt),
            _ => $"[{eventType}]"
        };
    }

    private string RenderPlayerRegistered(PlayerRegistered evt)
    {
        return $"Player '{evt.DisplayName}' registered ({evt.PlayerType})";
    }

    private string RenderFundsDeposited(FundsDeposited evt)
    {
        return $"Deposited {evt.Amount?.Amount} chips (balance: {evt.NewBalance?.Amount})";
    }

    private string RenderFundsWithdrawn(FundsWithdrawn evt)
    {
        return $"Withdrew {evt.Amount?.Amount} chips (balance: {evt.NewBalance?.Amount})";
    }

    private string RenderFundsReserved(FundsReserved evt)
    {
        return $"Reserved {evt.Amount?.Amount} chips for table";
    }

    private string RenderFundsReleased(FundsReleased evt)
    {
        return $"Released {evt.Amount?.Amount} chips from table";
    }

    private string RenderTableCreated(TableCreated evt)
    {
        return $"Table '{evt.TableName}' created ({evt.GameVariant}, {evt.SmallBlind}/{evt.BigBlind})";
    }

    private string RenderPlayerJoined(PlayerJoined evt)
    {
        var player = GetPlayerName(evt.PlayerRoot);
        return $"{player} joined seat {evt.SeatPosition} with {evt.BuyInAmount} chips";
    }

    private string RenderPlayerLeft(PlayerLeft evt)
    {
        var player = GetPlayerName(evt.PlayerRoot);
        return $"{player} left with {evt.ChipsCashedOut} chips";
    }

    private string RenderHandStarted(HandStarted evt)
    {
        return $"=== Hand #{evt.HandNumber} Started (Dealer: seat {evt.DealerPosition}) ===";
    }

    private string RenderHandEnded(HandEnded evt)
    {
        return $"=== Hand Ended ===";
    }

    private string RenderCardsDealt(CardsDealt evt)
    {
        return $"Cards dealt to {evt.PlayerCards.Count} players";
    }

    private string RenderBlindPosted(BlindPosted evt)
    {
        var player = GetPlayerName(evt.PlayerRoot);
        return $"{player} posts {evt.BlindType} blind: {evt.Amount}";
    }

    private string RenderActionTaken(ActionTaken evt)
    {
        var player = GetPlayerName(evt.PlayerRoot);
        var action = evt.Action switch
        {
            ActionType.Fold => "folds",
            ActionType.Check => "checks",
            ActionType.Call => $"calls {evt.Amount}",
            ActionType.Bet => $"bets {evt.Amount}",
            ActionType.Raise => $"raises to {evt.Amount}",
            ActionType.AllIn => $"all-in for {evt.Amount}",
            _ => evt.Action.ToString()
        };
        return $"{player} {action} (pot: {evt.PotTotal})";
    }

    private string RenderCommunityCardsDealt(CommunityCardsDealt evt)
    {
        var cards = string.Join(" ", evt.Cards.Select(RenderCard));
        var phase = evt.Phase.ToString().ToUpper();
        return $"*** {phase}: {cards} ***";
    }

    private string RenderPotAwarded(PotAwarded evt)
    {
        var winners = evt.Winners.Select(w =>
        {
            var player = GetPlayerName(w.PlayerRoot);
            return $"{player} wins {w.Amount}";
        });
        return string.Join(", ", winners);
    }

    private string RenderHandComplete(HandComplete evt)
    {
        return $"Hand #{evt.HandNumber} complete";
    }

    private static string RenderCard(Card card)
    {
        var rank = card.Rank switch
        {
            Rank.Two => "2",
            Rank.Three => "3",
            Rank.Four => "4",
            Rank.Five => "5",
            Rank.Six => "6",
            Rank.Seven => "7",
            Rank.Eight => "8",
            Rank.Nine => "9",
            Rank.Ten => "T",
            Rank.Jack => "J",
            Rank.Queen => "Q",
            Rank.King => "K",
            Rank.Ace => "A",
            _ => "?"
        };
        var suit = card.Suit switch
        {
            Suit.Clubs => "c",
            Suit.Diamonds => "d",
            Suit.Hearts => "h",
            Suit.Spades => "s",
            _ => "?"
        };
        return rank + suit;
    }
}
