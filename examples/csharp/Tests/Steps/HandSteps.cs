using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using TechTalk.SpecFlow;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Hand.Agg;
using Tests.Support;

namespace Tests.Steps;

[Binding]
public class HandSteps
{
    private readonly TestContext _context;

    public HandSteps(TestContext context)
    {
        _context = context;
    }

    [Given(@"a new hand aggregate")]
    public void GivenANewHandAggregate()
    {
        _context.HandAggregate = new HandAggregate();
        _context.HandEventBook = new EventBook();
    }

    [Given(@"a dealt hand with players at positions (\d+) and (\d+)")]
    public void GivenADealtHandWithPlayersAtPositions(int pos1, int pos2)
    {
        var player1 = ByteString.CopyFromUtf8($"player_at_{pos1}");
        var player2 = ByteString.CopyFromUtf8($"player_at_{pos2}");

        var pc1 = new PlayerHoleCards { PlayerRoot = player1 };
        pc1.Cards.Add(new Card { Suit = Suit.Hearts, Rank = Rank.Ace });
        pc1.Cards.Add(new Card { Suit = Suit.Spades, Rank = Rank.King });

        var pc2 = new PlayerHoleCards { PlayerRoot = player2 };
        pc2.Cards.Add(new Card { Suit = Suit.Diamonds, Rank = Rank.Queen });
        pc2.Cards.Add(new Card { Suit = Suit.Clubs, Rank = Rank.Jack });

        var evt = new CardsDealt
        {
            TableRoot = ByteString.CopyFromUtf8("test_table"),
            HandNumber = 1,
            GameVariant = GameVariant.TexasHoldem,
            DealerPosition = pos1,
            DealtAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.Players.Add(new PlayerInHand { PlayerRoot = player1, Position = pos1, Stack = 1000 });
        evt.Players.Add(new PlayerInHand { PlayerRoot = player2, Position = pos2, Stack = 1000 });
        evt.PlayerCards.Add(pc1);
        evt.PlayerCards.Add(pc2);

        _context.AddHandEvent(evt);

        _context.HandAggregate = new HandAggregate();
        _context.HandAggregate.Rehydrate(_context.HandEventBook);
    }

    [When(@"I deal cards to (\d+) players")]
    public void WhenIDealCardsToPlayers(int playerCount)
    {
        var cmd = new DealCards
        {
            TableRoot = ByteString.CopyFromUtf8("test_table"),
            HandNumber = 1,
            GameVariant = GameVariant.TexasHoldem,
            DealerPosition = 0,
            SmallBlind = 5,
            BigBlind = 10
        };

        for (var i = 0; i < playerCount; i++)
        {
            cmd.Players.Add(new PlayerInHand
            {
                PlayerRoot = ByteString.CopyFromUtf8($"player_{i}"),
                Position = i,
                Stack = 1000
            });
        }

        ExecuteHandCommand(cmd);
    }

    [When(@"player at position (\d+) posts small blind of (\d+)")]
    public void WhenPlayerPostsSmallBlind(int position, long amount)
    {
        var playerRoot = ByteString.CopyFromUtf8($"player_at_{position}");

        var cmd = new PostBlind
        {
            PlayerRoot = playerRoot,
            BlindType = "small",
            Amount = amount
        };

        ExecuteHandCommand(cmd);
    }

    [When(@"player at position (\d+) posts big blind of (\d+)")]
    public void WhenPlayerPostsBigBlind(int position, long amount)
    {
        var playerRoot = ByteString.CopyFromUtf8($"player_at_{position}");

        var cmd = new PostBlind
        {
            PlayerRoot = playerRoot,
            BlindType = "big",
            Amount = amount
        };

        ExecuteHandCommand(cmd);
    }

    [When(@"player at position (\d+) folds")]
    public void WhenPlayerFolds(int position)
    {
        var playerRoot = ByteString.CopyFromUtf8($"player_at_{position}");

        var cmd = new PlayerAction
        {
            PlayerRoot = playerRoot,
            Action = ActionType.Fold,
            Amount = 0
        };

        ExecuteHandCommand(cmd);
    }

    [When(@"player at position (\d+) calls")]
    public void WhenPlayerCalls(int position)
    {
        var playerRoot = ByteString.CopyFromUtf8($"player_at_{position}");

        var cmd = new PlayerAction
        {
            PlayerRoot = playerRoot,
            Action = ActionType.Call,
            Amount = 10 // Assuming BB
        };

        ExecuteHandCommand(cmd);
    }

    [Then(@"a CardsDealt event should be emitted")]
    public void ThenACardsDealtEventShouldBeEmitted()
    {
        _context.LastEvent.Should().BeOfType<CardsDealt>();
    }

    [Then(@"a BlindPosted event should be emitted")]
    public void ThenABlindPostedEventShouldBeEmitted()
    {
        _context.LastEvent.Should().BeOfType<BlindPosted>();
    }

    [Then(@"an ActionTaken event should be emitted")]
    public void ThenAnActionTakenEventShouldBeEmitted()
    {
        _context.LastEvent.Should().BeOfType<ActionTaken>();
    }

    [Then(@"the action should be ""(.*)""")]
    public void ThenTheActionShouldBe(string action)
    {
        var evt = _context.LastEvent as ActionTaken;
        evt.Should().NotBeNull();

        var expectedAction = action.ToLower() switch
        {
            "fold" => ActionType.Fold,
            "check" => ActionType.Check,
            "call" => ActionType.Call,
            "bet" => ActionType.Bet,
            "raise" => ActionType.Raise,
            "all-in" => ActionType.AllIn,
            _ => ActionType.ActionUnspecified
        };
        evt!.Action.Should().Be(expectedAction);
    }

    private void ExecuteHandCommand(object cmd)
    {
        _context.LastException = null;
        _context.LastEvent = null;

        try
        {
            _context.HandAggregate ??= new HandAggregate();
            _context.HandAggregate.Rehydrate(_context.HandEventBook);

            var result = _context.HandAggregate.HandleCommand(cmd as Google.Protobuf.IMessage ?? throw new InvalidOperationException());
            _context.LastEvent = result;
            _context.AddHandEvent(result);
        }
        catch (CommandRejectedError ex)
        {
            _context.LastException = ex;
        }
    }
}
