using Angzarr;
using Angzarr.Client;
using FluentAssertions;
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

        var response = new RejectionHandlerResponse
        {
            Events = eventBook
        };

        response.Events.Should().NotBeNull();
        response.Events!.Pages.Count.Should().Be(1);
        response.Notification.Should().BeNull();
    }

    [Fact]
    public void ResponseWithNotificationOnly()
    {
        var notification = MakeNotification("inventory", "ReserveStock", "out of stock");

        var response = new RejectionHandlerResponse
        {
            Notification = notification
        };

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
            Notification = notification
        };

        response.Events.Should().NotBeNull();
        response.Notification.Should().NotBeNull();
    }

    [Fact]
    public void ResponseEventsAreAccessible()
    {
        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage
        {
            Event = Any.Pack(new RejectionNotification { RejectionReason = "test1" })
        });
        eventBook.Pages.Add(new EventPage
        {
            Event = Any.Pack(new RejectionNotification { RejectionReason = "test2" })
        });

        var response = new RejectionHandlerResponse
        {
            Events = eventBook
        };

        response.Events!.Pages.Count.Should().Be(2);
    }

    // =========================================================================
    // CommandRouter OnRejected Tests
    // =========================================================================

    private class TestState
    {
        public int Value { get; set; }
    }

    [Fact]
    public void CommandRouter_OnRejected_ReturnsEvents()
    {
        var router = new CommandRouter<TestState>("test", _ => new TestState())
            .OnRejected("inventory", "ReserveStock", (notification, state) =>
            {
                return new RejectionHandlerResponse
                {
                    Events = new EventBook
                    {
                        Pages =
                        {
                            new EventPage
                            {
                                Event = new Any { TypeUrl = "type.googleapis.com/test.Compensated" }
                            }
                        }
                    }
                };
            });

        var notification = MakeNotification("inventory", "ReserveStock", "out of stock");
        var notificationAny = Any.Pack(notification);

        var cmd = new ContextualCommand
        {
            Command = new CommandBook
            {
                Pages = { new CommandPage { Command = notificationAny } }
            }
        };

        var response = router.Dispatch(cmd);

        response.Events.Should().NotBeNull();
        response.Events!.Pages.Count.Should().Be(1);
    }

    [Fact]
    public void CommandRouter_OnRejected_ReturnsNotification()
    {
        var router = new CommandRouter<TestState>("test", _ => new TestState())
            .OnRejected("payment", "Charge", (notification, state) =>
            {
                return new RejectionHandlerResponse
                {
                    Notification = notification
                };
            });

        var notification = MakeNotification("payment", "Charge", "declined");
        var notificationAny = Any.Pack(notification);

        var cmd = new ContextualCommand
        {
            Command = new CommandBook
            {
                Pages = { new CommandPage { Command = notificationAny } }
            }
        };

        var response = router.Dispatch(cmd);

        response.Notification.Should().NotBeNull();
    }

    [Fact]
    public void CommandRouter_OnRejected_NoHandler_DelegatesToFramework()
    {
        var router = new CommandRouter<TestState>("test", _ => new TestState());
        // No rejection handler registered

        var notification = MakeNotification("unknown", "UnknownCommand", "reason");
        var notificationAny = Any.Pack(notification);

        var cmd = new ContextualCommand
        {
            Command = new CommandBook
            {
                Pages = { new CommandPage { Command = notificationAny } }
            }
        };

        var response = router.Dispatch(cmd);

        // Should delegate to framework (revocation response with EmitSystemRevocation = true)
        response.Revocation.Should().NotBeNull();
        response.Revocation!.EmitSystemRevocation.Should().BeTrue();
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    private static EventBook MakeEventBook()
    {
        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage
        {
            Event = new Any { TypeUrl = "type.googleapis.com/test.TestEvent" }
        });
        return eventBook;
    }

    private static Notification MakeNotification(string domain, string commandType, string reason)
    {
        var rejectedCommand = new CommandBook
        {
            Cover = new Cover { Domain = domain }
        };
        rejectedCommand.Pages.Add(new CommandPage
        {
            Command = new Any { TypeUrl = $"type.googleapis.com/test.{commandType}" }
        });

        var rejection = new RejectionNotification
        {
            IssuerName = "test-saga",
            IssuerType = "saga",
            RejectionReason = reason,
            RejectedCommand = rejectedCommand
        };

        return new Notification
        {
            Payload = Any.Pack(rejection)
        };
    }
}
