using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Proto.Angzarr;
using Angzarr.Proto.Examples;

namespace Angzarr.Examples.PrjCloudEvents;

/// <summary>
/// CloudEvents projector - publishes player events as CloudEvents.
///
/// This projector transforms internal domain events into CloudEvents 1.0 format
/// for external consumption via HTTP webhooks or Kafka.
/// </summary>

// docs:start:cloudevents_oo
public class PlayerCloudEventsProjector : CloudEventsProjector
{
    public PlayerCloudEventsProjector()
        : base("prj-player-cloudevents", "player") { }

    [Publishes(typeof(PlayerRegistered))]
    public CloudEvent? OnPlayerRegistered(PlayerRegistered @event)
    {
        // Filter sensitive fields, return public version
        var publicEvent = new PublicPlayerRegistered
        {
            DisplayName = @event.DisplayName,
            PlayerType = @event.PlayerType
        };
        return new CloudEvent
        {
            Type = "com.poker.player.registered",
            Data = Any.Pack(publicEvent)
        };
    }

    [Publishes(typeof(FundsDeposited))]
    public CloudEvent? OnFundsDeposited(FundsDeposited @event)
    {
        var publicEvent = new PublicFundsDeposited
        {
            Amount = @event.Amount
        };
        return new CloudEvent
        {
            Type = "com.poker.player.deposited",
            Data = Any.Pack(publicEvent),
            Extensions = { ["priority"] = "normal" }
        };
    }
}
// docs:end:cloudevents_oo

// docs:start:cloudevents_router
public static class PlayerCloudEventsHandlers
{
    public static CloudEvent? HandlePlayerRegistered(PlayerRegistered @event)
    {
        var publicEvent = new PublicPlayerRegistered
        {
            DisplayName = @event.DisplayName,
            PlayerType = @event.PlayerType
        };
        return new CloudEvent
        {
            Type = "com.poker.player.registered",
            Data = Any.Pack(publicEvent)
        };
    }

    public static CloudEvent? HandleFundsDeposited(FundsDeposited @event)
    {
        var publicEvent = new PublicFundsDeposited
        {
            Amount = @event.Amount
        };
        return new CloudEvent
        {
            Type = "com.poker.player.deposited",
            Data = Any.Pack(publicEvent),
            Extensions = { ["priority"] = "normal" }
        };
    }

    public static CloudEventsRouter BuildRouter() =>
        new CloudEventsRouter("prj-player-cloudevents", "player")
            .On<PlayerRegistered>(HandlePlayerRegistered)
            .On<FundsDeposited>(HandleFundsDeposited);
}
// docs:end:cloudevents_router
