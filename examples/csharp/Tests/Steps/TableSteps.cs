using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using TechTalk.SpecFlow;
using TechTalk.SpecFlow.Assist;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Table.Agg;

namespace Tests.Steps;

[Binding]
public class TableSteps
{
    private readonly ScenarioContext _context;

    public TableSteps(ScenarioContext context)
    {
        _context = context;
    }

    private List<EventPage> Events
    {
        get => _context.TryGetValue("tableEvents", out List<EventPage>? events) ? events! : new List<EventPage>();
        set => _context["tableEvents"] = value;
    }

    private Any? ResultEventAny
    {
        get => _context.TryGetValue("tableResultEventAny", out Any? evt) ? evt : null;
        set => _context["tableResultEventAny"] = value!;
    }

    private CommandRejectedError? Error
    {
        get => _context.TryGetValue("tableError", out CommandRejectedError? err) ? err : null;
        set => _context["tableError"] = value!;
    }

    private TableAggregate? Aggregate
    {
        get => _context.TryGetValue("tableAggregate", out TableAggregate? agg) ? agg : null;
        set => _context["tableAggregate"] = value!;
    }

    private int MinBuyIn
    {
        get => _context.TryGetValue("minBuyIn", out int val) ? val : 200;
        set => _context["minBuyIn"] = value;
    }

    private int MaxPlayers
    {
        get => _context.TryGetValue("maxPlayers", out int val) ? val : 9;
        set => _context["maxPlayers"] = value;
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
                Domain = "table",
                Root = new UUID { Value = ByteString.CopyFromUtf8("table-123") }
            }
        };
        book.Pages.AddRange(Events);
        return book;
    }

    private void ExecuteCommand(IMessage cmd)
    {
        Error = null;
        ResultEventAny = null;

        var eventBook = MakeEventBook();
        var agg = new TableAggregate();
        agg.Rehydrate(eventBook);
        Aggregate = agg;

        try
        {
            var result = agg.HandleCommand(cmd);
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

    [Given(@"no prior events for the table aggregate")]
    public void GivenNoPriorEventsForTheTableAggregate()
    {
        Events = new List<EventPage>();
        MinBuyIn = 200;
        MaxPlayers = 9;
    }

    [Given(@"a TableCreated event for ""(.*)""")]
    public void GivenATableCreatedEventFor(string name)
    {
        if (Events == null) Events = new List<EventPage>();

        var evt = new TableCreated
        {
            TableName = name,
            GameVariant = GameVariant.TexasHoldem,
            SmallBlind = 5,
            BigBlind = 10,
            MinBuyIn = MinBuyIn,
            MaxBuyIn = 1000,
            MaxPlayers = MaxPlayers,
            ActionTimeoutSeconds = 30,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        Events.Add(MakeEventPage(evt, Events.Count));
    }

    [Given(@"a TableCreated event for ""(.*)"" with min_buy_in (\d+)")]
    public void GivenATableCreatedEventWithMinBuyIn(string name, int minBuyIn)
    {
        MinBuyIn = minBuyIn;
        GivenATableCreatedEventFor(name);
    }

    [Given(@"a TableCreated event for ""(.*)"" with max_players (\d+)")]
    public void GivenATableCreatedEventWithMaxPlayers(string name, int maxPlayers)
    {
        MaxPlayers = maxPlayers;
        GivenATableCreatedEventFor(name);
    }

    [Given(@"a PlayerJoined event for player ""(.*)"" at seat (\d+)")]
    public void GivenAPlayerJoinedEventForPlayerAtSeat(string playerId, int seat)
    {
        GivenAPlayerJoinedEventForPlayerAtSeatWithStack(playerId, seat, 500);
    }

    [Given(@"a PlayerJoined event for player ""(.*)"" at seat (\d+) with stack (\d+)")]
    public void GivenAPlayerJoinedEventForPlayerAtSeatWithStack(string playerId, int seat, int stack)
    {
        if (Events == null) Events = new List<EventPage>();

        var evt = new PlayerJoined
        {
            PlayerRoot = ByteString.CopyFromUtf8(playerId),
            SeatPosition = seat,
            BuyInAmount = stack,
            Stack = stack,
            JoinedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        Events.Add(MakeEventPage(evt, Events.Count));
    }

    [Given(@"a HandStarted event for hand (\d+)")]
    public void GivenAHandStartedEventForHand(int handNumber)
    {
        GivenAHandStartedEventForHandWithDealerAtSeat(handNumber, 0);
    }

    [Given(@"a HandStarted event for hand (\d+) with dealer at seat (\d+)")]
    public void GivenAHandStartedEventForHandWithDealerAtSeat(int handNumber, int dealerSeat)
    {
        if (Events == null) Events = new List<EventPage>();

        var evt = new HandStarted
        {
            HandNumber = handNumber,
            HandRoot = ByteString.CopyFromUtf8($"hand-{handNumber}"),
            DealerPosition = dealerSeat,
            GameVariant = GameVariant.TexasHoldem,
            SmallBlind = 5,
            BigBlind = 10,
            StartedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        Events.Add(MakeEventPage(evt, Events.Count));
    }

    [Given(@"a HandEnded event for hand (\d+)")]
    public void GivenAHandEndedEventForHand(int handNumber)
    {
        if (Events == null) Events = new List<EventPage>();

        var evt = new HandEnded
        {
            HandRoot = ByteString.CopyFromUtf8($"hand_{handNumber}"),
            EndedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        Events.Add(MakeEventPage(evt, Events.Count));
    }

    // --- When steps ---

    [When(@"I handle a CreateTable command with name ""(.*)"" and variant ""(.*)"":")]
    public void WhenIHandleACreateTableCommandWithNameAndVariant(string name, string variant, TechTalk.SpecFlow.Table table)
    {
        var row = table.Rows[0];
        var gameVariant = variant.ToUpper() switch
        {
            "TEXAS_HOLDEM" => GameVariant.TexasHoldem,
            "OMAHA" => GameVariant.Omaha,
            "FIVE_CARD_DRAW" => GameVariant.FiveCardDraw,
            _ => GameVariant.Unspecified
        };

        var cmd = new CreateTable
        {
            TableName = name,
            GameVariant = gameVariant,
            SmallBlind = int.Parse(row["small_blind"]),
            BigBlind = int.Parse(row["big_blind"]),
            MinBuyIn = int.Parse(row["min_buy_in"]),
            MaxBuyIn = int.Parse(row["max_buy_in"]),
            MaxPlayers = int.Parse(row["max_players"])
        };
        ExecuteCommand(cmd);
    }

    [When(@"I handle a JoinTable command for player ""(.*)"" at seat (-?\d+) with buy-in (\d+)")]
    public void WhenIHandleAJoinTableCommandForPlayerAtSeatWithBuyIn(string playerId, int seat, int buyIn)
    {
        var cmd = new JoinTable
        {
            PlayerRoot = ByteString.CopyFromUtf8(playerId),
            PreferredSeat = seat,
            BuyInAmount = buyIn
        };
        ExecuteCommand(cmd);
    }

    [When(@"I handle a LeaveTable command for player ""(.*)""")]
    public void WhenIHandleALeaveTableCommandForPlayer(string playerId)
    {
        var cmd = new LeaveTable
        {
            PlayerRoot = ByteString.CopyFromUtf8(playerId)
        };
        ExecuteCommand(cmd);
    }

    [When(@"I handle a StartHand command")]
    public void WhenIHandleAStartHandCommand()
    {
        var cmd = new StartHand();
        ExecuteCommand(cmd);
    }

    [When(@"I handle an EndHand command with winner ""(.*)"" winning (\d+)")]
    public void WhenIHandleAnEndHandCommandWithWinnerWinning(string winnerId, int amount)
    {
        var cmd = new EndHand
        {
            HandRoot = ByteString.CopyFromUtf8("current_hand")
        };
        cmd.Results.Add(new PotResult
        {
            PotType = "main",
            WinnerRoot = ByteString.CopyFromUtf8(winnerId),
            Amount = amount
        });
        ExecuteCommand(cmd);
    }

    [When(@"I rebuild the table state")]
    public void WhenIRebuildTheTableState()
    {
        var eventBook = MakeEventBook();
        var agg = new TableAggregate();
        agg.Rehydrate(eventBook);
        Aggregate = agg;
    }

    // --- Then steps ---

    [Then(@"the result is a (TableCreated|PlayerJoined|PlayerLeft|HandStarted|HandEnded) event")]
    public void ThenTheResultIsATableEvent(string eventType)
    {
        Error.Should().BeNull($"Expected {eventType} event but got error: {Error?.Message}");
        ResultEventAny.Should().NotBeNull();
        ResultEventAny!.TypeUrl.Should().EndWith(eventType);
    }

    [Then(@"the table event has table_name ""(.*)""")]
    public void ThenTheTableEventHasTableName(string name)
    {
        var evt = ResultEventAny!.Unpack<TableCreated>();
        evt.TableName.Should().Be(name);
    }

    [Then(@"the table event has game_variant ""(.*)""")]
    public void ThenTheTableEventHasGameVariant(string variant)
    {
        var evt = ResultEventAny!.Unpack<TableCreated>();
        var expected = variant.ToUpper() switch
        {
            "TEXAS_HOLDEM" => GameVariant.TexasHoldem,
            "OMAHA" => GameVariant.Omaha,
            "FIVE_CARD_DRAW" => GameVariant.FiveCardDraw,
            _ => GameVariant.Unspecified
        };
        evt.GameVariant.Should().Be(expected);
    }

    [Then(@"the table event has small_blind (\d+)")]
    public void ThenTheTableEventHasSmallBlind(int amount)
    {
        var evt = ResultEventAny!.Unpack<TableCreated>();
        evt.SmallBlind.Should().Be(amount);
    }

    [Then(@"the table event has big_blind (\d+)")]
    public void ThenTheTableEventHasBigBlind(int amount)
    {
        var evt = ResultEventAny!.Unpack<TableCreated>();
        evt.BigBlind.Should().Be(amount);
    }

    [Then(@"the table event has seat_position (\d+)")]
    public void ThenTheTableEventHasSeatPosition(int position)
    {
        var evt = ResultEventAny!.Unpack<PlayerJoined>();
        evt.SeatPosition.Should().Be(position);
    }

    [Then(@"the table event has buy_in_amount (\d+)")]
    public void ThenTheTableEventHasBuyInAmount(int amount)
    {
        var evt = ResultEventAny!.Unpack<PlayerJoined>();
        evt.BuyInAmount.Should().Be(amount);
    }

    [Then(@"the table event has chips_cashed_out (\d+)")]
    public void ThenTheTableEventHasChipsCashedOut(int amount)
    {
        var evt = ResultEventAny!.Unpack<PlayerLeft>();
        evt.ChipsCashedOut.Should().Be(amount);
    }

    [Then(@"the table event has hand_number (\d+)")]
    public void ThenTheTableEventHasHandNumber(int number)
    {
        var evt = ResultEventAny!.Unpack<HandStarted>();
        evt.HandNumber.Should().Be(number);
    }

    [Then(@"the table event has (\d+) active_players")]
    public void ThenTheTableEventHasActivePlayers(int count)
    {
        var evt = ResultEventAny!.Unpack<HandStarted>();
        evt.ActivePlayers.Count.Should().Be(count);
    }

    [Then(@"the table event has dealer_position (\d+)")]
    public void ThenTheTableEventHasDealerPosition(int position)
    {
        var evt = ResultEventAny!.Unpack<HandStarted>();
        evt.DealerPosition.Should().Be(position);
    }

    [Then(@"player ""(.*)"" stack change is (\d+)")]
    public void ThenPlayerStackChangeIs(string playerId, int amount)
    {
        var evt = ResultEventAny!.Unpack<HandEnded>();
        // StackChanges is a map<string, long> where key is player_root_hex
        var playerKey = Convert.ToHexString(System.Text.Encoding.UTF8.GetBytes(playerId)).ToLowerInvariant();
        evt.StackChanges.Should().ContainKey(playerKey);
        evt.StackChanges[playerKey].Should().Be(amount);
    }

    // Reuse shared Then steps
    [Then(@"the command fails with status ""(.*)""")]
    public void ThenTheTableCommandFailsWithStatus(string status)
    {
        Error.Should().NotBeNull("Expected command to fail but it succeeded");
    }

    [Then(@"the error message contains ""(.*)""")]
    public void ThenTheTableErrorMessageContains(string text)
    {
        Error.Should().NotBeNull("Expected an error but got success");
        Error!.Message.ToLower().Should().Contain(text.ToLower());
    }

    // State assertions
    [Then(@"the table state has (\d+) players")]
    public void ThenTheTableStateHasPlayers(int count)
    {
        Aggregate.Should().NotBeNull();
        Aggregate!.PlayerCount.Should().Be(count);
    }

    [Then(@"the table state has seat (\d+) occupied by ""(.*)""")]
    public void ThenTheTableStateHasSeatOccupiedBy(int seat, string playerId)
    {
        Aggregate.Should().NotBeNull();
        Aggregate!.GetSeatOccupant(seat).Should().Be(playerId);
    }

    [Then(@"the table state has status ""(.*)""")]
    public void ThenTheTableStateHasStatus(string status)
    {
        Aggregate.Should().NotBeNull();
        Aggregate!.Status.Should().Be(status);
    }

    [Then(@"the table state has hand_count (\d+)")]
    public void ThenTheTableStateHasHandCount(int count)
    {
        Aggregate.Should().NotBeNull();
        Aggregate!.HandCount.Should().Be(count);
    }
}
