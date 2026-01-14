using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Moq;
using Serilog;
using Xunit;

namespace Angzarr.Examples.Projector.Tests;

public class TransactionLogProjectorTests
{
    private readonly Mock<ILogger> _mockLogger;
    private readonly ITransactionLogProjector _projector;

    public TransactionLogProjectorTests()
    {
        _mockLogger = new Mock<ILogger>();
        _mockLogger.Setup(l => l.ForContext<TransactionLogProjector>()).Returns(_mockLogger.Object);
        _projector = new TransactionLogProjector(_mockLogger.Object);
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
    public void LogEvents_TransactionCreated_LogsEvent()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new TransactionCreated
        {
            CustomerId = "cust-123",
            SubtotalCents = 1000,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        created.Items.Add(new LineItem { Name = "Widget", Quantity = 2, UnitPriceCents = 500 });

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = transactionId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    [Fact]
    public void LogEvents_DiscountApplied_LogsEvent()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var applied = new DiscountApplied
        {
            DiscountType = "percentage",
            Value = 10,
            DiscountCents = 100
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = transactionId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(applied) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    [Fact]
    public void LogEvents_TransactionCompleted_LogsEvent()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var completed = new TransactionCompleted
        {
            FinalTotalCents = 900,
            PaymentMethod = "card",
            LoyaltyPointsEarned = 9,
            CompletedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = transactionId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(completed) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    [Fact]
    public void LogEvents_TransactionCancelled_LogsEvent()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var cancelled = new TransactionCancelled
        {
            Reason = "customer request",
            CancelledAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = transactionId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(cancelled) });

        _projector.LogEvents(eventBook);

        VerifyLoggedOnce();
    }

    [Fact]
    public void LogEvents_FullTransactionLifecycle_LogsAllEvents()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new TransactionCreated { CustomerId = "cust-1", SubtotalCents = 1000 };
        var discount = new DiscountApplied { DiscountType = "fixed", Value = 100, DiscountCents = 100 };
        var completed = new TransactionCompleted { FinalTotalCents = 900, PaymentMethod = "card" };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = transactionId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(discount) });
        eventBook.Pages.Add(new EventPage { Num = 2, Event = Any.Pack(completed) });

        _projector.LogEvents(eventBook);

        VerifyLoggedTimes(3);
    }

    [Fact]
    public void LogEvents_UnknownEventType_LogsWithRawBytes()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var unknown = new CustomerCreated { Name = "test" };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = transactionId }
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
            Cover = new Cover { Domain = "transaction" }
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
        var created = new TransactionCreated { CustomerId = "test", SubtotalCents = 100 };
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
