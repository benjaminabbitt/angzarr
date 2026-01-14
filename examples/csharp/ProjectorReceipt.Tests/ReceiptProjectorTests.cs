using Angzarr;
using Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Moq;
using Serilog;
using Xunit;

namespace Angzarr.Examples.Projector.Tests;

public class ReceiptProjectorTests
{
    private readonly IReceiptProjector _projector;

    public ReceiptProjectorTests()
    {
        var logger = new Mock<ILogger>();
        logger.Setup(l => l.ForContext<ReceiptProjector>()).Returns(logger.Object);
        _projector = new ReceiptProjector(logger.Object);
    }

    [Fact]
    public void Project_EmptyEventBook_ReturnsNull()
    {
        var eventBook = new EventBook();

        var result = _projector.Project(eventBook);

        Assert.Null(result);
    }

    [Fact]
    public void Project_NotCompleted_ReturnsNull()
    {
        var created = new TransactionCreated
        {
            CustomerId = "cust-1",
            SubtotalCents = 1000
        };
        var eventBook = new EventBook();
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });

        var result = _projector.Project(eventBook);

        Assert.Null(result);
    }

    [Fact]
    public void Project_Completed_ReturnsProjection()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new TransactionCreated
        {
            CustomerId = "cust-1",
            SubtotalCents = 1000
        };
        created.Items.Add(new LineItem { Name = "Widget", Quantity = 2, UnitPriceCents = 500 });

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
                Root = new UUID { Value = transactionId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(completed) });

        var result = _projector.Project(eventBook);

        Assert.NotNull(result);
        Assert.Equal("receipt", result.Projector);
    }

    [Fact]
    public void Project_ContainsReceipt()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new TransactionCreated { CustomerId = "cust-123", SubtotalCents = 1500 };
        created.Items.Add(new LineItem { Name = "Gadget", Quantity = 3, UnitPriceCents = 500 });

        var completed = new TransactionCompleted
        {
            FinalTotalCents = 1500,
            PaymentMethod = "credit",
            LoyaltyPointsEarned = 15
        };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = transactionId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(completed) });

        var result = _projector.Project(eventBook);

        var receipt = result!.Projection_.Unpack<Receipt>();
        Assert.Equal("cust-123", receipt.CustomerId);
        Assert.Equal(1500, receipt.SubtotalCents);
        Assert.Equal(1500, receipt.FinalTotalCents);
        Assert.Equal("credit", receipt.PaymentMethod);
        Assert.Equal(15, receipt.LoyaltyPointsEarned);
        Assert.Single(receipt.Items);
    }

    [Fact]
    public void Project_WithDiscount_IncludesDiscountInReceipt()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new TransactionCreated { CustomerId = "cust-1", SubtotalCents = 1000 };
        var discount = new DiscountApplied { DiscountType = "percentage", Value = 10, DiscountCents = 100 };
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

        var result = _projector.Project(eventBook);

        var receipt = result!.Projection_.Unpack<Receipt>();
        Assert.Equal(100, receipt.DiscountCents);
    }

    [Fact]
    public void Project_FormattedTextContainsReceipt()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new TransactionCreated { CustomerId = "cust-1", SubtotalCents = 1000 };
        created.Items.Add(new LineItem { Name = "Widget", Quantity = 1, UnitPriceCents = 1000 });
        var completed = new TransactionCompleted { FinalTotalCents = 1000, PaymentMethod = "cash" };

        var eventBook = new EventBook
        {
            Cover = new Cover
            {
                Domain = "transaction",
                Root = new UUID { Value = transactionId }
            }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 1, Event = Any.Pack(completed) });

        var result = _projector.Project(eventBook);

        var receipt = result!.Projection_.Unpack<Receipt>();
        Assert.Contains("RECEIPT", receipt.FormattedText);
        Assert.Contains("Widget", receipt.FormattedText);
        Assert.Contains("Thank you", receipt.FormattedText);
    }

    [Fact]
    public void Project_SequenceMatchesLastPage()
    {
        var transactionId = ByteString.CopyFrom(Guid.NewGuid().ToByteArray());
        var created = new TransactionCreated { CustomerId = "cust-1", SubtotalCents = 1000 };
        var completed = new TransactionCompleted { FinalTotalCents = 1000, PaymentMethod = "card" };

        var eventBook = new EventBook
        {
            Cover = new Cover { Root = new UUID { Value = transactionId } }
        };
        eventBook.Pages.Add(new EventPage { Num = 0, Event = Any.Pack(created) });
        eventBook.Pages.Add(new EventPage { Num = 5, Event = Any.Pack(completed) });

        var result = _projector.Project(eventBook);

        Assert.Equal(5u, result!.Sequence);
    }
}
