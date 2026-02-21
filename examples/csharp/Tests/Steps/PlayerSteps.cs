using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using TechTalk.SpecFlow;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Player.Agg;

namespace Tests.Steps;

[Binding]
public class PlayerSteps
{
    private readonly ScenarioContext _context;

    public PlayerSteps(ScenarioContext context)
    {
        _context = context;
    }

    private List<EventPage> Events
    {
        get
        {
            if (!_context.TryGetValue("events", out List<EventPage>? events))
            {
                events = new List<EventPage>();
                _context["events"] = events;
            }
            return events!;
        }
        set => _context["events"] = value;
    }

    private IMessage? ResultEvent
    {
        get => _context.TryGetValue("resultEvent", out IMessage? evt) ? evt : null;
        set => _context["resultEvent"] = value!;
    }

    private Any? ResultEventAny
    {
        get => _context.TryGetValue("resultEventAny", out Any? evt) ? evt : null;
        set => _context["resultEventAny"] = value!;
    }

    private CommandRejectedError? Error
    {
        get => _context.TryGetValue("error", out CommandRejectedError? err) ? err : null;
        set => _context["error"] = value!;
    }

    private PlayerAggregate? Aggregate
    {
        get => _context.TryGetValue("playerAggregate", out PlayerAggregate? agg) ? agg : null;
        set => _context["playerAggregate"] = value!;
    }

    private EventPage MakeEventPage(IMessage evt, int seq)
    {
        var any = Any.Pack(evt, "type.googleapis.com/");
        return new EventPage
        {
            Sequence = (uint)seq,
            Event = any
        };
    }

    private EventBook MakeEventBook()
    {
        var book = new EventBook
        {
            Cover = new Cover
            {
                Domain = "player",
                Root = new UUID { Value = ByteString.CopyFromUtf8("player-123") }
            }
        };
        book.Pages.AddRange(Events);
        return book;
    }

    private void ExecuteCommand(IMessage cmd)
    {
        Error = null;
        ResultEvent = null;
        ResultEventAny = null;

        var eventBook = MakeEventBook();
        var agg = new PlayerAggregate();
        agg.Rehydrate(eventBook);
        Aggregate = agg;

        try
        {
            var result = agg.HandleCommand(cmd);
            ResultEvent = result;
            var any = Any.Pack(result, "type.googleapis.com/");
            ResultEventAny = any;
            Events.Add(MakeEventPage(result, Events.Count));
        }
        catch (CommandRejectedError e)
        {
            Error = e;
        }
    }

    // --- Given steps ---

    [Given(@"no prior events for the player aggregate")]
    public void GivenNoPriorEventsForThePlayerAggregate()
    {
        Events = new List<EventPage>();
    }

    [Given(@"a PlayerRegistered event for ""(.*)""")]
    public void GivenAPlayerRegisteredEventFor(string name)
    {
        if (Events == null) Events = new List<EventPage>();

        var evt = new PlayerRegistered
        {
            DisplayName = name,
            Email = $"{name.ToLower()}@example.com",
            PlayerType = PlayerType.Human,
            RegisteredAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        Events.Add(MakeEventPage(evt, Events.Count));
    }

    [Given(@"a FundsDeposited event with amount (\d+)")]
    public void GivenAFundsDepositedEventWithAmount(int amount)
    {
        if (Events == null) Events = new List<EventPage>();

        // Calculate prior balance
        long priorBalance = 0;
        foreach (var page in Events)
        {
            if (page.Event.TypeUrl.EndsWith("FundsDeposited"))
            {
                var evt = page.Event.Unpack<FundsDeposited>();
                if (evt.NewBalance != null)
                    priorBalance = evt.NewBalance.Amount;
            }
        }

        var newBalance = priorBalance + amount;
        var depositEvt = new FundsDeposited
        {
            Amount = new Currency { Amount = amount, CurrencyCode = "CHIPS" },
            NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
            DepositedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        Events.Add(MakeEventPage(depositEvt, Events.Count));
    }

    [Given(@"a FundsReserved event with amount (\d+) for table ""(.*)""")]
    public void GivenAFundsReservedEventWithAmountForTable(int amount, string tableId)
    {
        if (Events == null) Events = new List<EventPage>();

        // Calculate balances
        long totalDeposited = 0;
        long totalReserved = 0;
        foreach (var page in Events)
        {
            if (page.Event.TypeUrl.EndsWith("FundsDeposited"))
            {
                var evt = page.Event.Unpack<FundsDeposited>();
                if (evt.NewBalance != null)
                    totalDeposited = evt.NewBalance.Amount;
            }
            else if (page.Event.TypeUrl.EndsWith("FundsReserved"))
            {
                var evt = page.Event.Unpack<FundsReserved>();
                if (evt.NewReservedBalance != null)
                    totalReserved = evt.NewReservedBalance.Amount;
            }
        }

        var newReserved = totalReserved + amount;
        var newAvailable = totalDeposited - newReserved;

        var reserveEvt = new FundsReserved
        {
            Amount = new Currency { Amount = amount, CurrencyCode = "CHIPS" },
            TableRoot = ByteString.CopyFromUtf8(tableId),
            NewAvailableBalance = new Currency { Amount = newAvailable, CurrencyCode = "CHIPS" },
            NewReservedBalance = new Currency { Amount = newReserved, CurrencyCode = "CHIPS" },
            ReservedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        Events.Add(MakeEventPage(reserveEvt, Events.Count));
    }

    // --- When steps ---

    [When(@"I handle a RegisterPlayer command with name ""(.*)"" and email ""(.*)""")]
    public void WhenIHandleARegisterPlayerCommandWithNameAndEmail(string name, string email)
    {
        var cmd = new RegisterPlayer
        {
            DisplayName = name,
            Email = email,
            PlayerType = PlayerType.Human
        };
        ExecuteCommand(cmd);
    }

    [When(@"I handle a RegisterPlayer command with name ""(.*)"" and email ""(.*)"" as AI")]
    public void WhenIHandleARegisterPlayerCommandAsAI(string name, string email)
    {
        var cmd = new RegisterPlayer
        {
            DisplayName = name,
            Email = email,
            PlayerType = PlayerType.Ai,
            AiModelId = "gpt-4"
        };
        ExecuteCommand(cmd);
    }

    [When(@"I handle a DepositFunds command with amount (\d+)")]
    public void WhenIHandleADepositFundsCommandWithAmount(int amount)
    {
        var cmd = new DepositFunds
        {
            Amount = new Currency { Amount = amount, CurrencyCode = "CHIPS" }
        };
        ExecuteCommand(cmd);
    }

    [When(@"I handle a WithdrawFunds command with amount (\d+)")]
    public void WhenIHandleAWithdrawFundsCommandWithAmount(int amount)
    {
        var cmd = new WithdrawFunds
        {
            Amount = new Currency { Amount = amount, CurrencyCode = "CHIPS" }
        };
        ExecuteCommand(cmd);
    }

    [When(@"I handle a ReserveFunds command with amount (\d+) for table ""(.*)""")]
    public void WhenIHandleAReserveFundsCommandWithAmountForTable(int amount, string tableId)
    {
        var cmd = new ReserveFunds
        {
            Amount = new Currency { Amount = amount, CurrencyCode = "CHIPS" },
            TableRoot = ByteString.CopyFromUtf8(tableId)
        };
        ExecuteCommand(cmd);
    }

    [When(@"I handle a ReleaseFunds command for table ""(.*)""")]
    public void WhenIHandleAReleaseFundsCommandForTable(string tableId)
    {
        var cmd = new ReleaseFunds
        {
            TableRoot = ByteString.CopyFromUtf8(tableId)
        };
        ExecuteCommand(cmd);
    }

    [When(@"I rebuild the player state")]
    public void WhenIRebuildThePlayerState()
    {
        var eventBook = MakeEventBook();
        var agg = new PlayerAggregate();
        agg.Rehydrate(eventBook);
        Aggregate = agg;
    }

    // --- Then steps ---

    [Then(@"the result is a (PlayerRegistered|FundsDeposited|FundsWithdrawn|FundsReserved|FundsReleased) event")]
    public void ThenTheResultIsAEvent(string eventType)
    {
        Error.Should().BeNull($"Expected {eventType} event but got error: {Error?.Message}");
        ResultEventAny.Should().NotBeNull();
        ResultEventAny!.TypeUrl.Should().EndWith(eventType);
    }

    [Then(@"the player event has display_name ""(.*)""")]
    public void ThenThePlayerEventHasDisplayName(string name)
    {
        var evt = ResultEventAny!.Unpack<PlayerRegistered>();
        evt.DisplayName.Should().Be(name);
    }

    [Then(@"the player event has player_type ""(.*)""")]
    public void ThenThePlayerEventHasPlayerType(string playerType)
    {
        var evt = ResultEventAny!.Unpack<PlayerRegistered>();
        var expected = playerType.ToUpper() switch
        {
            "HUMAN" => PlayerType.Human,
            "AI" => PlayerType.Ai,
            _ => PlayerType.Unspecified
        };
        evt.PlayerType.Should().Be(expected);
    }

    [Then(@"the player event has amount (\d+)")]
    public void ThenThePlayerEventHasAmount(int amount)
    {
        var typeUrl = ResultEventAny!.TypeUrl;

        if (typeUrl.EndsWith("FundsDeposited"))
        {
            var evt = ResultEventAny.Unpack<FundsDeposited>();
            evt.Amount!.Amount.Should().Be(amount);
        }
        else if (typeUrl.EndsWith("FundsWithdrawn"))
        {
            var evt = ResultEventAny.Unpack<FundsWithdrawn>();
            evt.Amount!.Amount.Should().Be(amount);
        }
        else if (typeUrl.EndsWith("FundsReserved"))
        {
            var evt = ResultEventAny.Unpack<FundsReserved>();
            evt.Amount!.Amount.Should().Be(amount);
        }
        else if (typeUrl.EndsWith("FundsReleased"))
        {
            var evt = ResultEventAny.Unpack<FundsReleased>();
            evt.Amount!.Amount.Should().Be(amount);
        }
        else
        {
            throw new InvalidOperationException($"Unknown event type: {typeUrl}");
        }
    }

    [Then(@"the player event has new_balance (\d+)")]
    public void ThenThePlayerEventHasNewBalance(int balance)
    {
        var typeUrl = ResultEventAny!.TypeUrl;

        if (typeUrl.EndsWith("FundsDeposited"))
        {
            var evt = ResultEventAny.Unpack<FundsDeposited>();
            evt.NewBalance!.Amount.Should().Be(balance);
        }
        else if (typeUrl.EndsWith("FundsWithdrawn"))
        {
            var evt = ResultEventAny.Unpack<FundsWithdrawn>();
            evt.NewBalance!.Amount.Should().Be(balance);
        }
        else
        {
            throw new InvalidOperationException($"Unknown event type for new_balance: {typeUrl}");
        }
    }

    [Then(@"the player event has new_available_balance (\d+)")]
    public void ThenThePlayerEventHasNewAvailableBalance(int balance)
    {
        var typeUrl = ResultEventAny!.TypeUrl;

        if (typeUrl.EndsWith("FundsReserved"))
        {
            var evt = ResultEventAny.Unpack<FundsReserved>();
            evt.NewAvailableBalance!.Amount.Should().Be(balance);
        }
        else if (typeUrl.EndsWith("FundsReleased"))
        {
            var evt = ResultEventAny.Unpack<FundsReleased>();
            evt.NewAvailableBalance!.Amount.Should().Be(balance);
        }
        else
        {
            throw new InvalidOperationException($"Unknown event type for new_available_balance: {typeUrl}");
        }
    }


    [Then(@"the player state has bankroll (\d+)")]
    public void ThenThePlayerStateHasBankroll(int amount)
    {
        Aggregate.Should().NotBeNull();
        Aggregate!.Bankroll.Should().Be(amount);
    }

    [Then(@"the player state has reserved_funds (\d+)")]
    public void ThenThePlayerStateHasReservedFunds(int amount)
    {
        Aggregate.Should().NotBeNull();
        Aggregate!.ReservedFunds.Should().Be(amount);
    }

    [Then(@"the player state has available_balance (\d+)")]
    public void ThenThePlayerStateHasAvailableBalance(int amount)
    {
        Aggregate.Should().NotBeNull();
        Aggregate!.AvailableBalance.Should().Be(amount);
    }
}
