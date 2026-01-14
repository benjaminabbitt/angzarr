using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Moq;
using Serilog;
using Xunit;

namespace Angzarr.Examples.Saga.Tests;

public class LoyaltySagaTests
{
    private readonly ILoyaltySaga _saga;

    public LoyaltySagaTests()
    {
        var logger = new Mock<ILogger>();
        logger.Setup(l => l.ForContext<LoyaltySaga>()).Returns(logger.Object);
        _saga = new LoyaltySaga(logger.Object);
    }

    [Fact]
    public void ProcessEvents_EmptyEventBook_ReturnsEmptyList()
    {
        var eventBook = new EventBook();

        var result = _saga.ProcessEvents(eventBook);

        Assert.Empty(result);
    }

    [Fact]
    public void ProcessEvents_NoTransactionCompleted_ReturnsEmptyList()
    {
        var created = new TransactionCreated { CustomerId = "cust-1", SubtotalCents = 1000 };
        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });

        var result = _saga.ProcessEvents(eventBook);

        Assert.Empty(result);
    }

    [Fact]
    public void ProcessEvents_TransactionCompleted_ReturnsAddLoyaltyPointsCommand()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var completed = new TransactionCompleted
        {
            FinalTotalCents = 1000,
            PaymentMethod = "card",
            LoyaltyPointsEarned = 10
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(completed) });

        var result = _saga.ProcessEvents(eventBook);

        Assert.Single(result);
        Assert.Equal("customer", result[0].Cover.Domain);
        Assert.Single(result[0].Pages);
        Assert.EndsWith("AddLoyaltyPoints", result[0].Pages[0].Command.TypeUrl);
    }

    [Fact]
    public void ProcessEvents_ZeroPoints_ReturnsEmptyList()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var completed = new TransactionCompleted
        {
            FinalTotalCents = 50,
            PaymentMethod = "cash",
            LoyaltyPointsEarned = 0
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(completed) });

        var result = _saga.ProcessEvents(eventBook);

        Assert.Empty(result);
    }

    [Fact]
    public void ProcessEvents_NoRootId_ReturnsEmptyList()
    {
        var completed = new TransactionCompleted
        {
            FinalTotalCents = 1000,
            PaymentMethod = "card",
            LoyaltyPointsEarned = 10
        };

        var eventBook = new EventBook
        {
            Cover = new Cover { Domain = "transaction" }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(completed) });

        var result = _saga.ProcessEvents(eventBook);

        Assert.Empty(result);
    }

    [Fact]
    public void ProcessEvents_MultipleCompletedEvents_ReturnsMultipleCommands()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var completed1 = new TransactionCompleted { LoyaltyPointsEarned = 10 };
        var completed2 = new TransactionCompleted { LoyaltyPointsEarned = 20 };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(completed1) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(completed2) });

        var result = _saga.ProcessEvents(eventBook);

        Assert.Equal(2, result.Count);
    }

    [Fact]
    public void ProcessEvents_CommandIsAsync()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var completed = new TransactionCompleted { LoyaltyPointsEarned = 10 };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(completed) });

        var result = _saga.ProcessEvents(eventBook);

        Assert.False(result[0].Pages[0].Synchronous);
    }

    [Fact]
    public void ProcessEvents_CommandHasCorrectPoints()
    {
        var customerId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var completed = new TransactionCompleted { LoyaltyPointsEarned = 42 };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = customerId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(completed) });

        var result = _saga.ProcessEvents(eventBook);

        var addPoints = result[0].Pages[0].Command.Unpack<AddLoyaltyPoints>();
        Assert.Equal(42, addPoints.Points);
        Assert.StartsWith("transaction:", addPoints.Reason);
    }
}
