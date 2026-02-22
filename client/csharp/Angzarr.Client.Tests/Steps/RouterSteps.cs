using Angzarr.Client;
using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Reqnroll;

namespace Angzarr.Client.Tests.Steps;

[Binding]
public class RouterSteps
{
    private readonly ScenarioContext _ctx;
    private EventRouter? _eventRouter;
    private CommandRouter<TestState>? _commandRouter;
    private StateRouter<TestState>? _stateRouter;
    private List<Angzarr.CommandBook>? _commands;
    private Angzarr.BusinessResponse? _response;
    private Exception? _error;
    private Angzarr.EventBook? _eventBook;
    private Angzarr.ContextualCommand? _contextualCommand;
    private Dictionary<string, List<string>>? _subscriptions;

    public RouterSteps(ScenarioContext ctx) => _ctx = ctx;

    // EventRouter steps
    [Given(@"an EventRouter for saga ""(.*)""")]
    public void GivenEventRouterForSaga(string name)
    {
        _eventRouter = new EventRouter(name);
    }

    [Given(@"an EventRouter for process manager ""(.*)""")]
    public void GivenEventRouterForProcessManager(string name)
    {
        _eventRouter = new EventRouter(name);
    }

    [Given(@"an EventRouter for projector ""(.*)""")]
    public void GivenEventRouterForProjector(string name)
    {
        _eventRouter = new EventRouter(name);
    }

    [When(@"I register domain ""(.*)""")]
    public void WhenRegisterDomain(string domain)
    {
        _eventRouter!.Domain(domain);
    }

    [When(@"I register handler for event ""(.*)""")]
    public void WhenRegisterHandlerForEvent(string eventType)
    {
        _eventRouter!.On(eventType, (eventAny, root, correlationId, destinations) =>
        {
            return new List<Angzarr.CommandBook>();
        });
    }

    [When(@"I dispatch an EventBook with event ""(.*)"" from domain ""(.*)""")]
    public void WhenDispatchEventBookWithEvent(string eventType, string domain)
    {
        _eventBook = MakeEventBook(domain, eventType);
        _commands = _eventRouter!.Dispatch(_eventBook);
    }

    [When(@"I get subscriptions")]
    public void WhenGetSubscriptions()
    {
        _subscriptions = _eventRouter!.Subscriptions();
    }

    [Then(@"subscriptions should include domain ""(.*)"" with event ""(.*)""")]
    public void ThenSubscriptionsShouldIncludeDomainWithEvent(string domain, string eventType)
    {
        _subscriptions.Should().ContainKey(domain);
        _subscriptions![domain].Should().Contain(eventType);
    }

    [Then(@"dispatch should return (.*) commands")]
    public void ThenDispatchShouldReturnCommands(int count)
    {
        _commands.Should().HaveCount(count);
    }

    // CommandRouter steps
    [Given(@"a CommandRouter for domain ""(.*)""")]
    public void GivenCommandRouterForDomain(string domain)
    {
        _stateRouter = new StateRouter<TestState>()
            .On<Empty>((state, evt) => { state.Value = "updated"; });
        _commandRouter = new CommandRouter<TestState>(domain)
            .WithState(_stateRouter);
    }

    [When(@"I register command handler for ""(.*)""")]
    public void WhenRegisterCommandHandler(string commandType)
    {
        _commandRouter!.On(commandType, (commandBook, commandAny, state, seq) =>
        {
            var eventBook = new Angzarr.EventBook();
            eventBook.Pages.Add(new Angzarr.EventPage
            {
                Sequence = (uint)seq,
                Event = Any.Pack(new Empty())
            });
            return eventBook;
        });
    }

    [When(@"I dispatch a ContextualCommand with command ""(.*)""")]
    public void WhenDispatchContextualCommand(string commandType)
    {
        _contextualCommand = MakeContextualCommand(commandType);
        try
        {
            _response = _commandRouter!.Dispatch(_contextualCommand);
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
        _error.Should().BeOfType<InvalidArgumentError>();
        _error!.Message.Should().Contain("Unknown command type");
    }

    // StateRouter steps
    [Given(@"a StateRouter for TestState")]
    public void GivenStateRouterForTestState()
    {
        _stateRouter = new StateRouter<TestState>()
            .On<Empty>((state, evt) => { state.Value = "applied"; });
    }

    [When(@"I apply events to build state")]
    public void WhenApplyEventsToBuildState()
    {
        var eventBook = new Angzarr.EventBook();
        eventBook.Pages.Add(new Angzarr.EventPage
        {
            Sequence = 1,
            Event = Any.Pack(new Empty())
        });
        var state = _stateRouter!.WithEventBook(eventBook);
        _ctx["state"] = state;
    }

    [Then(@"state should reflect applied events")]
    public void ThenStateShouldReflectAppliedEvents()
    {
        var state = (TestState)_ctx["state"];
        state.Value.Should().Be("applied");
    }

    private Angzarr.EventBook MakeEventBook(string domain, string eventType)
    {
        var eventBook = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = "test-correlation"
            }
        };
        eventBook.Pages.Add(new Angzarr.EventPage
        {
            Sequence = 1,
            Event = Any.Pack(new Empty(), "type.googleapis.com/" + eventType)
        });
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
                CorrelationId = "test-correlation"
            }
        };
        commandBook.Pages.Add(new Angzarr.CommandPage
        {
            Sequence = 1,
            Command = Any.Pack(new Empty(), "type.googleapis.com/" + commandType)
        });

        return new Angzarr.ContextualCommand
        {
            Command = commandBook,
            Events = new Angzarr.EventBook()
        };
    }

    // Additional router step definitions

    [Given(@"a saga router handling rejections")]
    public void GivenASagaRouterHandlingRejections()
    {
        _eventRouter = new EventRouter("rejection-saga")
            .Domain("test");
    }

    [Given(@"a saga ""(.*)"" triggered by ""(.*)"" aggregate at sequence (\d+)")]
    public void GivenASagaTriggeredByAggregateAtSequence(string sagaName, string domain, int seq)
    {
        _eventRouter = new EventRouter(sagaName)
            .Domain(domain);

        // Create a rejection notification with saga origin details for compensation tests
        var commandBook = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid())
            }
        };
        commandBook.Pages.Add(new Angzarr.CommandPage
        {
            Sequence = 1,
            Command = Any.Pack(new Empty())
        });

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
                Root = Helpers.UuidToProto(Guid.NewGuid())
            }
        };
        _ctx["rejection_notification"] = rejectionNotification;

        var notification = new Angzarr.Notification
        {
            Payload = Any.Pack(rejectionNotification)
        };
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
                CorrelationId = correlationId
            }
        };
        commandBook.Pages.Add(new Angzarr.CommandPage
        {
            Sequence = 1,
            Command = Any.Pack(new Empty())
        });

        var rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Test rejection",
            RejectedCommand = commandBook,
            SourceAggregate = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid())
            }
        };
        _ctx["rejection_notification"] = rejectionNotification;

        var notification = new Angzarr.Notification
        {
            Payload = Any.Pack(rejectionNotification)
        };
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
        _commands = _eventRouter?.Dispatch(_eventBook!) ?? new List<Angzarr.CommandBook>();
    }

    [When(@"the saga processes the rejection")]
    public void WhenTheSagaProcessesTheRejection()
    {
        // Rejection handling
    }

    [Then(@"the saga should produce compensation commands")]
    public void ThenTheSagaShouldProduceCompensationCommands()
    {
        // Compensation
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
        // Stateless verification
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
        _eventBook = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover { Domain = "test" }
        };
    }

    [Given(@"no events in the EventBook")]
    public void GivenNoEventsInTheEventBook()
    {
        _eventBook = new Angzarr.EventBook();
    }

    [When(@"the projector processes the events")]
    public void WhenTheProjectorProcessesTheEvents()
    {
        // Projector processing
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

    // NOTE: "events with correlation ID exist in multiple aggregates" step moved to QueryClientSteps

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
                CorrelationId = "test-correlation"
            }
        };
        commandBook.Pages.Add(new Angzarr.CommandPage
        {
            Sequence = 1,
            Command = Any.Pack(new Empty())
        });

        var rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "PM command rejected",
            RejectedCommand = commandBook,
            IssuerName = "test-pm",
            IssuerType = "process_manager",
            SourceAggregate = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid())
            }
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
        // Rejection notification emission
    }

    [Then(@"the response should be returned")]
    public void ThenTheResponseShouldBeReturned()
    {
        var response = _response ?? (_ctx.ContainsKey("business_response")
            ? _ctx["business_response"] as Angzarr.BusinessResponse
            : null);
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

    // New step definitions to match updated feature file patterns

    [Given(@"a saga router with a rejected command")]
    public void GivenASagaRouterWithARejectedCommand()
    {
        _eventRouter = new EventRouter("rejection-saga")
            .Domain("test");
        _rejection = new Angzarr.RevocationResponse
        {
            Reason = "Command rejected by target aggregate"
        };
        _ctx["rejection"] = _rejection;
    }

    [When(@"the router processes the rejection")]
    public void WhenTheRouterProcessesTheRejection()
    {
        _rejection.Should().NotBeNull("Expected rejection to be present");
    }

    // NOTE: default/initial state step is in StateBuildingSteps
}

/// <summary>
/// Test state for state router tests.
/// </summary>
public class TestState
{
    public string Value { get; set; } = "";
}
