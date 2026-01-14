using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Moq;
using Serilog;
using Xunit;

namespace Angzarr.Examples.Projector.Tests;

public class CustomerLogProjectorTests
{
    private readonly Mock<ILogger> _mockLogger;
    private readonly ICustomerLogProjector _projector;

    public CustomerLogProjectorTests()
    {
        _mockLogger = new Mock<ILogger>();
        _mockLogger.Setup(l => l.ForContext<CustomerLogProjector>()).Returns(_mockLogger.Object);
        _projector = new CustomerLogProjector(_mockLogger.Object);
    }

    [Fact]
    public void LogEvents_EmptyEventBook_NoLogging()
    {
        var eventBook = new EventBook();

        _projector.LogEvents(eventBook);

        _mockLogger.Verify(
            l => l.Information(It.IsAny<string>(), It.IsAny<object>()),
            Times.Never);
    }

    [Fact]
    public void LogEvents_CustomerCreated_LogsEvent()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new CustomerCreated
        {
            Name = "John Doe",
            Email = "john@example.com",
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "customer",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    [Fact]
    public void LogEvents_LoyaltyPointsAdded_LogsEvent()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var added = new LoyaltyPointsAdded
        {
            Points = 100,
            NewBalance = 100,
            Reason = "welcome bonus"
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "customer",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(added) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    [Fact]
    public void LogEvents_LoyaltyPointsRedeemed_LogsEvent()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var redeemed = new LoyaltyPointsRedeemed
        {
            Points = 50,
            NewBalance = 50,
            RedemptionType = "discount"
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "customer",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(redeemed) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    [Fact]
    public void LogEvents_MultipleEvents_LogsAll()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new CustomerCreated { Name = "Jane", Email = "jane@example.com" };
        var added = new LoyaltyPointsAdded { Points = 50, NewBalance = 50 };
        var redeemed = new LoyaltyPointsRedeemed { Points = 25, NewBalance = 25 };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "customer",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(added) });
        eventBook.Pages.Add(new EventPage { Num = 2, Event = Any.Pack(redeemed) });

        _projector.LogEvents(eventBook);

        VerifyLoggedTimes(3);
    }

    [Fact]
    public void LogEvents_UnknownEventType_LogsWithRawBytes()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var unknown = new TransactionCreated { CustomerId = "test" };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "customer",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(unknown) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    [Fact]
    public void LogEvents_NullEvent_SkipsPage()
    {
        var eventBook = new EventBook
        {
            Cover = new Cover { Domain = "customer" }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = null });

        _projector.LogEvents(eventBook);

        _mockLogger.Verify(
            l => l.Information(It.IsAny<string>(), It.IsAny<object>()),
            Times.Never);
    }

    [Fact]
    public void LogEvents_NoCover_UsesDefaultDomain()
    {
        var created = new CustomerCreated { Name = "Test", Email = "test@test.com" };
        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    private void VerifyLoggedOnce() => VerifyLoggedTimes(1);

    private void VerifyLoggedTimes(int times)
    {
        _mockLogger.Verify(
            l => l.Information(
                It.Is<string>(s => s == "event {@Data}"),
                It.IsAny<It.IsAnyType>()),
            Times.Exactly(times));
    }
}
