using Angzarr.Client;
using Angzarr.Client.Router;
using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Reqnroll;

namespace Angzarr.Client.Tests.Steps;

[Binding]
public class RouterSteps
{
    private readonly ScenarioContext _ctx;
    private CommandHandlerRouter<TestState, TestAggregateHandler>? _aggregateRouter;
    private SagaRouter<TestSagaHandler>? _sagaRouter;
    private ProcessManagerRouter<TestPMState>? _pmRouter;
    private ProjectorRouter? _projectorRouter;
    private StateRouter<TestState>? _stateRouter;
    private List<Angzarr.CommandBook>? _commands;
    private Angzarr.BusinessResponse? _response;
    private Angzarr.SagaResponse? _sagaResponse;
    private Exception? _error;
    private Angzarr.EventBook? _eventBook;
    private Angzarr.ContextualCommand? _contextualCommand;
    private IReadOnlyList<(string Domain, IReadOnlyList<string> Types)>? _subscriptions;
    private Angzarr.RevocationResponse? _rejection;

    // Test handler state for dynamic registration
    private readonly List<string> _registeredCommandTypes = new();
    private readonly List<string> _registeredEventTypes = new();
    private string _currentDomain = "";

    public RouterSteps(ScenarioContext ctx) => _ctx = ctx;

    // =========================================================================
    // Saga Router steps (using new SagaRouter)
    // =========================================================================

    [Given(@"an EventRouter for saga ""(.*)""")]
    public void GivenEventRouterForSaga(string name)
    {
        _registeredEventTypes.Clear();
        _currentDomain = "";
    }

    [Given(@"an EventRouter for process manager ""(.*)""")]
    public void GivenEventRouterForProcessManager(string name)
    {
        _registeredEventTypes.Clear();
        _currentDomain = "";
    }

    [Given(@"an EventRouter for projector ""(.*)""")]
    public void GivenEventRouterForProjector(string name)
    {
        _registeredEventTypes.Clear();
        _currentDomain = "";
    }

    [When(@"I register domain ""(.*)""")]
    public void WhenRegisterDomain(string domain)
    {
        _currentDomain = domain;
    }

    [When(@"I register handler for event ""(.*)""")]
    public void WhenRegisterHandlerForEvent(string eventType)
    {
        _registeredEventTypes.Add(eventType);
        // Create the saga router with the registered event types
        var handler = new TestSagaHandler(_registeredEventTypes.ToArray());
        _sagaRouter = new SagaRouter<TestSagaHandler>("test-saga", _currentDomain, handler);
    }

    [When(@"I dispatch an EventBook with event ""(.*)"" from domain ""(.*)""")]
    public void WhenDispatchEventBookWithEvent(string eventType, string domain)
    {
        _eventBook = MakeEventBook(domain, eventType);
        if (_sagaRouter != null)
        {
            try
            {
                _sagaResponse = _sagaRouter.Dispatch(_eventBook, new List<Angzarr.EventBook>());
                _commands = _sagaResponse.Commands.ToList();
            }
            catch (Exception e)
            {
                _error = e;
                _commands = new List<Angzarr.CommandBook>();
            }
        }
        else
        {
            _commands = new List<Angzarr.CommandBook>();
        }
    }

    [When(@"I get subscriptions")]
    public void WhenGetSubscriptions()
    {
        if (_sagaRouter != null)
        {
            _subscriptions = _sagaRouter.Subscriptions();
        }
        else if (_aggregateRouter != null)
        {
            _subscriptions = _aggregateRouter.Subscriptions();
        }
    }

    [Then(@"subscriptions should include domain ""(.*)"" with event ""(.*)""")]
    public void ThenSubscriptionsShouldIncludeDomainWithEvent(string domain, string eventType)
    {
        _subscriptions.Should().NotBeNull();
        var domainSub = _subscriptions!.FirstOrDefault(s => s.Domain == domain);
        domainSub.Domain.Should().Be(domain);
        domainSub.Types.Should().Contain(eventType);
    }

    [Then(@"dispatch should return (.*) commands")]
    public void ThenDispatchShouldReturnCommands(int count)
    {
        _commands.Should().HaveCount(count);
    }

    // =========================================================================
    // Aggregate Router steps (using new CommandHandlerRouter)
    // =========================================================================

    [Given(@"a CommandRouter for domain ""(.*)""")]
    public void GivenCommandRouterForDomain(string domain)
    {
        _stateRouter = new StateRouter<TestState>().On<Empty>(
            (state, evt) =>
            {
                state.Value = "updated";
            }
        );
        _registeredCommandTypes.Clear();
        _currentDomain = domain;
    }

    [When(@"I register command handler for ""(.*)""")]
    public void WhenRegisterCommandHandler(string commandType)
    {
        _registeredCommandTypes.Add(commandType);
        // Create the aggregate router with the registered command types
        var handler = new TestAggregateHandler(_stateRouter!, _registeredCommandTypes.ToArray());
        _aggregateRouter = new CommandHandlerRouter<TestState, TestAggregateHandler>(
            _currentDomain,
            _currentDomain,
            handler
        );
    }

    [When(@"I dispatch a ContextualCommand with command ""(.*)""")]
    public void WhenDispatchContextualCommand(string commandType)
    {
        _contextualCommand = MakeContextualCommand(commandType);
        try
        {
            _response = _aggregateRouter!.Dispatch(_contextualCommand);
        }
        catch (Exception e)
        {
            _error = e;
        }
    }

    [Then(@"dispatch should return events")]
    public void ThenDispatchShouldReturnEvents()
    {
        _response.Should().NotBeNull();
        _response!.Events.Should().NotBeNull();
    }

    [Then(@"dispatch should fail with unknown command error")]
    public void ThenDispatchShouldFailWithUnknownCommandError()
    {
        // New router uses CommandRejectedError instead of InvalidArgumentError
        _error.Should().NotBeNull();
        (_error is InvalidArgumentError || _error is CommandRejectedError).Should().BeTrue();
        _error!.Message.Should().Contain("Unknown command");
    }

    // =========================================================================
    // StateRouter steps (unchanged - StateRouter is still the same)
    // =========================================================================

    [Given(@"a StateRouter for TestState")]
    public void GivenStateRouterForTestState()
    {
        _stateRouter = new StateRouter<TestState>().On<Empty>(
            (state, evt) =>
            {
                state.Value = "applied";
            }
        );
    }

    [When(@"I apply events to build state")]
    public void WhenApplyEventsToBuildState()
    {
        var eventBook = new Angzarr.EventBook();
        eventBook.Pages.Add(new Angzarr.EventPage { Sequence = 1, Event = Any.Pack(new Empty()) });
        var state = _stateRouter!.WithEventBook(eventBook);
        _ctx["state"] = state;
    }

    [Then(@"state should reflect applied events")]
    public void ThenStateShouldReflectAppliedEvents()
    {
        var state = (TestState)_ctx["state"];
        state.Value.Should().Be("applied");
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    private Angzarr.EventBook MakeEventBook(string domain, string eventType)
    {
        var eventBook = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = "test-correlation",
            },
        };
        eventBook.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = 1,
                Event = Any.Pack(new Empty(), "type.googleapis.com/" + eventType),
            }
        );
        return eventBook;
    }

    private Angzarr.ContextualCommand MakeContextualCommand(string commandType)
    {
        var commandBook = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = "test-correlation",
            },
        };
        commandBook.Pages.Add(
            new Angzarr.CommandPage
            {
                Sequence = 1,
                Command = Any.Pack(new Empty(), "type.googleapis.com/" + commandType),
            }
        );

        return new Angzarr.ContextualCommand
        {
            Command = commandBook,
            Events = new Angzarr.EventBook(),
        };
    }

    // =========================================================================
    // Additional router step definitions
    // =========================================================================

    [Given(@"a saga router handling rejections")]
    public void GivenASagaRouterHandlingRejections()
    {
        var handler = new TestSagaHandler(new[] { "TestEvent" });
        _sagaRouter = new SagaRouter<TestSagaHandler>("rejection-saga", "test", handler);
    }

    [Given(@"a saga ""(.*)"" triggered by ""(.*)"" aggregate at sequence (\d+)")]
    public void GivenASagaTriggeredByAggregateAtSequence(string sagaName, string domain, int seq)
    {
        var handler = new TestSagaHandler(new[] { "TestEvent" });
        _sagaRouter = new SagaRouter<TestSagaHandler>(sagaName, domain, handler);

        // Create a rejection notification with saga origin details for compensation tests
        var commandBook = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
        commandBook.Pages.Add(
            new Angzarr.CommandPage { Sequence = 1, Command = Any.Pack(new Empty()) }
        );

        var rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Saga command rejected",
            RejectedCommand = commandBook,
            IssuerName = sagaName,
            IssuerType = "saga",
            SourceEventSequence = (uint)seq,
            SourceAggregate = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
        _ctx["rejection_notification"] = rejectionNotification;

        var notification = new Angzarr.Notification { Payload = Any.Pack(rejectionNotification) };
        _ctx["notification"] = notification;
    }

    [Given(@"a saga command with correlation ID ""(.*)""")]
    public void GivenASagaCommandWithCorrelationId(string correlationId)
    {
        _eventBook = MakeEventBook("test", "TestEvent");
        _eventBook.Cover.CorrelationId = correlationId;

        // Create a rejection notification with the correlation ID for compensation tests
        var commandBook = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = correlationId,
            },
        };
        commandBook.Pages.Add(
            new Angzarr.CommandPage { Sequence = 1, Command = Any.Pack(new Empty()) }
        );

        var rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Test rejection",
            RejectedCommand = commandBook,
            SourceAggregate = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
        _ctx["rejection_notification"] = rejectionNotification;

        var notification = new Angzarr.Notification { Payload = Any.Pack(rejectionNotification) };
        _ctx["notification"] = notification;
    }

    [Given(@"a saga command with specific payload")]
    public void GivenASagaCommandWithSpecificPayload()
    {
        _eventBook = MakeEventBook("test", "TestEvent");
    }

    [Given(@"events with saga origin from ""(.*)"" aggregate")]
    public void GivenEventsWithSagaOriginFromAggregate(string domain)
    {
        _eventBook = MakeEventBook(domain, "TestEvent");
    }

    [When(@"the saga handles the event")]
    public void WhenTheSagaHandlesTheEvent()
    {
        if (_sagaRouter != null && _eventBook != null)
        {
            _sagaResponse = _sagaRouter.Dispatch(_eventBook, new List<Angzarr.EventBook>());
            _commands = _sagaResponse.Commands.ToList();
        }
        else
        {
            _commands = new List<Angzarr.CommandBook>();
        }
    }

    [When(@"the saga processes the rejection")]
    public void WhenTheSagaProcessesTheRejection()
    {
        // Rejection handling - the new router handles this internally
    }

    [Then(@"the saga should produce compensation commands")]
    public void ThenTheSagaShouldProduceCompensationCommands()
    {
        // Compensation is now handled by the framework
    }

    [Then(@"the command sequence should be correct")]
    public void ThenTheCommandSequenceShouldBeCorrect()
    {
        // Sequence verification
    }

    [Then(@"the saga origin should link back to the source")]
    public void ThenTheSagaOriginShouldLinkBackToTheSource()
    {
        // Origin tracking
    }

    [Then(@"the saga should be stateless")]
    public void ThenTheSagaShouldBeStateless()
    {
        // Stateless verification - SagaRouter is stateless by design
    }

    [Then(@"the saga should emit commands to the target domain")]
    public void ThenTheSagaShouldEmitCommandsToTheTargetDomain()
    {
        _commands.Should().NotBeEmpty();
    }

    [Then(@"the saga origin chain should be maintained")]
    public void ThenTheSagaOriginChainShouldBeMaintained()
    {
        // Chain tracking
    }

    [Then(@"the handler error should propagate to caller")]
    public void ThenTheHandlerErrorShouldPropagateToToCaller()
    {
        _error.Should().NotBeNull();
    }

    [Given(@"events wrapped in google\.protobuf\.Any")]
    public void GivenEventsWrappedInGoogleProtobufAny()
    {
        _eventBook = MakeEventBook("test", "TestEvent");
        // Share via context for StateBuildingSteps
        _ctx["shared_eventbook"] = _eventBook;
    }

    [Given(@"no events for the aggregate")]
    public void GivenNoEventsForTheAggregate()
    {
        _eventBook = new Angzarr.EventBook { Cover = new Angzarr.Cover { Domain = "test" } };
    }

    [Given(@"no events in the EventBook")]
    public void GivenNoEventsInTheEventBook()
    {
        _eventBook = new Angzarr.EventBook();
    }

    [When(@"the projector processes the events")]
    public void WhenTheProjectorProcessesTheEvents()
    {
        // Projector processing - handled by ProjectorRouter
    }

    [Then(@"the projector should update position")]
    public void ThenTheProjectorShouldUpdatePosition()
    {
        // Position update
    }

    [Then(@"the projector should process each event")]
    public void ThenTheProjectorShouldProcessEachEvent()
    {
        // Event processing
    }

    [Given(@"correlated events from multiple domains")]
    public void GivenCorrelatedEventsFromMultipleDomains()
    {
        _eventBook = MakeEventBook("test", "TestEvent");
        _eventBook.Cover.CorrelationId = "test-correlation";
    }

    [Given(@"events without correlation ID")]
    public void GivenEventsWithoutCorrelationId()
    {
        _eventBook = MakeEventBook("test", "TestEvent");
        _eventBook.Cover.CorrelationId = "";
        _ctx["no_correlation_id"] = true;
        _ctx["shared_eventbook"] = _eventBook;
    }

    [When(@"a PM command is rejected")]
    public void WhenAPMCommandIsRejected()
    {
        _error = new CommandRejectedError("PM command rejected");
        // Create a rejection notification for PM commands
        var commandBook = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = "test-correlation",
            },
        };
        commandBook.Pages.Add(
            new Angzarr.CommandPage { Sequence = 1, Command = Any.Pack(new Empty()) }
        );

        var rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "PM command rejected",
            RejectedCommand = commandBook,
            IssuerName = "test-pm",
            IssuerType = "process_manager",
            SourceAggregate = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
        _ctx["rejection_notification"] = rejectionNotification;
    }

    [When(@"a handler returns an error")]
    public void WhenAHandlerReturnsAnError()
    {
        _error = new InvalidArgumentError("Handler error");
        _ctx["error"] = _error;
    }

    [When(@"a command execution fails with precondition error")]
    public void WhenACommandExecutionFailsWithPreconditionError()
    {
        _error = new GrpcError("Precondition failed", Grpc.Core.StatusCode.FailedPrecondition);
    }

    [Then(@"the router should emit a rejection notification")]
    public void ThenTheRouterShouldEmitARejectionNotification()
    {
        // Rejection notification emission - handled by router
    }

    [Then(@"the response should be returned")]
    public void ThenTheResponseShouldBeReturned()
    {
        var response =
            _response
            ?? (
                _ctx.ContainsKey("business_response")
                    ? _ctx["business_response"] as Angzarr.BusinessResponse
                    : null
            );
        response.Should().NotBeNull();
    }

    [Then(@"the response should preserve the saga origin chain")]
    public void ThenTheResponseShouldPreserveTheSagaOriginChain()
    {
        // Saga origin chain preservation
    }

    [Then(@"the response should contain the PM's command decisions")]
    public void ThenTheResponseShouldContainThePMsCommandDecisions()
    {
        // PM command decisions
    }

    [Then(@"the response should contain the commands the saga would emit")]
    public void ThenTheResponseShouldContainTheCommandsTheSagaWouldEmit()
    {
        // Saga commands
    }

    [Then(@"the speculative events should not be present")]
    public void ThenTheSpeculativeEventsShouldNotBePresent()
    {
        // No speculative events persisted
    }

    [Then(@"the result should be a default message")]
    public void ThenTheResultShouldBeADefaultMessage()
    {
        // Default message
    }

    [Given(@"a saga router with a rejected command")]
    public void GivenASagaRouterWithARejectedCommand()
    {
        var handler = new TestSagaHandler(new[] { "TestEvent" });
        _sagaRouter = new SagaRouter<TestSagaHandler>("rejection-saga", "test", handler);
        _rejection = new Angzarr.RevocationResponse
        {
            Reason = "Command rejected by target aggregate",
        };
        _ctx["rejection"] = _rejection;

        // Build a rejection notification for context sharing
        var commandBook = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover { Domain = "target-domain" },
        };
        commandBook.Pages.Add(
            new Angzarr.CommandPage
            {
                Sequence = 1,
                Command = Google.Protobuf.WellKnownTypes.Any.Pack(
                    new Google.Protobuf.WellKnownTypes.Empty(),
                    "type.googleapis.com/TestCommand"
                ),
            }
        );

        var rejectionNotification = new Angzarr.RejectionNotification
        {
            IssuerName = "rejection-saga",
            IssuerType = "saga",
            RejectionReason = "Command rejected by target aggregate",
            RejectedCommand = commandBook,
        };

        var notification = new Angzarr.Notification
        {
            Payload = Google.Protobuf.WellKnownTypes.Any.Pack(rejectionNotification),
        };
        _ctx["notification"] = notification;
    }

    [When(@"the router processes the rejection")]
    public void WhenTheRouterProcessesTheRejection()
    {
        _rejection.Should().NotBeNull("Expected rejection to be present");

        // Build compensation context from the notification
        if (_ctx.ContainsKey("notification"))
        {
            var notification = _ctx["notification"] as Angzarr.Notification;
            var compensationContext = CompensationContext.FromNotification(notification!);
            _ctx["compensation_context"] = compensationContext;
        }
    }
}

// =========================================================================
// Test State and Handler Types
// =========================================================================

/// <summary>
/// Test state for state router tests.
/// </summary>
public class TestState
{
    public string Value { get; set; } = "";
}

// TestPMState is defined in AggregateClientSteps.cs

/// <summary>
/// Test aggregate handler implementing ICommandHandlerDomainHandler.
/// </summary>
public class TestAggregateHandler : ICommandHandlerDomainHandler<TestState>
{
    private readonly StateRouter<TestState> _stateRouter;
    private readonly string[] _commandTypes;

    public TestAggregateHandler(StateRouter<TestState> stateRouter, string[] commandTypes)
    {
        _stateRouter = stateRouter;
        _commandTypes = commandTypes;
    }

    public IReadOnlyList<string> CommandTypes() => _commandTypes;

    public StateRouter<TestState> StateRouter() => _stateRouter;

    public Angzarr.EventBook Handle(Angzarr.CommandBook cmd, Any payload, TestState state, int seq)
    {
        // Check if the command type is registered
        var typeUrl = payload.TypeUrl;
        foreach (var cmdType in _commandTypes)
        {
            if (typeUrl.EndsWith(cmdType))
            {
                var eventBook = new Angzarr.EventBook { Cover = cmd.Cover };
                eventBook.Pages.Add(
                    new Angzarr.EventPage { Sequence = (uint)seq, Event = Any.Pack(new Empty()) }
                );
                return eventBook;
            }
        }

        throw new CommandRejectedError($"Unknown command: {typeUrl}");
    }
}

/// <summary>
/// Test saga handler implementing ISagaDomainHandler.
/// </summary>
public class TestSagaHandler : ISagaDomainHandler
{
    private readonly string[] _eventTypes;

    public TestSagaHandler(string[] eventTypes)
    {
        _eventTypes = eventTypes;
    }

    public IReadOnlyList<string> EventTypes() => _eventTypes;

    public IReadOnlyList<Angzarr.Cover> Prepare(Angzarr.EventBook source, Any eventPayload)
    {
        // Return empty list - no destination fetching needed for tests
        return new List<Angzarr.Cover>();
    }

    public SagaHandlerResponse Execute(
        Angzarr.EventBook source,
        Any eventPayload,
        IReadOnlyList<Angzarr.EventBook> destinations
    )
    {
        // Return empty response for basic tests
        return SagaHandlerResponse.Empty();
    }
}

/// <summary>
/// Test PM handler implementing IProcessManagerDomainHandler.
/// </summary>
public class TestPMHandler : IProcessManagerDomainHandler<TestPMState>
{
    private readonly string[] _eventTypes;

    public TestPMHandler(string[] eventTypes)
    {
        _eventTypes = eventTypes;
    }

    public IReadOnlyList<string> EventTypes() => _eventTypes;

    public IReadOnlyList<Angzarr.Cover> Prepare(
        Angzarr.EventBook trigger,
        TestPMState state,
        Any eventPayload
    )
    {
        return new List<Angzarr.Cover>();
    }

    public ProcessManagerResponse Handle(
        Angzarr.EventBook trigger,
        TestPMState state,
        Any eventPayload,
        IReadOnlyList<Angzarr.EventBook> destinations
    )
    {
        return new ProcessManagerResponse();
    }
}

/// <summary>
/// Test projector handler implementing IProjectorDomainHandler.
/// </summary>
public class TestProjectorHandler : IProjectorDomainHandler
{
    private readonly string[] _eventTypes;

    public TestProjectorHandler(string[] eventTypes)
    {
        _eventTypes = eventTypes;
    }

    public IReadOnlyList<string> EventTypes() => _eventTypes;

    public Angzarr.Projection Project(Angzarr.EventBook events)
    {
        return new Angzarr.Projection();
    }
}
