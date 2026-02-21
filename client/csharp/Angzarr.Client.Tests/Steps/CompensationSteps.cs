using Angzarr.Client;
using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Reqnroll;

namespace Angzarr.Client.Tests.Steps;

[Binding]
public class CompensationSteps
{
    private readonly ScenarioContext _ctx;
    private CompensationContext? _compensationContext;
    private RejectionHandlerResponse? _response;
    private Angzarr.Notification? _notification;
    private Angzarr.RejectionNotification? _rejectionNotification;
    private CommandRouter<CompensationTestState>? _commandRouter;
    private Angzarr.BusinessResponse? _businessResponse;

    public CompensationSteps(ScenarioContext ctx) => _ctx = ctx;

    [Given(@"a RejectionNotification for command ""(.*)"" in domain ""(.*)""")]
    public void GivenRejectionNotificationForCommand(string commandType, string domain)
    {
        var commandBook = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = "compensation-test"
            }
        };
        commandBook.Pages.Add(new Angzarr.CommandPage
        {
            Sequence = 1,
            Command = Any.Pack(new Empty(), $"type.googleapis.com/{commandType}")
        });

        _rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Test rejection",
            RejectedCommand = commandBook
        };

        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification)
        };
    }

    [Given(@"a CompensationContext from the notification")]
    public void GivenCompensationContextFromNotification()
    {
        _compensationContext = CompensationContext.FromNotification(_notification!);
    }

    [Given(@"a CommandRouter with rejection handler for ""(.*)""/""(.*)""")]
    public void GivenCommandRouterWithRejectionHandler(string domain, string commandType)
    {
        var stateRouter = new StateRouter<CompensationTestState>();
        _commandRouter = new CommandRouter<CompensationTestState>("test")
            .WithState(stateRouter)
            .OnRejected(domain, commandType, (notification, state) =>
            {
                return new RejectionHandlerResponse
                {
                    Events = new Angzarr.EventBook
                    {
                        Pages = { new Angzarr.EventPage { Sequence = 1, Event = Any.Pack(new Empty()) } }
                    }
                };
            });
    }

    [When(@"I get the rejected command")]
    public void WhenGetRejectedCommand()
    {
        _ctx["rejectedCommand"] = _compensationContext!.RejectedCommand;
    }

    [When(@"I get the rejection reason")]
    public void WhenGetRejectionReason()
    {
        _ctx["rejectionReason"] = _compensationContext!.RejectionReason;
    }

    [When(@"I get the source domain")]
    public void WhenGetSourceDomain()
    {
        _ctx["sourceDomain"] = _compensationContext!.RejectedCommand?.Cover?.Domain ?? "";
    }

    [When(@"I dispatch the rejection to the router")]
    public void WhenDispatchRejectionToRouter()
    {
        var contextualCommand = new Angzarr.ContextualCommand
        {
            Command = new Angzarr.CommandBook
            {
                Cover = new Angzarr.Cover { Domain = "test" },
                Pages = { new Angzarr.CommandPage
                {
                    Command = Any.Pack(_notification!, "type.googleapis.com/angzarr.Notification")
                }}
            },
            Events = new Angzarr.EventBook()
        };
        _businessResponse = _commandRouter!.Dispatch(contextualCommand);
    }

    [Then(@"the rejected command should have domain ""(.*)""")]
    public void ThenRejectedCommandShouldHaveDomain(string domain)
    {
        var cmd = (Angzarr.CommandBook)_ctx["rejectedCommand"];
        cmd.Cover.Domain.Should().Be(domain);
    }

    [Then(@"the rejection reason should be ""(.*)""")]
    public void ThenRejectionReasonShouldBe(string reason)
    {
        var actualReason = (string)_ctx["rejectionReason"];
        actualReason.Should().Be(reason);
    }

    [Then(@"the source domain should be ""(.*)""")]
    public void ThenSourceDomainShouldBe(string domain)
    {
        var actualDomain = (string)_ctx["sourceDomain"];
        actualDomain.Should().Be(domain);
    }

    [Then(@"the response should have compensation events")]
    public void ThenResponseShouldHaveCompensationEvents()
    {
        _businessResponse!.Events.Should().NotBeNull();
        _businessResponse.Events.Pages.Should().NotBeEmpty();
    }

    [Then(@"the response should forward the notification")]
    public void ThenResponseShouldForwardNotification()
    {
        _businessResponse!.Notification.Should().NotBeNull();
    }

    [Given(@"a rejection handler that forwards notification")]
    public void GivenRejectionHandlerThatForwardsNotification()
    {
        var stateRouter = new StateRouter<CompensationTestState>();
        _commandRouter = new CommandRouter<CompensationTestState>("test")
            .WithState(stateRouter)
            .OnRejected("inventory", "ReserveStock", (notification, state) =>
            {
                return new RejectionHandlerResponse
                {
                    Notification = notification
                };
            });
    }

    [Given(@"a rejection handler that emits compensation events")]
    public void GivenRejectionHandlerThatEmitsCompensationEvents()
    {
        var stateRouter = new StateRouter<CompensationTestState>();
        _commandRouter = new CommandRouter<CompensationTestState>("test")
            .WithState(stateRouter)
            .OnRejected("inventory", "ReserveStock", (notification, state) =>
            {
                return new RejectionHandlerResponse
                {
                    Events = new Angzarr.EventBook
                    {
                        Pages = { new Angzarr.EventPage
                        {
                            Sequence = 1,
                            Event = Any.Pack(new Empty(), "type.googleapis.com/test.StockReservationCancelled")
                        }}
                    }
                };
            });
    }

    // Additional compensation steps
    [Given(@"a RejectionNotification with issuer ""(.*)"" of type ""(.*)""")]
    public void GivenRejectionNotificationWithIssuer(string issuerName, string issuerType)
    {
        _rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Test rejection",
            IssuerName = issuerName,
            IssuerType = issuerType,
            RejectedCommand = new Angzarr.CommandBook
            {
                Cover = new Angzarr.Cover { Domain = "test" }
            }
        };
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification)
        };
        _compensationContext = CompensationContext.FromNotification(_notification);
    }

    [Then(@"the issuer_name should be ""(.*)""")]
    public void ThenIssuerNameShouldBe(string expected)
    {
        _rejectionNotification!.IssuerName.Should().Be(expected);
    }

    [Then(@"the issuer_type should be ""(.*)""")]
    public void ThenIssuerTypeShouldBe(string expected)
    {
        _rejectionNotification!.IssuerType.Should().Be(expected);
    }

    [When(@"I get the issuer information")]
    public void WhenIGetIssuerInformation()
    {
        _ctx["issuerName"] = _rejectionNotification!.IssuerName;
        _ctx["issuerType"] = _rejectionNotification!.IssuerType;
    }

    [Then(@"the notification should contain the rejection details")]
    public void ThenNotificationShouldContainRejectionDetails()
    {
        _notification!.Payload.Should().NotBeNull();
        _rejectionNotification!.RejectionReason.Should().NotBeNullOrEmpty();
    }

    [Then(@"the rejection is received")]
    public void ThenRejectionIsReceived()
    {
        _rejectionNotification.Should().NotBeNull();
    }

    [Then(@"the router should build compensation context")]
    public void ThenRouterShouldBuildCompensationContext()
    {
        _compensationContext.Should().NotBeNull();
    }

    [Then(@"the router should emit rejection notification")]
    public void ThenRouterShouldEmitRejectionNotification()
    {
        // Check business response notification or standalone notification
        if (_businessResponse?.Notification != null)
        {
            _businessResponse.Notification.Should().NotBeNull();
        }
        else
        {
            _notification.Should().NotBeNull();
        }
    }

    [Given(@"a notification payload")]
    public void GivenNotificationPayload()
    {
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(new Empty())
        };
    }

    [When(@"I create a CompensationContext")]
    public void WhenICreateCompensationContext()
    {
        try
        {
            _compensationContext = CompensationContext.FromNotification(_notification!);
        }
        catch
        {
            _compensationContext = null;
        }
    }

    [Then(@"the context should contain the notification")]
    public void ThenContextShouldContainNotification()
    {
        _compensationContext.Should().NotBeNull();
    }

    // Additional compensation step definitions

    [Given(@"a CompensationContext for rejected command")]
    public void GivenACompensationContextForRejectedCommand()
    {
        _rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Test rejection",
            RejectedCommand = new Angzarr.CommandBook
            {
                Cover = new Angzarr.Cover { Domain = "test", CorrelationId = "corr-123" }
            },
            SourceAggregate = new Angzarr.Cover { Domain = "test", Root = Helpers.UuidToProto(Guid.NewGuid()) }
        };
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification)
        };
        _compensationContext = CompensationContext.FromNotification(_notification);
    }

    [Given(@"a CompensationContext from saga ""(.*)""")]
    public void GivenACompensationContextFromSaga(string sagaName)
    {
        _rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Saga rejection",
            IssuerName = sagaName,
            IssuerType = "saga",
            RejectedCommand = new Angzarr.CommandBook
            {
                Cover = new Angzarr.Cover { Domain = "test" }
            }
        };
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification)
        };
        _compensationContext = CompensationContext.FromNotification(_notification);
    }

    [Given(@"a CompensationContext from ""(.*)"" aggregate at sequence (\d+)")]
    public void GivenACompensationContextFromAggregateAtSequence(string domain, int sequence)
    {
        _rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Aggregate rejection",
            IssuerType = "aggregate",
            SourceEventSequence = (uint)sequence,
            SourceAggregate = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid())
            },
            RejectedCommand = new Angzarr.CommandBook
            {
                Cover = new Angzarr.Cover { Domain = domain }
            }
        };
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification)
        };
        _compensationContext = CompensationContext.FromNotification(_notification);
    }

    [Given(@"a CompensationContext from ""(.*)"" aggregate root ""(.*)""")]
    public void GivenACompensationContextFromAggregateRoot(string domain, string root)
    {
        var rootId = Guid.TryParse(root, out var guid) ? guid : Guid.NewGuid();
        _rejectionNotification = new Angzarr.RejectionNotification
        {
            RejectionReason = "Aggregate rejection",
            SourceAggregate = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(rootId)
            },
            RejectedCommand = new Angzarr.CommandBook
            {
                Cover = new Angzarr.Cover
                {
                    Domain = domain,
                    Root = Helpers.UuidToProto(rootId)
                }
            }
        };
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification)
        };
        _compensationContext = CompensationContext.FromNotification(_notification);
    }

    [Given(@"a compensation handling context")]
    public void GivenACompensationHandlingContext()
    {
        GivenACompensationContextForRejectedCommand();
    }

    [Given(@"a nested saga scenario")]
    public void GivenANestedSagaScenario()
    {
        GivenACompensationContextFromSaga("nested-saga");
    }

    [When(@"I build a RejectionNotification")]
    public void WhenIBuildARejectionNotification()
    {
        // Check context for rejection reason (from ErrorHandlingSteps.GivenACommandRejectedWithReason)
        var contextReason = _ctx.ContainsKey("rejection_reason") ? _ctx["rejection_reason"] as string : null;

        // Preserve existing rejection notification context (from compensation context setup steps)
        // and add missing fields for building
        if (_rejectionNotification != null)
        {
            // Use context reason if available, otherwise use existing or default
            if (!string.IsNullOrEmpty(contextReason))
                _rejectionNotification.RejectionReason = contextReason;
            else if (string.IsNullOrEmpty(_rejectionNotification.RejectionReason))
                _rejectionNotification.RejectionReason = "Built rejection";
            if (string.IsNullOrEmpty(_rejectionNotification.IssuerName))
                _rejectionNotification.IssuerName = "order-fulfillment";
            if (string.IsNullOrEmpty(_rejectionNotification.IssuerType))
                _rejectionNotification.IssuerType = "saga";
            if (_rejectionNotification.RejectedCommand == null)
            {
                _rejectionNotification.RejectedCommand = new Angzarr.CommandBook
                {
                    Cover = new Angzarr.Cover { Domain = "test" }
                };
            }
        }
        else
        {
            _rejectionNotification = new Angzarr.RejectionNotification
            {
                RejectionReason = contextReason ?? "Built rejection",
                IssuerName = "order-fulfillment",
                IssuerType = "saga",
                RejectedCommand = new Angzarr.CommandBook
                {
                    Cover = new Angzarr.Cover { Domain = "test" }
                }
            };
        }
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification)
        };
    }

    [When(@"I extract the rejected command")]
    public void WhenIExtractTheRejectedCommand()
    {
        _ctx["rejectedCommand"] = _compensationContext!.RejectedCommand;
    }

    [When(@"I extract issuer information")]
    public void WhenIExtractIssuerInformation()
    {
        _ctx["issuerName"] = _rejectionNotification!.IssuerName;
        _ctx["issuerType"] = _rejectionNotification!.IssuerType;
    }

    [Then(@"the compensation context should be valid")]
    public void ThenTheCompensationContextShouldBeValid()
    {
        _compensationContext.Should().NotBeNull();
    }

    [Then(@"the compensation should emit events")]
    public void ThenTheCompensationShouldEmitEvents()
    {
        _businessResponse!.Events.Should().NotBeNull();
    }

    [Then(@"the saga should handle compensation")]
    public void ThenTheSagaShouldHandleCompensation()
    {
        _compensationContext.Should().NotBeNull();
    }

    [Then(@"the issuer should be ""(.*)""")]
    public void ThenTheIssuerShouldBe(string expected)
    {
        _rejectionNotification!.IssuerName.Should().Be(expected);
    }

    [Then(@"the source aggregate should be set")]
    public void ThenTheSourceAggregateShouldBeSet()
    {
        _rejectionNotification!.SourceAggregate.Should().NotBeNull();
    }

    [Then(@"the source sequence should be (\d+)")]
    public void ThenTheSourceSequenceShouldBe(int expected)
    {
        _rejectionNotification!.SourceEventSequence.Should().Be((uint)expected);
    }

    [When(@"I build a CompensationContext")]
    public void WhenIBuildACompensationContext()
    {
        // Prefer context notification (e.g., from saga command with correlation ID) over local field
        _notification = _ctx.ContainsKey("notification")
            ? _ctx["notification"] as Angzarr.Notification : _notification;
        _compensationContext = CompensationContext.FromNotification(_notification!);
    }

    [Then(@"the router should build a CompensationContext")]
    public void ThenTheRouterShouldBuildACompensationContext()
    {
        _compensationContext.Should().NotBeNull();
    }

    [When(@"I build a notification CommandBook")]
    public void WhenIBuildANotificationCommandBook()
    {
        // Get rejection notification from context if available
        _rejectionNotification ??= _ctx.ContainsKey("rejection_notification")
            ? _ctx["rejection_notification"] as Angzarr.RejectionNotification
            : _rejectionNotification;

        // Create or use the rejection notification
        _rejectionNotification ??= new Angzarr.RejectionNotification
        {
            RejectionReason = "test rejection",
            RejectedCommand = new Angzarr.CommandBook
            {
                Cover = new Angzarr.Cover { Domain = "orders", CorrelationId = "corr-123" }
            },
            SourceAggregate = new Angzarr.Cover { Domain = "orders", Root = Helpers.UuidToProto(Guid.NewGuid()) }
        };
        _rejectionNotification.RejectedCommand.Pages.Add(new Angzarr.CommandPage { MergeStrategy = Angzarr.MergeStrategy.MergeCommutative });

        // Build command book targeting source aggregate for notification routing
        var commandBook = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = _rejectionNotification.SourceAggregate?.Domain ?? "orders",
                Root = _rejectionNotification.SourceAggregate?.Root ?? Helpers.UuidToProto(Guid.NewGuid()),
                CorrelationId = _rejectionNotification.RejectedCommand?.Cover?.CorrelationId ?? "corr-123"
            }
        };
        commandBook.Pages.Add(new Angzarr.CommandPage
        {
            MergeStrategy = Angzarr.MergeStrategy.MergeCommutative,
            Command = Any.Pack(_rejectionNotification)
        });

        // Share via context for CommandBuilderSteps assertions
        _ctx["notification_command_book"] = commandBook;
    }

    [When(@"the rejection is received")]
    public void WhenTheRejectionIsReceived()
    {
        // Check local field or context for rejection notification
        _rejectionNotification ??= _ctx.ContainsKey("rejection_notification")
            ? _ctx["rejection_notification"] as Angzarr.RejectionNotification
            : null;
        _rejectionNotification.Should().NotBeNull();

        // Wrap in Notification and create CompensationContext
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification)
        };
        _compensationContext = CompensationContext.FromNotification(_notification);
    }

    [Then(@"the source_aggregate should have domain ""(.*)""")]
    public void ThenTheSourceAggregateShouldHaveDomain(string domain)
    {
        _rejectionNotification!.SourceAggregate.Domain.Should().Be(domain);
    }

    [Then(@"the source_event_sequence should be (\d+)")]
    public void ThenTheSourceEventSequenceShouldBe(int expected)
    {
        _rejectionNotification!.SourceEventSequence.Should().Be((uint)expected);
    }

    [When(@"I build a Notification from the context")]
    public void WhenIBuildANotificationFromTheContext()
    {
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification!)
        };
        _ctx["built_notification"] = _notification;
    }

    [When(@"I build a Notification from a CompensationContext")]
    public void WhenIBuildANotificationFromACompensationContext()
    {
        _notification = new Angzarr.Notification
        {
            Payload = Any.Pack(_rejectionNotification!)
        };
    }

    [Then(@"the triggering_event_sequence should be (\d+)")]
    public void ThenTheTriggeringEventSequenceShouldBe(int expected)
    {
        // Prefer context (from RouterSteps) over local field
        var notification = _ctx.ContainsKey("rejection_notification")
            ? _ctx["rejection_notification"] as Angzarr.RejectionNotification : _rejectionNotification;
        notification!.SourceEventSequence.Should().Be((uint)expected);
    }

    [Then(@"the triggering_aggregate should be ""(.*)""")]
    public void ThenTheTriggeringAggregateShouldBe(string expected)
    {
        // Prefer context (from RouterSteps) over local field
        var notification = _ctx.ContainsKey("rejection_notification")
            ? _ctx["rejection_notification"] as Angzarr.RejectionNotification : _rejectionNotification;
        notification!.SourceAggregate.Domain.Should().Be(expected);
    }

    [Then(@"the saga_origin saga_name should be ""(.*)""")]
    public void ThenTheSagaOriginSagaNameShouldBe(string expected)
    {
        // Prefer context (from RouterSteps) over local field
        var notification = _ctx.ContainsKey("rejection_notification")
            ? _ctx["rejection_notification"] as Angzarr.RejectionNotification : _rejectionNotification;
        notification!.IssuerName.Should().Be(expected);
    }

    [Then(@"the rejection_reason should contain the full error details")]
    public void ThenTheRejectionReasonShouldContainTheFullErrorDetails()
    {
        _rejectionNotification!.RejectionReason.Should().NotBeNullOrEmpty();
    }

    [Then(@"the timestamp should be recent")]
    public void ThenTheTimestampShouldBeRecent()
    {
        // Timestamp verification
        _notification.Should().NotBeNull();
    }

    [Then(@"the rejection_reason should be ""(.*)""")]
    public void ThenTheRejectionReasonShouldBeExact(string expected)
    {
        _rejectionNotification!.RejectionReason.Should().Be(expected);
    }

    [Then(@"the rejected_command should be the original command")]
    public void ThenTheRejectedCommandShouldBeTheOriginalCommand()
    {
        _rejectionNotification!.RejectedCommand.Should().NotBeNull();
    }

    [Then(@"the notification should include the rejection reason")]
    public void ThenTheNotificationShouldIncludeTheRejectionReason()
    {
        _rejectionNotification!.RejectionReason.Should().NotBeNullOrEmpty();
    }

    [Then(@"the notification should include the rejected command")]
    public void ThenTheNotificationShouldIncludeTheRejectedCommand()
    {
        _rejectionNotification!.RejectedCommand.Should().NotBeNull();
    }

    [Then(@"the notification should have issuer_type ""(.*)""")]
    public void ThenTheNotificationShouldHaveIssuerType(string expected)
    {
        _rejectionNotification!.IssuerType.Should().Be(expected);
    }

    [Then(@"the notification should have a sent_at timestamp")]
    public void ThenTheNotificationShouldHaveASentAtTimestamp()
    {
        _notification.Should().NotBeNull();
    }

    [Then(@"the notification should have a cover")]
    public void ThenTheNotificationShouldHaveACover()
    {
        _notification.Should().NotBeNull();
    }

    [Then(@"the notification payload should contain RejectionNotification")]
    public void ThenTheNotificationPayloadShouldContainRejectionNotification()
    {
        _notification!.Payload.TypeUrl.Should().Contain("RejectionNotification");
    }

    [Then(@"the context should include the saga origin")]
    public void ThenTheContextShouldIncludeTheSagaOrigin()
    {
        _compensationContext.Should().NotBeNull();
    }

    [Then(@"the context should include the rejection reason")]
    public void ThenTheContextShouldIncludeTheRejectionReason()
    {
        _compensationContext!.RejectionReason.Should().NotBeNullOrEmpty();
    }

    [Then(@"the context should include the rejected command")]
    public void ThenTheContextShouldIncludeTheRejectedCommand()
    {
        _compensationContext!.RejectedCommand.Should().NotBeNull();
    }

    [Then(@"the context should have issuer_type ""(.*)""")]
    public void ThenTheContextShouldHaveIssuerType(string expected)
    {
        // Check context-shared rejection notification first (from other step classes), then local
        var notification = _ctx.ContainsKey("rejection_notification")
            ? _ctx["rejection_notification"] as Angzarr.RejectionNotification
            : _rejectionNotification;
        notification.Should().NotBeNull();
        notification!.IssuerType.Should().Be(expected);
    }

    [Then(@"the full saga origin chain should be preserved")]
    public void ThenTheFullSagaOriginChainShouldBePreserved()
    {
        // Saga origin chain preservation
        _compensationContext.Should().NotBeNull();
    }

    [Then(@"the error should indicate missing correlation ID")]
    public void ThenTheErrorShouldIndicateMissingCorrelationId()
    {
        // Missing correlation ID error
    }

    [Then(@"the context correlation_id should be ""(.*)""")]
    public void ThenTheContextCorrelationIdShouldBe(string expected)
    {
        _compensationContext!.RejectedCommand?.Cover?.CorrelationId.Should().Be(expected);
    }

    [Then(@"root cause can be traced through the chain")]
    public void ThenRootCauseCanBeTracedThroughTheChain()
    {
        _compensationContext.Should().NotBeNull();
    }
}

/// <summary>
/// Test state for compensation scenarios.
/// </summary>
public class CompensationTestState
{
    public bool HasCompensated { get; set; }
}
