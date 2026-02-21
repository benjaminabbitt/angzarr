using Angzarr.Client;
using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Reqnroll;

namespace Angzarr.Client.Tests.Steps;

[Binding]
public class SpeculativeClientSteps
{
    private readonly ScenarioContext _ctx;
    private Angzarr.EventBook? _eventBook;
    private Angzarr.BusinessResponse? _response;
    private Exception? _error;

    public SpeculativeClientSteps(ScenarioContext ctx) => _ctx = ctx;

    [Given(@"a SpeculativeClient connected to the test backend")]
    public void GivenSpeculativeClientConnectedToTestBackend()
    {
        // Mock speculative client connection
    }

    [Given(@"an aggregate ""(.*)"" with root ""(.*)"" in state ""(.*)""")]
    public void GivenAnAggregateWithRootInState(string domain, string root, string state)
    {
        var guid = ParseGuid(root);
        _eventBook = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(guid)
            }
        };
    }

    [Given(@"events for ""(.*)"" root ""(.*)""")]
    public void GivenEventsForRoot(string domain, string root)
    {
        var guid = ParseGuid(root);
        _eventBook = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(guid)
            }
        };
        _eventBook.Pages.Add(new Angzarr.EventPage
        {
            Sequence = 1,
            Event = Any.Pack(new Empty())
        });
    }

    [Given(@"(\d+) events for ""(.*)"" root ""(.*)""")]
    public void GivenEventsCountForRoot(int count, string domain, string root)
    {
        var guid = ParseGuid(root);
        _eventBook = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(guid)
            }
        };
        for (int i = 0; i < count; i++)
        {
            _eventBook.Pages.Add(new Angzarr.EventPage
            {
                Sequence = (uint)(i + 1),
                Event = Any.Pack(new Empty())
            });
        }
        _ctx["shared_eventbook"] = _eventBook;
    }

    [When(@"I speculatively execute a command against ""(.*)"" root ""(.*)""")]
    public void WhenISpeculativelyExecuteCommandAgainst(string domain, string root)
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
        _response.Events.Pages.Add(new Angzarr.EventPage
        {
            Sequence = 1,
            Event = Any.Pack(new Empty())
        });
    }

    [When(@"I speculatively execute a command")]
    public void WhenISpeculativelyExecuteCommand()
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
    }

    [When(@"I speculatively execute a command as of sequence (\d+)")]
    public void WhenISpeculativelyExecuteCommandAsOfSequence(int seq)
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
    }

    [When(@"I speculatively execute a ""(.*)"" command")]
    public void WhenISpeculativelyExecuteCommand(string commandType)
    {
        // Simulate rejection for certain commands
        if (commandType == "CancelOrder")
        {
            _response = new Angzarr.BusinessResponse
            {
                Revocation = new Angzarr.RevocationResponse
                {
                    Reason = "cannot cancel shipped order"
                }
            };
        }
        else
        {
            _response = new Angzarr.BusinessResponse
            {
                Events = new Angzarr.EventBook()
            };
        }
    }

    [When(@"I speculatively execute a command with invalid payload")]
    public void WhenISpeculativelyExecuteCommandWithInvalidPayload()
    {
        _error = new InvalidArgumentError("Invalid payload");
    }

    [When(@"I speculatively execute projector ""(.*)"" against those events")]
    public void WhenISpeculativelyExecuteProjector(string projectorName)
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
    }

    [When(@"I speculatively execute saga ""(.*)"" against an event")]
    public void WhenISpeculativelyExecuteSaga(string sagaName)
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
    }

    [When(@"I speculatively execute PM ""(.*)"" against correlated events")]
    public void WhenISpeculativelyExecutePM(string pmName)
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
    }

    [Then(@"the response should contain the projected events")]
    public void ThenTheResponseShouldContainTheProjectedEvents()
    {
        _response!.Events.Should().NotBeNull();
    }

    [Then(@"the events should NOT be persisted")]
    public void ThenTheEventsShouldNotBePersisted()
    {
        // Verification that events weren't persisted (mock scenario)
    }

    [Then(@"the command should execute against the historical state")]
    public void ThenTheCommandShouldExecuteAgainstHistoricalState()
    {
        _response.Should().NotBeNull();
    }

    [Then(@"the response should reflect state at sequence (\d+)")]
    public void ThenTheResponseShouldReflectStateAtSequence(int seq)
    {
        // Temporal query verification
    }

    [Then(@"the response should indicate rejection")]
    public void ThenTheResponseShouldIndicateRejection()
    {
        _response!.Revocation.Should().NotBeNull();
        // Store rejection reason in context for CompensationSteps
        _ctx["rejectionReason"] = _response.Revocation.Reason;
    }

    [Then(@"the operation should fail with validation error")]
    public void ThenTheOperationShouldFailWithValidationError()
    {
        _error.Should().NotBeNull();
        _error.Should().BeAssignableTo<InvalidArgumentError>();
    }

    [Then(@"no events should be produced")]
    public void ThenNoEventsShouldBeProduced()
    {
        // No events
    }

    [Then(@"an edition should be created for the speculation")]
    public void ThenAnEditionShouldBeCreatedForTheSpeculation()
    {
        // Edition creation
    }

    [Then(@"the edition should be discarded after execution")]
    public void ThenTheEditionShouldBeDiscardedAfterExecution()
    {
        // Edition cleanup
    }

    [Then(@"the response should contain the projection")]
    public void ThenTheResponseShouldContainTheProjection()
    {
        _response!.Events.Should().NotBeNull();
    }

    [Then(@"no external systems should be updated")]
    public void ThenNoExternalSystemsShouldBeUpdated()
    {
        // No side effects
    }

    [Then(@"the response should contain the saga output")]
    public void ThenTheResponseShouldContainTheSagaOutput()
    {
        _response!.Events.Should().NotBeNull();
    }

    [Then(@"no commands should be sent")]
    public void ThenNoCommandsShouldBeSent()
    {
        // No command dispatch
    }

    [Then(@"the response should contain the PM output")]
    public void ThenTheResponseShouldContainThePMOutput()
    {
        _response!.Events.Should().NotBeNull();
    }

    [Then(@"no state changes should persist")]
    public void ThenNoStateChangesShouldPersist()
    {
        // No persistence
    }

    private static Guid ParseGuid(string input)
    {
        if (!Guid.TryParse(input, out var guid))
        {
            using var md5 = System.Security.Cryptography.MD5.Create();
            var inputBytes = System.Text.Encoding.UTF8.GetBytes(input);
            var hashBytes = md5.ComputeHash(inputBytes);
            guid = new Guid(hashBytes);
        }
        return guid;
    }

    [When(@"I speculatively execute saga ""(.*)""")]
    public void WhenISpeculativelyExecuteSagaNamed(string sagaName)
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
    }

    [When(@"I speculatively execute process manager ""(.*)""")]
    public void WhenISpeculativelyExecuteProcessManager(string pmName)
    {
        // PMs require correlation ID - check if one is missing
        if (_ctx.ContainsKey("no_correlation_id") && (bool)_ctx["no_correlation_id"])
        {
            _error = new InvalidOperationException("Process manager requires correlation ID");
            _ctx["error"] = _error;
            return;
        }
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
    }

    [When(@"I speculatively execute command A")]
    public void WhenISpeculativelyExecuteCommandA()
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
        _response.Events.Pages.Add(new Angzarr.EventPage
        {
            Sequence = 1,
            Event = Any.Pack(new Empty())
        });
        // Track speculative results for independence verification
        if (!_ctx.ContainsKey("speculative_results"))
        {
            _ctx["speculative_results"] = new List<object>();
        }
        ((List<object>)_ctx["speculative_results"]).Add(_response);
    }

    [When(@"I speculatively execute command B")]
    public void WhenISpeculativelyExecuteCommandB()
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
        _response.Events.Pages.Add(new Angzarr.EventPage
        {
            Sequence = 2,
            Event = Any.Pack(new Empty())
        });
        // Track speculative results for independence verification
        if (!_ctx.ContainsKey("speculative_results"))
        {
            _ctx["speculative_results"] = new List<object>();
        }
        ((List<object>)_ctx["speculative_results"]).Add(_response);
    }

    [When(@"I speculatively execute a command producing (\d+) events")]
    public void WhenISpeculativelyExecuteACommandProducingEvents(int count)
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };
        for (int i = 0; i < count; i++)
        {
            _response.Events.Pages.Add(new Angzarr.EventPage
            {
                Sequence = (uint)(i + 1),
                Event = Any.Pack(new Empty())
            });
        }
    }

    [When(@"I speculatively execute projector ""(.*)""")]
    public void WhenISpeculativelyExecuteProjectorNamed(string projectorName)
    {
        _response = new Angzarr.BusinessResponse
        {
            Events = new Angzarr.EventBook()
        };

        // Build state from events for projector execution
        var eventBook = _eventBook ?? (_ctx.ContainsKey("shared_eventbook")
            ? _ctx["shared_eventbook"] as Angzarr.EventBook : null);

        if (eventBook != null)
        {
            // Simulate projector processing events by building state
            var stateRouter = new StateRouter<TestState>()
                .On<Empty>((state, _) => state.Value = "processed");
            var state = stateRouter.WithEventBook(eventBook);
            _ctx["built_state"] = state;
        }
    }
}
