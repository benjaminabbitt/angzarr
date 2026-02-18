using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.SagaPlayer;

/// <summary>
/// Saga: Hand -> Player
/// Reacts to PotAwarded events from Hand domain.
/// Sends DepositFunds commands to Player domain.
/// </summary>
public static class HandPlayerSaga
{
    public static EventRouter Create()
    {
        return new EventRouter("saga-hand-player", "hand")
            .Sends("player", "DepositFunds")
            .Prepare<PotAwarded>(PreparePotAwarded)
            .On<PotAwarded>(HandlePotAwarded);
    }

    private static List<Cover> PreparePotAwarded(PotAwarded evt)
    {
        return evt.Winners.Select(winner => new Cover
        {
            Domain = "player",
            Root = new UUID { Value = winner.PlayerRoot }
        }).ToList();
    }

    private static object HandlePotAwarded(PotAwarded evt, List<EventBook> destinations)
    {
        var destMap = destinations
            .Where(d => d.Cover?.Root != null)
            .ToDictionary(
                d => Convert.ToHexString(d.Cover.Root.Value.ToByteArray()).ToLowerInvariant(),
                d => d
            );

        var commands = new List<CommandBook>();

        foreach (var winner in evt.Winners)
        {
            var playerKey = Convert.ToHexString(winner.PlayerRoot.ToByteArray()).ToLowerInvariant();
            var destSeq = destMap.TryGetValue(playerKey, out var dest)
                ? EventRouter.NextSequence(dest)
                : 0;

            var depositFunds = new DepositFunds
            {
                Amount = new Currency { Amount = winner.Amount, CurrencyCode = "CHIPS" }
            };

            var cmdAny = EventRouter.PackCommand(depositFunds);

            commands.Add(new CommandBook
            {
                Cover = new Cover
                {
                    Domain = "player",
                    Root = new UUID { Value = winner.PlayerRoot }
                },
                Pages = { new CommandPage { Sequence = destSeq, Command = cmdAny } }
            });
        }

        return commands;
    }
}
