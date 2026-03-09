using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Hand.SagaPlayer;

/// <summary>
/// Saga: Hand -> Player
/// Reacts to PotAwarded events from Hand domain.
/// Sends DepositFunds commands to Player domain.
/// Sagas are stateless translators - framework handles sequence stamping.
/// </summary>
public static class HandPlayerSaga
{
    public static EventRouter Create()
    {
        return new EventRouter("saga-hand-player").Domain("hand").On<PotAwarded>(HandlePotAwarded);
    }

    private static object HandlePotAwarded(PotAwarded evt, List<EventBook> destinations)
    {
        // Sagas are stateless - destinations not used, framework stamps sequences
        var commands = new List<CommandBook>();

        foreach (var winner in evt.Winners)
        {
            var depositFunds = new DepositFunds
            {
                Amount = new Currency { Amount = winner.Amount, CurrencyCode = "CHIPS" },
            };

            var cmdAny = EventRouter.PackCommand(depositFunds);

            commands.Add(
                new CommandBook
                {
                    Cover = new Cover
                    {
                        Domain = "player",
                        Root = new UUID { Value = winner.PlayerRoot },
                    },
                    Pages =
                    {
                        new CommandPage
                        {
                            Header = new PageHeader
                            {
                                AngzarrDeferred = new AngzarrDeferredSequence(),
                            },
                            Command = cmdAny,
                        },
                    },
                }
            );
        }

        return commands;
    }
}
