using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Player.Agg.Handlers;

namespace Player.Agg;

/// <summary>
/// gRPC service for the Player aggregate (functional pattern).
///
/// Uses CommandRouter with standalone functional handlers following
/// the guard/validate/compute pattern.
/// </summary>
public class PlayerAggregateService : CommandHandlerService.CommandHandlerServiceBase
{
    private readonly CommandRouter _router;

    public PlayerAggregateService(CommandRouter router)
    {
        _router = router;
    }

    public override Task<BusinessResponse> Handle(
        ContextualCommand request,
        ServerCallContext context
    )
    {
        try
        {
            // Unpack the command from Any
            var commandAny = request.Command?.Pages.FirstOrDefault()?.Command;
            if (commandAny == null)
            {
                return Task.FromResult(
                    new BusinessResponse
                    {
                        Revocation = new RevocationResponse
                        {
                            Reason = "No command in request",
                            Abort = true,
                        },
                    }
                );
            }

            // Check for Notification (rejection/compensation)
            if (commandAny.TypeUrl.EndsWith("Notification"))
            {
                return Task.FromResult(HandleNotification(request, commandAny));
            }

            var command = UnpackCommand(commandAny);
            if (command == null)
            {
                return Task.FromResult(
                    new BusinessResponse
                    {
                        Revocation = new RevocationResponse
                        {
                            Reason = $"Unknown command type: {commandAny.TypeUrl}",
                            Abort = true,
                        },
                    }
                );
            }

            // Handle the command
            var eventMessage = _router.Handle(command, request.Events);

            // Build response
            var eventBook = new EventBook();
            var eventAny = Any.Pack(eventMessage, "type.googleapis.com/");
            eventBook.Pages.Add(
                new EventPage { Sequence = request.Events.NextSequence, Event = eventAny }
            );

            return Task.FromResult(new BusinessResponse { Events = eventBook });
        }
        catch (CommandRejectedError ex)
        {
            return Task.FromResult(
                new BusinessResponse
                {
                    Revocation = new RevocationResponse { Reason = ex.Message, Abort = true },
                }
            );
        }
        catch (Exception ex)
        {
            return Task.FromResult(
                new BusinessResponse
                {
                    Revocation = new RevocationResponse { Reason = ex.Message, Abort = true },
                }
            );
        }
    }

    private BusinessResponse HandleNotification(ContextualCommand request, Any commandAny)
    {
        try
        {
            var notification = commandAny.Unpack<Notification>();

            // Extract target domain and command from rejection
            var targetDomain = "";
            var targetCommand = "";

            if (notification.Payload != null)
            {
                try
                {
                    var rejection = notification.Payload.Unpack<RejectionNotification>();
                    if (rejection.RejectedCommand?.Pages.Count > 0)
                    {
                        targetDomain = rejection.RejectedCommand.Cover?.Domain ?? "";
                        var cmdTypeUrl = rejection.RejectedCommand.Pages[0].Command?.TypeUrl ?? "";
                        targetCommand = cmdTypeUrl.Contains('/')
                            ? cmdTypeUrl.Split('/').Last()
                            : cmdTypeUrl;
                    }
                }
                catch
                {
                    // Malformed rejection notification
                }
            }

            // Handle JoinTable rejection from table domain
            if (targetDomain == "table" && targetCommand.EndsWith("JoinTable"))
            {
                var state = PlayerState.FromEventBook(request.Events ?? new EventBook());
                var seq = request.Events?.NextSequence ?? 0;

                var evt = RejectedHandler.HandleJoinRejected(notification, state);

                var eventBook = new EventBook();
                eventBook.Pages.Add(
                    new EventPage { Sequence = seq, Event = Any.Pack(evt, "type.googleapis.com/") }
                );
                return new BusinessResponse { Events = eventBook };
            }

            // Default: delegate to framework
            return new BusinessResponse
            {
                Revocation = new RevocationResponse
                {
                    EmitSystemRevocation = true,
                    SendToDeadLetterQueue = false,
                    Escalate = false,
                    Abort = false,
                    Reason = $"No handler for rejection {targetDomain}/{targetCommand}",
                },
            };
        }
        catch (Exception ex)
        {
            return new BusinessResponse
            {
                Revocation = new RevocationResponse
                {
                    EmitSystemRevocation = true,
                    Reason = $"Failed to decode notification: {ex.Message}",
                },
            };
        }
    }

    public override Task<ReplayResponse> Replay(ReplayRequest request, ServerCallContext context)
    {
        // Build state from events
        var eventBook = new EventBook();
        eventBook.Pages.AddRange(request.Events);
        var state = PlayerState.FromEventBook(eventBook);

        // Serialize state to proto
        var protoState = new Angzarr.Examples.PlayerState
        {
            PlayerId = state.PlayerId,
            DisplayName = state.DisplayName,
            Email = state.Email,
            PlayerType = state.PlayerType,
            AiModelId = state.AiModelId,
            Bankroll = new Currency { Amount = state.Bankroll, CurrencyCode = "CHIPS" },
            ReservedFunds = new Currency { Amount = state.ReservedFunds, CurrencyCode = "CHIPS" },
            Status = state.Status,
        };
        foreach (var kvp in state.TableReservations)
        {
            protoState.TableReservations[kvp.Key] = kvp.Value;
        }

        var response = new ReplayResponse { State = Any.Pack(protoState, "type.googleapis.com/") };
        return Task.FromResult(response);
    }

    private static IMessage? UnpackCommand(Any commandAny)
    {
        var typeUrl = commandAny.TypeUrl;
        var typeName = typeUrl.Contains('/') ? typeUrl.Split('/').Last() : typeUrl;

        return typeName switch
        {
            "examples.RegisterPlayer" => commandAny.Unpack<RegisterPlayer>(),
            "examples.DepositFunds" => commandAny.Unpack<DepositFunds>(),
            "examples.WithdrawFunds" => commandAny.Unpack<WithdrawFunds>(),
            "examples.ReserveFunds" => commandAny.Unpack<ReserveFunds>(),
            "examples.ReleaseFunds" => commandAny.Unpack<ReleaseFunds>(),
            "examples.TransferFunds" => commandAny.Unpack<TransferFunds>(),
            "examples.RequestAction" => commandAny.Unpack<RequestAction>(),
            _ => null,
        };
    }
}
