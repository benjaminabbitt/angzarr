using System.Collections.Generic;
using Angzarr.Client;
using Angzarr.Proto.Angzarr;
using Angzarr.Proto.Examples;

namespace Angzarr.Examples.PrjOutput;

/// <summary>
/// Output projector - renders poker events as human-readable text.
///
/// Demonstrates both OO-style (class-based) and StateRouter (functional) patterns.
/// </summary>

// docs:start:projector_oo
public class OutputProjector
{
    private readonly Dictionary<string, string> _playerNames = new();

    [Projects(typeof(PlayerRegistered))]
    public void HandlePlayerRegistered(PlayerRegistered @event)
    {
        _playerNames[@event.PlayerId] = @event.DisplayName;
        Console.WriteLine($"[Player] {@event.DisplayName} registered");
    }

    [Projects(typeof(FundsDeposited))]
    public void HandleFundsDeposited(FundsDeposited @event)
    {
        var name = _playerNames.GetValueOrDefault(@event.PlayerId, @event.PlayerId);
        var amount = @event.Amount?.Amount ?? 0;
        Console.WriteLine($"[Player] {name} deposited ${amount / 100.0:F2}");
    }

    [Projects(typeof(CardsDealt))]
    public void HandleCardsDealt(CardsDealt @event)
    {
        foreach (var player in @event.PlayerCards)
        {
            var name = _playerNames.GetValueOrDefault(player.PlayerId, player.PlayerId);
            var cards = FormatCards(player.HoleCards);
            Console.WriteLine($"[Hand] {name} dealt {cards}");
        }
    }

    private static string FormatCards(IEnumerable<Card> cards) =>
        string.Join(" ", cards.Select(c => $"{c.Rank}{c.Suit}"));
}
// docs:end:projector_oo

// docs:start:state_router
public static class OutputProjectorRouter
{
    public static StateRouter BuildRouter()
    {
        var playerNames = new Dictionary<string, string>();

        return new StateRouter("prj-output")
            .Subscribes("player", new[] { "PlayerRegistered", "FundsDeposited" })
            .Subscribes("hand", new[] { "CardsDealt", "ActionTaken", "PotAwarded" })
            .On<PlayerRegistered>(evt => {
                playerNames[evt.PlayerId] = evt.DisplayName;
                Console.WriteLine($"[Player] {evt.DisplayName} registered");
            })
            .On<FundsDeposited>(evt => {
                var name = playerNames.GetValueOrDefault(evt.PlayerId, evt.PlayerId);
                Console.WriteLine($"[Player] {name} deposited ${evt.Amount?.Amount / 100.0:F2}");
            })
            .On<CardsDealt>(evt => {
                foreach (var player in evt.PlayerCards)
                {
                    var name = playerNames.GetValueOrDefault(player.PlayerId, player.PlayerId);
                    Console.WriteLine($"[Hand] {name} dealt cards");
                }
            });
    }
}
// docs:end:state_router
