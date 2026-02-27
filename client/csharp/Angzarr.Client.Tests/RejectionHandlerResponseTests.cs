using Angzarr;
using Angzarr.Client;
using Angzarr.Client.Router;
using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Xunit;

namespace Angzarr.Client.Tests;

/// <summary>
/// Integration tests for RejectionHandlerResponse.
///
/// Tests the unified response type for rejection handlers that can return
/// both compensation events AND upstream notification.
/// </summary>
public class RejectionHandlerResponseTests
{
    // =========================================================================
    // RejectionHandlerResponse Tests
    // =========================================================================

    [Fact]
    public void EmptyResponse_HasNoEventsOrNotification()
    {
        var response = new RejectionHandlerResponse();

        response.Events.Should().BeNull();
        response.Notification.Should().BeNull();
    }

    [Fact]
    public void ResponseWithEventsOnly()
    {
        var eventBook = MakeEventBook();

        var response = new RejectionHandlerResponse { Events = eventBook };

        response.Events.Should().NotBeNull();
        response.Events!.Pages.Count.Should().Be(1);
        response.Notification.Should().BeNull();
    }

    [Fact]
    public void ResponseWithNotificationOnly()
    {
        var notification = MakeNotification("inventory", "ReserveStock", "out of stock");

        var response = new RejectionHandlerResponse { Notification = notification };

        response.Events.Should().BeNull();
        response.Notification.Should().NotBeNull();
    }

    [Fact]
    public void ResponseWithBothEventsAndNotification()
    {
        var eventBook = MakeEventBook();
        var notification = MakeNotification("payment", "ProcessPayment", "declined");

        var response = new RejectionHandlerResponse
        {
            Events = eventBook,
            Notification = notification,
        };

        response.Events.Should().NotBeNull();
        response.Notification.Should().NotBeNull();
    }

    [Fact]
    public void ResponseEventsAreAccessible()
    {
        var eventBook = new EventBook();
        eventBook.Pages.Add(
            new EventPage
            {
                Event = Any.Pack(new RejectionNotification { RejectionReason = "test1" }),
            }
        );
        eventBook.Pages.Add(
            new EventPage
            {
                Event = Any.Pack(new RejectionNotification { RejectionReason = "test2" }),
            }
        );

        var response = new RejectionHandlerResponse { Events = eventBook };

        response.Events!.Pages.Count.Should().Be(2);
    }

    // =========================================================================
    // CommandHandlerRouter Rejection Handling Tests (using new unified router pattern)
    // =========================================================================

    private class TestState
    {
        public int Value { get; set; }
    }

    /// <summary>
    /// Handler that processes rejection notifications and returns compensation events.
    /// </summary>
    private class RejectionTestHandler : ICommandHandlerDomainHandler<TestState>
    {
        private readonly StateRouter<TestState> _stateRouter = new StateRouter<TestState>();
        private readonly Func<Notification, TestState, RejectionHandlerResponse>? _rejectionHandler;

        public RejectionTestHandler(
            Func<Notification, TestState, RejectionHandlerResponse>? rejectionHandler = null
        )
        {
            _rejectionHandler = rejectionHandler;
        }

        public IReadOnlyList<string> CommandTypes() => new[] { "angzarr.Notification" };

        public StateRouter<TestState> StateRouter() => _stateRouter;

        public EventBook Handle(CommandBook cmd, Any payload, TestState state, int seq)
        {
            // Regular command handling - not rejections
            return new EventBook();
        }

        public RejectionHandlerResponse OnRejected(
            Notification notification,
            TestState state,
            string targetDomain,
            string targetCommand
        )
        {
            if (_rejectionHandler != null)
            {
                return _rejectionHandler(notification, state);
            }
            // No handler - return empty response (framework handles)
            return new RejectionHandlerResponse();
        }
    }

    [Fact]
    public void CommandHandlerRouter_WithRejectionHandler_ReturnsEvents()
    {
        var handler = new RejectionTestHandler(
            (notification, state) =>
            {
                return new RejectionHandlerResponse
                {
                    Events = new EventBook
                    {
                        Pages =
                        {
                            new EventPage
                            {
                                Event = new Any
                                {
                                    TypeUrl = "type.googleapis.com/test.Compensated",
                                },
                            },
                        },
                    },
                };
            }
        );

        var router = new CommandHandlerRouter<TestState, RejectionTestHandler>(
            "test",
            "test",
            handler
        );

        var notification = MakeNotification("inventory", "ReserveStock", "out of stock");
        var notificationAny = Any.Pack(notification);

        var cmd = new ContextualCommand
        {
            Command = new CommandBook { Pages = { new CommandPage { Command = notificationAny } } },
        };

        var response = router.Dispatch(cmd);

        response.Events.Should().NotBeNull();
        response.Events!.Pages.Count.Should().Be(1);
    }

    [Fact]
    public void CommandHandlerRouter_WithRejectionHandler_HandlerReceivesNotification()
    {
        Notification? receivedNotification = null;

        var handler = new RejectionTestHandler(
            (notification, state) =>
            {
                receivedNotification = notification;
                return new RejectionHandlerResponse
                {
                    Events = new EventBook
                    {
                        Pages = { new EventPage { Event = Any.Pack(new Empty()) } },
                    },
                };
            }
        );

        var router = new CommandHandlerRouter<TestState, RejectionTestHandler>(
            "test",
            "test",
            handler
        );

        var notification = MakeNotification("payment", "Charge", "declined");
        var notificationAny = Any.Pack(notification);

        var cmd = new ContextualCommand
        {
            Command = new CommandBook { Pages = { new CommandPage { Command = notificationAny } } },
        };

        router.Dispatch(cmd);

        receivedNotification.Should().NotBeNull();
        receivedNotification!.Payload.TypeUrl.Should().Contain("RejectionNotification");
    }

    [Fact]
    public void CommandHandlerRouter_NoHandler_ReturnsRevocationResponse()
    {
        // Handler without rejection handling
        var handler = new RejectionTestHandler(null);

        var router = new CommandHandlerRouter<TestState, RejectionTestHandler>(
            "test",
            "test",
            handler
        );

        var notification = MakeNotification("unknown", "UnknownCommand", "reason");
        var notificationAny = Any.Pack(notification);

        var cmd = new ContextualCommand
        {
            Command = new CommandBook { Pages = { new CommandPage { Command = notificationAny } } },
        };

        var response = router.Dispatch(cmd);

        // When no rejection handler is registered, the router returns a RevocationResponse
        // to delegate handling to the framework
        response.Revocation.Should().NotBeNull();
        response.Revocation!.EmitSystemRevocation.Should().BeTrue();
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    private static EventBook MakeEventBook()
    {
        var eventBook = new EventBook();
        eventBook.Pages.Add(
            new EventPage { Event = new Any { TypeUrl = "type.googleapis.com/test.TestEvent" } }
        );
        return eventBook;
    }

    private static Notification MakeNotification(string domain, string commandType, string reason)
    {
        var rejectedCommand = new CommandBook { Cover = new Cover { Domain = domain } };
        rejectedCommand.Pages.Add(
            new CommandPage
            {
                Command = new Any { TypeUrl = $"type.googleapis.com/test.{commandType}" },
            }
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
