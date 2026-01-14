using Examples;
using Angzarr;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Net.Client;
using Reqnroll;
using Xunit;

namespace Integration.Tests.StepDefinitions;

[Binding]
public class EndToEndSteps : IDisposable
{
    private const string CustomerDomain = "customer";
    private const string TransactionDomain = "transaction";

    private readonly TestContext _context = new();

    [BeforeScenario]
    public void SetUp()
    {
        _context.Reset();
    }

    [AfterScenario]
    public void TearDown()
    {
        _context.Dispose();
    }

    [Given("the angzarr system is running at {string}")]
    public void GivenTheAngzarrSystemIsRunningAt(string hostPort)
    {
        var parts = hostPort.Split(':');
        _context.AngzarrHost = parts[0];
        _context.AngzarrPort = int.Parse(parts[1]);

        _context.Channel = GrpcChannel.ForAddress($"http://{_context.AngzarrHost}:{_context.AngzarrPort}");
    }

    [Given("a new customer id")]
    public void GivenANewCustomerId()
    {
        _context.CurrentCustomerId = Guid.NewGuid();
    }

    [Given("a new transaction id for the customer")]
    public void GivenANewTransactionIdForTheCustomer()
    {
        _context.CurrentTransactionId = Guid.NewGuid();
    }

    [When("I send a CreateCustomer command with name {string} and email {string}")]
    [Given("I send a CreateCustomer command with name {string} and email {string}")]
    public void WhenISendACreateCustomerCommandWithNameAndEmail(string name, string email)
    {
        var command = new CreateCustomer
        {
            Name = name,
            Email = email
        };

        SendCommand(CustomerDomain, _context.CurrentCustomerId!.Value, command);
    }

    [When("I send a CreateTransaction command with items:")]
    public void WhenISendACreateTransactionCommandWithItems(Table table)
    {
        var command = new CreateTransaction
        {
            CustomerId = _context.CurrentCustomerId.ToString()!
        };

        foreach (var row in table.Rows)
        {
            command.Items.Add(new LineItem
            {
                ProductId = row["product_id"],
                Name = row["name"],
                Quantity = int.Parse(row["quantity"]),
                UnitPriceCents = int.Parse(row["unit_price_cents"])
            });
        }

        SendCommand(TransactionDomain, _context.CurrentTransactionId!.Value, command);
    }

    [When("I send a CompleteTransaction command with payment method {string}")]
    public void WhenISendACompleteTransactionCommandWithPaymentMethod(string paymentMethod)
    {
        var command = new CompleteTransaction
        {
            PaymentMethod = paymentMethod
        };

        SendCommand(TransactionDomain, _context.CurrentTransactionId!.Value, command);
    }

    [When("I query events for the customer aggregate")]
    public void WhenIQueryEventsForTheCustomerAggregate()
    {
        QueryEvents(CustomerDomain, _context.CurrentCustomerId!.Value);
    }

    [Then("the command succeeds")]
    [Given("the command succeeds")]
    public void ThenTheCommandSucceeds()
    {
        Assert.Null(_context.LastException);
        Assert.NotNull(_context.LastResponse);
    }

    [Then("the customer aggregate has {int} event(s)")]
    public void ThenTheCustomerAggregateHasEvents(int expectedCount)
    {
        var eventCount = GetEventCount(CustomerDomain, _context.CurrentCustomerId!.Value);
        Assert.Equal(expectedCount, eventCount);
    }

    [Then("the transaction aggregate has {int} event(s)")]
    public void ThenTheTransactionAggregateHasEvents(int expectedCount)
    {
        var eventCount = GetEventCount(TransactionDomain, _context.CurrentTransactionId!.Value);
        Assert.Equal(expectedCount, eventCount);
    }

    [Then("the latest event type is {string}")]
    public void ThenTheLatestEventTypeIs(string expectedType)
    {
        Assert.NotNull(_context.LastResponse);
        Assert.NotNull(_context.LastResponse.Events);
        Assert.NotEmpty(_context.LastResponse.Events.Pages);

        var eventTypeUrl = _context.LastResponse.Events.Pages.Last().Event.TypeUrl;
        var actualType = eventTypeUrl[(eventTypeUrl.LastIndexOf('.') + 1)..];
        Assert.Equal(expectedType, actualType);
    }

    [Then("a projection was returned from projector {string}")]
    public void ThenAProjectionWasReturnedFromProjector(string projectorName)
    {
        Assert.NotNull(_context.LastResponse);
        Assert.Contains(_context.LastResponse.Projections, p => p.Projector == projectorName);
    }

    [Then("the projection contains a Receipt with total {int} cents")]
    public void ThenTheProjectionContainsAReceiptWithTotalCents(int expectedTotal)
    {
        Assert.NotNull(_context.LastResponse);

        var receiptProjection = _context.LastResponse.Projections
            .FirstOrDefault(p => p.Projector == "receipt");
        Assert.NotNull(receiptProjection);

        var receipt = receiptProjection.Projection_.Unpack<Receipt>();
        Assert.Equal(expectedTotal, receipt.FinalTotalCents);
    }

    [Then("I receive {int} event(s)")]
    public void ThenIReceiveEvents(int expectedCount)
    {
        Assert.NotNull(_context.LastEventBook);
        Assert.Equal(expectedCount, _context.LastEventBook.Pages.Count);
    }

    [Then("the event at sequence {int} has type {string}")]
    public void ThenTheEventAtSequenceHasType(int sequence, string expectedType)
    {
        Assert.NotNull(_context.LastEventBook);
        Assert.True(sequence < _context.LastEventBook.Pages.Count, "Sequence out of bounds");

        var eventTypeUrl = _context.LastEventBook.Pages[sequence].Event.TypeUrl;
        var actualType = eventTypeUrl[(eventTypeUrl.LastIndexOf('.') + 1)..];
        Assert.Equal(expectedType, actualType);
    }

    private void SendCommand(string domain, Guid aggregateId, IMessage command)
    {
        try
        {
            var client = new BusinessCoordinator.BusinessCoordinatorClient(_context.Channel);

            var commandBook = new CommandBook
            {
                Cover = new Cover
                {
                    Domain = domain,
                    Root = ToProtoUuid(aggregateId)
                }
            };
            commandBook.Pages.Add(new CommandPage
            {
                Sequence = 0,
                Synchronous = true,
                Command = Any.Pack(command)
            });

            var response = client.Handle(commandBook);
            _context.LastResponse = response;
            _context.LastException = null;
        }
        catch (Exception e)
        {
            _context.LastException = e;
            _context.LastResponse = null;
        }
    }

    private void QueryEvents(string domain, Guid aggregateId)
    {
        try
        {
            var client = new EventQuery.EventQueryClient(_context.Channel);

            var query = new Query
            {
                Domain = domain,
                Root = ToProtoUuid(aggregateId)
            };

            using var call = client.GetEvents(query);
            var results = ReadStreamToList(call.ResponseStream);
            if (results.Count > 0)
            {
                _context.LastEventBook = results[0];
            }
            _context.LastException = null;
        }
        catch (Exception e)
        {
            _context.LastException = e;
            _context.LastEventBook = null;
        }
    }

    private int GetEventCount(string domain, Guid aggregateId)
    {
        try
        {
            var client = new EventQuery.EventQueryClient(_context.Channel);

            var query = new Query
            {
                Domain = domain,
                Root = ToProtoUuid(aggregateId)
            };

            using var call = client.GetEvents(query);
            var results = ReadStreamToList(call.ResponseStream);
            return results.Count > 0 ? results[0].Pages.Count : 0;
        }
        catch
        {
            return 0;
        }
    }

    private static List<EventBook> ReadStreamToList(Grpc.Core.IAsyncStreamReader<EventBook> stream)
    {
        var results = new List<EventBook>();
        while (stream.MoveNext(CancellationToken.None).GetAwaiter().GetResult())
        {
            results.Add(stream.Current);
        }
        return results;
    }

    private static Angzarr.UUID ToProtoUuid(Guid guid)
    {
        return new Angzarr.UUID
        {
            Value = ByteString.CopyFrom(guid.ToByteArray())
        };
    }

    public void Dispose()
    {
        _context.Dispose();
    }
}
