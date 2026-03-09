using Angzarr;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace PrjCloudEvents;

// docs:start:cloudevents_projector
/// <summary>
/// gRPC service for CloudEvents projector.
///
/// Transforms player domain events into CloudEvents format for external consumption.
/// Filters sensitive fields (email, internal IDs) before publishing.
/// </summary>
public class CloudEventsProjectorService : ProjectorService.ProjectorServiceBase
{
    private const string ProjectorName = "prj-player-cloudevents";

    public override Task<Projection> Handle(EventBook request, ServerCallContext context)
    {
        var projection = HandlePlayerEvents(request);
        return Task.FromResult(projection);
    }

    public override Task<Projection> HandleSpeculative(EventBook request, ServerCallContext context)
    {
        var projection = HandlePlayerEvents(request);
        return Task.FromResult(projection);
    }

    private static Projection HandlePlayerEvents(EventBook events)
    {
        if (events?.Cover == null)
        {
            return new Projection();
        }

        var cloudEvents = new List<CloudEvent>();
        uint lastSeq = 0;

        foreach (var page in events.Pages)
        {
            var eventAny = page.Event;
            if (eventAny == null)
                continue;

            lastSeq = page.Header?.Sequence ?? 0;

            var typeUrl = eventAny.TypeUrl;
            var typeName = typeUrl[(typeUrl.LastIndexOf('.') + 1)..];

            var cloudEvent = TransformToCloudEvent(typeName, eventAny);
            if (cloudEvent != null)
            {
                cloudEvents.Add(cloudEvent);
            }
        }

        // Pack CloudEventsResponse into Projection.Projection field
        var ceResponse = new CloudEventsResponse();
        ceResponse.Events.AddRange(cloudEvents);
        var projectionAny = Any.Pack(ceResponse, "type.googleapis.com/");

        return new Projection
        {
            Cover = events.Cover,
            Projector = ProjectorName,
            Sequence = lastSeq,
            Projection_ = projectionAny,
        };
    }

    private static CloudEvent? TransformToCloudEvent(string typeName, Any eventAny)
    {
        switch (typeName)
        {
            case "PlayerRegistered":
                var registered = eventAny.Unpack<PlayerRegistered>();
                // Create public version - filter sensitive fields
                var publicRegistered = new PlayerRegistered
                {
                    DisplayName = registered.DisplayName,
                    PlayerType = registered.PlayerType,
                    // Omit: Email (PII), AiModelId (internal)
                };
                return new CloudEvent
                {
                    Type = "com.poker.player.registered",
                    Data = Any.Pack(publicRegistered, "type.googleapis.com/"),
                };

            case "FundsDeposited":
                var deposited = eventAny.Unpack<FundsDeposited>();
                // Create public version
                var publicDeposited = new FundsDeposited
                {
                    Amount = deposited.Amount,
                    // Omit: NewBalance (sensitive account info)
                };
                var depositEvent = new CloudEvent
                {
                    Type = "com.poker.player.deposited",
                    Data = Any.Pack(publicDeposited, "type.googleapis.com/"),
                };
                depositEvent.Extensions.Add("priority", "normal");
                return depositEvent;

            case "FundsWithdrawn":
                var withdrawn = eventAny.Unpack<FundsWithdrawn>();
                var publicWithdrawn = new FundsWithdrawn { Amount = withdrawn.Amount };
                return new CloudEvent
                {
                    Type = "com.poker.player.withdrawn",
                    Data = Any.Pack(publicWithdrawn, "type.googleapis.com/"),
                };

            default:
                return null;
        }
    }
}
// docs:end:cloudevents_projector
