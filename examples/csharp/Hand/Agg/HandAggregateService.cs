using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.Agg;

/// <summary>
/// gRPC service for the Hand aggregate.
/// </summary>
public class HandAggregateService : AggregateService.AggregateServiceBase
{
    private readonly CommandRouter _router;

    public HandAggregateService(CommandRouter router)
    {
        _router = router;
    }

    public override Task<ComponentDescriptor> GetDescriptor(GetDescriptorRequest request, ServerCallContext context)
    {
        var descriptor = new ComponentDescriptor
        {
            Name = "hand",
            ComponentType = "aggregate"
        };
        var input = new Target { Domain = "hand" };
        input.Types_.Add("DealCards");
        input.Types_.Add("PostBlind");
        input.Types_.Add("PlayerAction");
        input.Types_.Add("DealCommunityCards");
        input.Types_.Add("RequestDraw");
        input.Types_.Add("RevealCards");
        input.Types_.Add("AwardPot");
        descriptor.Inputs.Add(input);
        return Task.FromResult(descriptor);
    }

    public override Task<BusinessResponse> Handle(ContextualCommand request, ServerCallContext context)
    {
        try
        {
            var commandAny = request.Command?.Pages.FirstOrDefault()?.Command;
            if (commandAny == null)
            {
                return Task.FromResult(new BusinessResponse
                {
                    Revocation = new RevocationResponse { Reason = "No command in request", Abort = true }
                });
            }

            var command = UnpackCommand(commandAny);
            if (command == null)
            {
                return Task.FromResult(new BusinessResponse
                {
                    Revocation = new RevocationResponse { Reason = $"Unknown command type: {commandAny.TypeUrl}", Abort = true }
                });
            }

            var eventMessage = _router.Handle(command, request.Events);

            var eventBook = new EventBook();
            var eventAny = Any.Pack(eventMessage, "type.googleapis.com/");
            eventBook.Pages.Add(new EventPage
            {
                Num = request.Events.NextSequence,
                Event = eventAny
            });

            return Task.FromResult(new BusinessResponse { Events = eventBook });
        }
        catch (CommandRejectedError ex)
        {
            return Task.FromResult(new BusinessResponse
            {
                Revocation = new RevocationResponse { Reason = ex.Message, Abort = true }
            });
        }
        catch (Exception ex)
        {
            return Task.FromResult(new BusinessResponse
            {
                Revocation = new RevocationResponse { Reason = ex.Message, Abort = true }
            });
        }
    }

    public override Task<ReplayResponse> Replay(ReplayRequest request, ServerCallContext context)
    {
        // Build state from events
        var eventBook = new EventBook();
        eventBook.Pages.AddRange(request.Events);
        var state = HandState.FromEventBook(eventBook);

        // Note: No proto state type defined, returning empty response
        // The state is built successfully and could be serialized if needed
        var response = new ReplayResponse();
        return Task.FromResult(response);
    }

    private static IMessage? UnpackCommand(Any commandAny)
    {
        var typeUrl = commandAny.TypeUrl;
        var typeName = typeUrl.Contains('/') ? typeUrl.Split('/').Last() : typeUrl;

        return typeName switch
        {
            "examples.DealCards" => commandAny.Unpack<DealCards>(),
            "examples.PostBlind" => commandAny.Unpack<PostBlind>(),
            "examples.PlayerAction" => commandAny.Unpack<PlayerAction>(),
            "examples.DealCommunityCards" => commandAny.Unpack<DealCommunityCards>(),
            "examples.RequestDraw" => commandAny.Unpack<RequestDraw>(),
            "examples.RevealCards" => commandAny.Unpack<RevealCards>(),
            "examples.AwardPot" => commandAny.Unpack<AwardPot>(),
            _ => null
        };
    }
}
