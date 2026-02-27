using Angzarr;
using Angzarr.Client;
using Angzarr.Client.Router;
using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Xunit;

namespace Angzarr.Client.Tests;

/// <summary>
/// Tests for the unified router implementations that match Rust patterns.
///
/// Verifies:
/// - CommandHandlerRouter (single domain, commands -> events)
/// - SagaRouter (single domain, events -> commands, stateless)
/// - ProcessManagerRouter (multi-domain, events -> commands + PM events)
/// - ProjectorRouter (multi-domain, events -> external output)
/// </summary>
public class UnifiedRouterTests
{
    // =========================================================================
    // Test State and Handler Types
    // =========================================================================

    private class PlayerState
    {
        public bool Exists { get; set; }
        public string PlayerId { get; set; } = "";
        public int Bankroll { get; set; }
    }

    private class PlayerHandler : ICommandHandlerDomainHandler<PlayerState>
    {
        private readonly StateRouter<PlayerState> _stateRouter;

        public PlayerHandler()
        {
            _stateRouter = new StateRouter<PlayerState>().On<Empty>(
                (state, evt) =>
                {
                    state.Exists = true;
                    state.PlayerId = "player_1";
                }
            );
        }

        public IReadOnlyList<string> CommandTypes() => new[] { "RegisterPlayer", "DepositFunds" };

        public StateRouter<PlayerState> StateRouter() => _stateRouter;

        public EventBook Handle(CommandBook cmd, Any payload, PlayerState state, int seq)
        {
            if (payload.TypeUrl.EndsWith("RegisterPlayer"))
            {
                var eventBook = new EventBook { Cover = cmd.Cover };
                eventBook.Pages.Add(
                    new EventPage { Sequence = (uint)seq, Event = Any.Pack(new Empty()) }
                );
                return eventBook;
            }

            if (payload.TypeUrl.EndsWith("DepositFunds"))
            {
                var eventBook = new EventBook { Cover = cmd.Cover };
                eventBook.Pages.Add(
                    new EventPage { Sequence = (uint)seq, Event = Any.Pack(new Empty()) }
                );
                return eventBook;
            }

            throw new CommandRejectedError($"Unknown command: {payload.TypeUrl}");
        }
    }

    private class OrderSagaHandler : ISagaDomainHandler
    {
        public IReadOnlyList<string> EventTypes() => new[] { "OrderCompleted", "OrderCancelled" };

        public IReadOnlyList<Cover> Prepare(EventBook source, Any eventPayload)
        {
            if (eventPayload.TypeUrl.EndsWith("OrderCompleted"))
            {
                return new[]
                {
                    new Cover { Domain = "fulfillment", Root = source.Cover?.Root },
                };
            }
            return Array.Empty<Cover>();
        }

        public SagaHandlerResponse Execute(
            EventBook source,
            Any eventPayload,
            IReadOnlyList<EventBook> destinations
        )
        {
            if (eventPayload.TypeUrl.EndsWith("OrderCompleted"))
            {
                var cmd = new CommandBook
                {
                    Cover = new Cover
                    {
                        Domain = "fulfillment",
                        Root = source.Cover?.Root,
                        CorrelationId = source.Cover?.CorrelationId ?? "",
                    },
                };
                cmd.Pages.Add(
                    new CommandPage
                    {
                        Sequence = 0,
                        Command = Any.Pack(new Empty(), "type.googleapis.com/CreateFulfillment"),
                    }
                );
                return SagaHandlerResponse.WithCommands(new[] { cmd });
            }
            return SagaHandlerResponse.Empty();
        }
    }

    private class HandFlowState
    {
        public string Phase { get; set; } = "init";
        public List<string> ReceivedEvents { get; } = new();
    }

    private class TablePmHandler : IProcessManagerDomainHandler<HandFlowState>
    {
        public IReadOnlyList<string> EventTypes() => new[] { "TableSeated", "PlayerJoined" };

        public IReadOnlyList<Cover> Prepare(
            EventBook trigger,
            HandFlowState state,
            Any eventPayload
        )
        {
            return Array.Empty<Cover>();
        }

        public ProcessManagerResponse Handle(
            EventBook trigger,
            HandFlowState state,
            Any eventPayload,
            IReadOnlyList<EventBook> destinations
        )
        {
            var response = new ProcessManagerResponse();

            if (eventPayload.TypeUrl.EndsWith("TableSeated"))
            {
                // Emit a PM event
                var pmEvents = new EventBook { Cover = new Cover { Domain = "hand-flow" } };
                pmEvents.Pages.Add(
                    new EventPage
                    {
                        Event = Any.Pack(new Empty(), "type.googleapis.com/HandFlowStarted"),
                    }
                );
                response.ProcessEvents = pmEvents;
            }

            return response;
        }
    }

    private class PlayerPmHandler : IProcessManagerDomainHandler<HandFlowState>
    {
        public IReadOnlyList<string> EventTypes() => new[] { "PlayerReady" };

        public IReadOnlyList<Cover> Prepare(
            EventBook trigger,
            HandFlowState state,
            Any eventPayload
        )
        {
            return Array.Empty<Cover>();
        }

        public ProcessManagerResponse Handle(
            EventBook trigger,
            HandFlowState state,
            Any eventPayload,
            IReadOnlyList<EventBook> destinations
        )
        {
            return new ProcessManagerResponse();
        }
    }

    private class PlayerProjectorHandler : IProjectorDomainHandler
    {
        public IReadOnlyList<string> EventTypes() => new[] { "PlayerRegistered", "FundsDeposited" };

        public Projection Project(EventBook events)
        {
            return new Projection { Projector = "player-projector" };
        }
    }

    private class HandProjectorHandler : IProjectorDomainHandler
    {
        public IReadOnlyList<string> EventTypes() => new[] { "CardsDealt", "ActionTaken" };

        public Projection Project(EventBook events)
        {
            return new Projection { Projector = "hand-projector" };
        }
    }

    // =========================================================================
    // CommandHandlerRouter Tests
    // =========================================================================

    [Fact]
    public void CommandHandlerRouter_Creation_SetsNameAndDomain()
    {
        var handler = new PlayerHandler();
        var router = new CommandHandlerRouter<PlayerState, PlayerHandler>(
            "player",
            "player",
            handler
        );

        router.Name.Should().Be("player");
        router.Domain.Should().Be("player");
    }

    [Fact]
    public void CommandHandlerRouter_CommandTypes_ReturnsHandlerTypes()
    {
        var handler = new PlayerHandler();
        var router = new CommandHandlerRouter<PlayerState, PlayerHandler>(
            "player",
            "player",
            handler
        );

        var types = router.CommandTypes();

        types.Should().Contain("RegisterPlayer");
        types.Should().Contain("DepositFunds");
    }

    [Fact]
    public void CommandHandlerRouter_Subscriptions_ReturnsDomainWithTypes()
    {
        var handler = new PlayerHandler();
        var router = new CommandHandlerRouter<PlayerState, PlayerHandler>(
            "player",
            "player",
            handler
        );

        var subs = router.Subscriptions();

        subs.Should().HaveCount(1);
        subs[0].Domain.Should().Be("player");
        subs[0].Types.Should().Contain("RegisterPlayer");
    }

    [Fact]
    public void CommandHandlerRouter_RebuildState_UsesHandler()
    {
        var handler = new PlayerHandler();
        var router = new CommandHandlerRouter<PlayerState, PlayerHandler>(
            "player",
            "player",
            handler
        );

        // Create an event book with one event
        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Event = Any.Pack(new Empty()) });

        var state = router.RebuildState(eventBook);

        state.Exists.Should().BeTrue();
        state.PlayerId.Should().Be("player_1");
    }

    [Fact]
    public void CommandHandlerRouter_Dispatch_ReturnsEvents()
    {
        var handler = new PlayerHandler();
        var router = new CommandHandlerRouter<PlayerState, PlayerHandler>(
            "player",
            "player",
            handler
        );

        var cmd = CreateContextualCommand("player", "RegisterPlayer");

        var response = router.Dispatch(cmd);

        response.Events.Should().NotBeNull();
        response.Events.Pages.Should().HaveCount(1);
    }

    [Fact]
    public void CommandHandlerRouter_Dispatch_Notification_ReturnsRevocation()
    {
        var handler = new PlayerHandler();
        var router = new CommandHandlerRouter<PlayerState, PlayerHandler>(
            "player",
            "player",
            handler
        );

        var notification = CreateNotification("inventory", "ReserveStock", "out of stock");
        var cmd = CreateContextualCommandWithNotification(notification);

        var response = router.Dispatch(cmd);

        // Default handler returns empty response, which becomes revocation
        response.Revocation.Should().NotBeNull();
        response.Revocation.EmitSystemRevocation.Should().BeTrue();
    }

    // =========================================================================
    // SagaRouter Tests
    // =========================================================================

    [Fact]
    public void SagaRouter_Creation_SetsNameAndDomain()
    {
        var handler = new OrderSagaHandler();
        var router = new SagaRouter<OrderSagaHandler>("saga-order-fulfillment", "order", handler);

        router.Name.Should().Be("saga-order-fulfillment");
        router.InputDomain.Should().Be("order");
    }

    [Fact]
    public void SagaRouter_EventTypes_ReturnsHandlerTypes()
    {
        var handler = new OrderSagaHandler();
        var router = new SagaRouter<OrderSagaHandler>("saga-order-fulfillment", "order", handler);

        var types = router.EventTypes();

        types.Should().Contain("OrderCompleted");
        types.Should().Contain("OrderCancelled");
    }

    [Fact]
    public void SagaRouter_Subscriptions_ReturnsDomainWithTypes()
    {
        var handler = new OrderSagaHandler();
        var router = new SagaRouter<OrderSagaHandler>("saga-order-fulfillment", "order", handler);

        var subs = router.Subscriptions();

        subs.Should().HaveCount(1);
        subs[0].Domain.Should().Be("order");
        subs[0].Types.Should().Contain("OrderCompleted");
    }

    [Fact]
    public void SagaRouter_PrepareDestinations_ReturnsCovers()
    {
        var handler = new OrderSagaHandler();
        var router = new SagaRouter<OrderSagaHandler>("saga-order-fulfillment", "order", handler);

        var source = CreateEventBook("order", "OrderCompleted");

        var destinations = router.PrepareDestinations(source);

        destinations.Should().HaveCount(1);
        destinations[0].Domain.Should().Be("fulfillment");
    }

    [Fact]
    public void SagaRouter_PrepareDestinations_NullSource_ReturnsEmpty()
    {
        var handler = new OrderSagaHandler();
        var router = new SagaRouter<OrderSagaHandler>("saga-order-fulfillment", "order", handler);

        var destinations = router.PrepareDestinations(null);

        destinations.Should().BeEmpty();
    }

    [Fact]
    public void SagaRouter_Dispatch_ReturnsCommands()
    {
        var handler = new OrderSagaHandler();
        var router = new SagaRouter<OrderSagaHandler>("saga-order-fulfillment", "order", handler);

        var source = CreateEventBook("order", "OrderCompleted");

        var response = router.Dispatch(source);

        response.Commands.Should().HaveCount(1);
        response.Commands[0].Cover.Domain.Should().Be("fulfillment");
    }

    [Fact]
    public void SagaRouter_Dispatch_EmptySource_Throws()
    {
        var handler = new OrderSagaHandler();
        var router = new SagaRouter<OrderSagaHandler>("saga-order-fulfillment", "order", handler);

        var source = new EventBook();

        var act = () => router.Dispatch(source);

        act.Should().Throw<InvalidArgumentError>().WithMessage("*no events*");
    }

    // =========================================================================
    // ProcessManagerRouter Tests
    // =========================================================================

    [Fact]
    public void ProcessManagerRouter_Creation_SetsNameAndPmDomain()
    {
        var router = new ProcessManagerRouter<HandFlowState>(
            "pmg-hand-flow",
            "hand-flow",
            events => new HandFlowState()
        );

        router.Name.Should().Be("pmg-hand-flow");
        router.PmDomain.Should().Be("hand-flow");
    }

    [Fact]
    public void ProcessManagerRouter_Domain_RegistersMultipleDomains()
    {
        var router = new ProcessManagerRouter<HandFlowState>(
            "pmg-hand-flow",
            "hand-flow",
            events => new HandFlowState()
        )
            .Domain("table", new TablePmHandler())
            .Domain("player", new PlayerPmHandler());

        var subs = router.Subscriptions();

        subs.Should().HaveCount(2);
        subs.Select(s => s.Domain).Should().Contain("table");
        subs.Select(s => s.Domain).Should().Contain("player");
    }

    [Fact]
    public void ProcessManagerRouter_Subscriptions_IncludesAllDomains()
    {
        var router = new ProcessManagerRouter<HandFlowState>(
            "pmg-hand-flow",
            "hand-flow",
            events => new HandFlowState()
        )
            .Domain("table", new TablePmHandler())
            .Domain("player", new PlayerPmHandler());

        var subs = router.Subscriptions();

        var tableSub = subs.FirstOrDefault(s => s.Domain == "table");
        tableSub.Types.Should().Contain("TableSeated");
        tableSub.Types.Should().Contain("PlayerJoined");

        var playerSub = subs.FirstOrDefault(s => s.Domain == "player");
        playerSub.Types.Should().Contain("PlayerReady");
    }

    [Fact]
    public void ProcessManagerRouter_RebuildState_UsesProvidedFunction()
    {
        var router = new ProcessManagerRouter<HandFlowState>(
            "pmg-hand-flow",
            "hand-flow",
            events =>
            {
                var state = new HandFlowState { Phase = "rebuilt" };
                return state;
            }
        );

        var state = router.RebuildState(new EventBook());

        state.Phase.Should().Be("rebuilt");
    }

    [Fact]
    public void ProcessManagerRouter_Dispatch_ReturnsProcessEvents()
    {
        var router = new ProcessManagerRouter<HandFlowState>(
            "pmg-hand-flow",
            "hand-flow",
            events => new HandFlowState()
        ).Domain("table", new TablePmHandler());

        var trigger = CreateEventBook("table", "TableSeated");
        var processState = new EventBook();

        var response = router.Dispatch(trigger, processState);

        response.ProcessEvents.Should().NotBeNull();
        response.ProcessEvents.Pages.Should().HaveCount(1);
    }

    [Fact]
    public void ProcessManagerRouter_Dispatch_UnknownDomain_Throws()
    {
        var router = new ProcessManagerRouter<HandFlowState>(
            "pmg-hand-flow",
            "hand-flow",
            events => new HandFlowState()
        ).Domain("table", new TablePmHandler());

        var trigger = CreateEventBook("unknown", "SomeEvent");
        var processState = new EventBook();

        var act = () => router.Dispatch(trigger, processState);

        act.Should().Throw<InvalidArgumentError>().WithMessage("*No handler for domain*unknown*");
    }

    // =========================================================================
    // ProjectorRouter Tests
    // =========================================================================

    [Fact]
    public void ProjectorRouter_Creation_SetsName()
    {
        var router = new ProjectorRouter("prj-output");

        router.Name.Should().Be("prj-output");
    }

    [Fact]
    public void ProjectorRouter_Domain_RegistersMultipleDomains()
    {
        var router = new ProjectorRouter("prj-output")
            .Domain("player", new PlayerProjectorHandler())
            .Domain("hand", new HandProjectorHandler());

        var subs = router.Subscriptions();

        subs.Should().HaveCount(2);
        subs.Select(s => s.Domain).Should().Contain("player");
        subs.Select(s => s.Domain).Should().Contain("hand");
    }

    [Fact]
    public void ProjectorRouter_Subscriptions_IncludesAllDomains()
    {
        var router = new ProjectorRouter("prj-output")
            .Domain("player", new PlayerProjectorHandler())
            .Domain("hand", new HandProjectorHandler());

        var subs = router.Subscriptions();

        var playerSub = subs.FirstOrDefault(s => s.Domain == "player");
        playerSub.Types.Should().Contain("PlayerRegistered");
        playerSub.Types.Should().Contain("FundsDeposited");

        var handSub = subs.FirstOrDefault(s => s.Domain == "hand");
        handSub.Types.Should().Contain("CardsDealt");
        handSub.Types.Should().Contain("ActionTaken");
    }

    [Fact]
    public void ProjectorRouter_Dispatch_ReturnsProjection()
    {
        var router = new ProjectorRouter("prj-output").Domain(
            "player",
            new PlayerProjectorHandler()
        );

        var events = CreateEventBook("player", "PlayerRegistered");

        var projection = router.Dispatch(events);

        projection.Projector.Should().Be("player-projector");
    }

    [Fact]
    public void ProjectorRouter_Dispatch_UnknownDomain_Throws()
    {
        var router = new ProjectorRouter("prj-output").Domain(
            "player",
            new PlayerProjectorHandler()
        );

        var events = CreateEventBook("unknown", "SomeEvent");

        var act = () => router.Dispatch(events);

        act.Should().Throw<InvalidArgumentError>().WithMessage("*No handler for domain*unknown*");
    }

    // =========================================================================
    // StateRouter Tests (from Angzarr.Client namespace)
    // =========================================================================

    [Fact]
    public void StateRouter_WithEventBook_AppliesEvents()
    {
        var router = new StateRouter<PlayerState>().On<Empty>(
            (state, evt) =>
            {
                state.Exists = true;
                state.Bankroll = 100;
            }
        );

        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Event = Any.Pack(new Empty()) });

        var state = router.WithEventBook(eventBook);

        state.Exists.Should().BeTrue();
        state.Bankroll.Should().Be(100);
    }

    [Fact]
    public void StateRouter_WithEventBook_NullBook_ReturnsDefault()
    {
        var router = new StateRouter<PlayerState>().On<Empty>((state, evt) => state.Exists = true);

        var state = router.WithEventBook(null);

        state.Exists.Should().BeFalse();
        state.Bankroll.Should().Be(0);
    }

    [Fact]
    public void StateRouter_WithEventBook_EmptyBook_ReturnsDefault()
    {
        var router = new StateRouter<PlayerState>().On<Empty>((state, evt) => state.Exists = true);

        var state = router.WithEventBook(new EventBook());

        state.Exists.Should().BeFalse();
    }

    [Fact]
    public void StateRouter_UnknownEventType_IsIgnored()
    {
        var router = new StateRouter<PlayerState>().On<Empty>((state, evt) => state.Exists = true);

        var eventBook = new EventBook();
        // Add an event with a different type URL
        eventBook.Pages.Add(
            new EventPage
            {
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/UnknownEvent",
                    Value = ByteString.Empty,
                },
            }
        );

        var state = router.WithEventBook(eventBook);

        // Unknown events are silently ignored (forward compatibility)
        state.Exists.Should().BeFalse();
    }

    [Fact]
    public void StateRouter_WithFactory_UsesCustomInitialization()
    {
        // Use WithFactory for custom initial state
        var router = StateRouter<PlayerState>
            .WithFactory(() =>
                new PlayerState
                {
                    Bankroll = 100, // Non-default initial value
                    PlayerId = "default_player",
                }
            )
            .On<Empty>((state, evt) => state.Exists = true);

        // With empty event book, should still have factory-initialized values
        var state = router.WithEventBook(new EventBook());

        state.Bankroll.Should().Be(100);
        state.PlayerId.Should().Be("default_player");
        state.Exists.Should().BeFalse(); // No events applied
    }

    [Fact]
    public void StateRouter_WithFactory_AppliesEventsToCustomState()
    {
        var router = StateRouter<PlayerState>
            .WithFactory(() => new PlayerState { Bankroll = 50 })
            .On<Empty>(
                (state, evt) =>
                {
                    state.Exists = true;
                    state.Bankroll += 50; // Add to existing
                }
            );

        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Event = Any.Pack(new Empty()) });

        var state = router.WithEventBook(eventBook);

        state.Bankroll.Should().Be(100); // 50 initial + 50 from event
        state.Exists.Should().BeTrue();
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    private static ContextualCommand CreateContextualCommand(string domain, string commandType)
    {
        var commandBook = new CommandBook
        {
            Cover = new Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = "test-correlation",
            },
        };
        commandBook.Pages.Add(
            new CommandPage
            {
                Sequence = 0,
                Command = new Any
                {
                    TypeUrl = $"type.googleapis.com/{commandType}",
                    Value = new Empty().ToByteString(),
                },
            }
        );

        return new ContextualCommand { Command = commandBook, Events = new EventBook() };
    }

    private static ContextualCommand CreateContextualCommandWithNotification(
        Notification notification
    )
    {
        var commandBook = new CommandBook
        {
            Cover = new Cover { Domain = "player", Root = Helpers.UuidToProto(Guid.NewGuid()) },
        };
        commandBook.Pages.Add(new CommandPage { Command = Any.Pack(notification) });

        return new ContextualCommand { Command = commandBook, Events = new EventBook() };
    }

    private static EventBook CreateEventBook(string domain, string eventType)
    {
        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = "test-correlation",
            },
        };
        eventBook.Pages.Add(
            new EventPage
            {
                Sequence = 1,
                Event = new Any
                {
                    TypeUrl = $"type.googleapis.com/{eventType}",
                    Value = new Empty().ToByteString(),
                },
            }
        );
        return eventBook;
    }

    private static Notification CreateNotification(string domain, string commandType, string reason)
    {
        var rejectedCommand = new CommandBook { Cover = new Cover { Domain = domain } };
        rejectedCommand.Pages.Add(
            new CommandPage { Command = new Any { TypeUrl = $"type.googleapis.com/{commandType}" } }
        );

        var rejection = new RejectionNotification
        {
            IssuerName = "test-saga",
            IssuerType = "saga",
            RejectionReason = reason,
            RejectedCommand = rejectedCommand,
        };

        return new Notification { Payload = Any.Pack(rejection) };
    }
}
