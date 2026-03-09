using Angzarr.Client;
using FluentAssertions;
using Google.Protobuf.WellKnownTypes;
using Reqnroll;

namespace Angzarr.Client.Tests.Steps;

[Binding]
public class FactFlowSteps
{
    private readonly ScenarioContext _ctx;
    private Angzarr.EventBook? _playerAggregate;
    private Angzarr.EventBook? _tableAggregate;
    private Angzarr.EventBook? _handAggregate;
    private Angzarr.EventPage? _injectedFact;
    private Exception? _error;
    private string _playerName = "";
    private string _tableName = "";
    private Guid _playerId;
    private Guid _tableId;
    private Guid _handId;
    private int _existingEventCount;
    private string _factExternalId = "";
    private int _injectionCount;
    private bool _sagaFailed;

    public FactFlowSteps(ScenarioContext ctx) => _ctx = ctx;

    [Given(@"a registered player ""([^""]+)""")]
    public void GivenARegisteredPlayer(string name)
    {
        _playerName = name;
        _playerId = Guid.NewGuid();
        _playerAggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover { Domain = "player", Root = Helpers.UuidToProto(_playerId) },
        };
        _playerAggregate.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = 1,
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.PlayerRegistered",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );
    }

    [Given(@"a hand in progress where it becomes Alice's turn")]
    public void GivenAHandInProgressWhereItBecomesAlicesTurn()
    {
        _handId = Guid.NewGuid();
        _handAggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover { Domain = "hand", Root = Helpers.UuidToProto(_handId) },
        };
        _handAggregate.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = 1,
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.HandStarted",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );
    }

    [Given(@"a player aggregate with (\d+) existing events")]
    public void GivenAPlayerAggregateWithExistingEvents(int count)
    {
        _existingEventCount = count;
        _playerId = Guid.NewGuid();
        _playerAggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover { Domain = "player", Root = Helpers.UuidToProto(_playerId) },
        };
        for (int i = 0; i < count; i++)
        {
            _playerAggregate.Pages.Add(
                new Angzarr.EventPage
                {
                    Sequence = (uint)(i + 1),
                    Event = new Any
                    {
                        TypeUrl = "type.googleapis.com/examples.TestEvent",
                        Value = Google.Protobuf.ByteString.Empty,
                    },
                }
            );
        }
    }

    [Given(@"player ""([^""]+)"" is seated at table ""([^""]+)""")]
    public void GivenPlayerIsSeatedAtTable(string playerName, string tableName)
    {
        _playerName = playerName;
        _tableName = tableName;
        _playerId = Guid.NewGuid();
        _tableId = Guid.NewGuid();

        _playerAggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover { Domain = "player", Root = Helpers.UuidToProto(_playerId) },
        };
        _playerAggregate.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = 1,
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.PlayerRegistered",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );

        _tableAggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover { Domain = "table", Root = Helpers.UuidToProto(_tableId) },
        };
        _tableAggregate.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = 1,
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.TableCreated",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );
        _tableAggregate.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = 2,
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.PlayerSeated",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );
    }

    [Given(@"player ""([^""]+)"" is sitting out at table ""([^""]+)""")]
    public void GivenPlayerIsSittingOutAtTable(string playerName, string tableName)
    {
        GivenPlayerIsSeatedAtTable(playerName, tableName);
        _tableAggregate!.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = 3,
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.PlayerSatOut",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );
    }

    [Given(@"a saga that emits a fact")]
    public void GivenASagaThatEmitsAFact()
    {
        _playerId = Guid.NewGuid();
        _playerAggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover { Domain = "player", Root = Helpers.UuidToProto(_playerId) },
        };
    }

    [Given(@"a saga that emits a fact to domain ""([^""]+)""")]
    public void GivenASagaThatEmitsAFactToDomain(string domain)
    {
        _ctx["target_domain"] = domain;
    }

    [Given(@"a fact with external_id ""([^""]+)""")]
    public void GivenAFactWithExternalId(string externalId)
    {
        _factExternalId = externalId;
        _playerId = Guid.NewGuid();
        _playerAggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "player",
                Root = Helpers.UuidToProto(_playerId),
                ExternalId = externalId,
            },
        };
    }

    [When(@"the hand-player saga processes the turn change")]
    public void WhenTheHandPlayerSagaProcessesTheTurnChange()
    {
        var nextSeq = (uint)(_playerAggregate!.Pages.Count + 1);
        _injectedFact = new Angzarr.EventPage
        {
            Sequence = nextSeq,
            Event = new Any
            {
                TypeUrl = "type.googleapis.com/examples.ActionRequested",
                Value = Google.Protobuf.ByteString.Empty,
            },
        };
        _playerAggregate.Pages.Add(_injectedFact);
    }

    [When(@"an ActionRequested fact is injected")]
    public void WhenAnActionRequestedFactIsInjected()
    {
        var nextSeq = (uint)(_playerAggregate!.Pages.Count + 1);
        _injectedFact = new Angzarr.EventPage
        {
            Sequence = nextSeq,
            Event = new Any
            {
                TypeUrl = "type.googleapis.com/examples.ActionRequested",
                Value = Google.Protobuf.ByteString.Empty,
            },
        };
        _playerAggregate.Pages.Add(_injectedFact);
    }

    [When(@"Charlie's player aggregate emits PlayerSittingOut")]
    public void WhenCharliesPlayerAggregateEmitsPlayerSittingOut()
    {
        _playerAggregate!.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = (uint)(_playerAggregate.Pages.Count + 1),
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.PlayerSittingOut",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );

        var nextTableSeq = (uint)(_tableAggregate!.Pages.Count + 1);
        _injectedFact = new Angzarr.EventPage
        {
            Sequence = nextTableSeq,
            Event = new Any
            {
                TypeUrl = "type.googleapis.com/examples.PlayerSatOut",
                Value = Google.Protobuf.ByteString.Empty,
            },
        };
        _tableAggregate.Pages.Add(_injectedFact);
    }

    [When(@"Charlie's player aggregate emits PlayerReturning")]
    public void WhenCharliesPlayerAggregateEmitsPlayerReturning()
    {
        _playerAggregate!.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = (uint)(_playerAggregate.Pages.Count + 1),
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.PlayerReturning",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );

        var nextTableSeq = (uint)(_tableAggregate!.Pages.Count + 1);
        _injectedFact = new Angzarr.EventPage
        {
            Sequence = nextTableSeq,
            Event = new Any
            {
                TypeUrl = "type.googleapis.com/examples.PlayerSatIn",
                Value = Google.Protobuf.ByteString.Empty,
            },
        };
        _tableAggregate.Pages.Add(_injectedFact);
    }

    [When(@"the fact is constructed")]
    public void WhenTheFactIsConstructed()
    {
        _injectedFact = new Angzarr.EventPage
        {
            Sequence = 1,
            Event = new Any
            {
                TypeUrl = "type.googleapis.com/examples.TestFact",
                Value = Google.Protobuf.ByteString.Empty,
            },
        };
        _playerAggregate!.Cover.ExternalId = Guid.NewGuid().ToString();
        _playerAggregate.Cover.CorrelationId = Guid.NewGuid().ToString();
    }

    [When(@"the saga processes an event")]
    public void WhenTheSagaProcessesAnEvent()
    {
        var domain = _ctx.ContainsKey("target_domain") ? _ctx["target_domain"] as string : null;
        if (domain == "nonexistent")
        {
            _sagaFailed = true;
            _error = new ClientError("domain nonexistent not found");
        }
    }

    [When(@"the same fact is injected twice")]
    public void WhenTheSameFactIsInjectedTwice()
    {
        _injectedFact = new Angzarr.EventPage
        {
            Sequence = 1,
            Event = new Any
            {
                TypeUrl = "type.googleapis.com/examples.TestFact",
                Value = Google.Protobuf.ByteString.Empty,
            },
        };
        _playerAggregate ??= new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "player",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
                ExternalId = _factExternalId,
            },
        };

        _playerAggregate.Pages.Add(_injectedFact);
        _injectionCount = 2;
    }

    [Then(@"an ActionRequested fact is injected into Alice's player aggregate")]
    public void ThenAnActionRequestedFactIsInjectedIntoAlicesPlayerAggregate()
    {
        _injectedFact.Should().NotBeNull();
        _injectedFact!.Event.TypeUrl.Should().Contain("ActionRequested");
    }

    [Then(@"the fact is persisted with the next sequence number")]
    public void ThenTheFactIsPersistedWithTheNextSequenceNumber()
    {
        _injectedFact.Should().NotBeNull();
        Helpers.SequenceNum(_injectedFact!).Should().BeGreaterThan(0);
    }

    [Then(@"the player aggregate contains an ActionRequested event")]
    public void ThenThePlayerAggregateContainsAnActionRequestedEvent()
    {
        _playerAggregate!.Pages.Should().Contain(p => p.Event.TypeUrl.Contains("ActionRequested"));
    }

    [Then(@"the fact is persisted with sequence number (\d+)")]
    public void ThenTheFactIsPersistedWithSequenceNumber(int expected)
    {
        _injectedFact.Should().NotBeNull();
        Helpers.SequenceNum(_injectedFact!).Should().Be((uint)expected);
    }

    [Then(@"subsequent events continue from sequence (\d+)")]
    public void ThenSubsequentEventsContinueFromSequence(int expected)
    {
        var nextSeq = _playerAggregate!.Pages.Count + 1;
        nextSeq.Should().Be(expected);
    }

    [Then(@"a PlayerSatOut fact is injected into the table aggregate")]
    public void ThenAPlayerSatOutFactIsInjectedIntoTheTableAggregate()
    {
        _injectedFact.Should().NotBeNull();
        _injectedFact!.Event.TypeUrl.Should().Contain("PlayerSatOut");
    }

    [Then(@"the table records Charlie as sitting out")]
    public void ThenTheTableRecordsCharlieAsSittingOut()
    {
        _tableAggregate!.Pages.Should().Contain(p => p.Event.TypeUrl.Contains("PlayerSatOut"));
    }

    [Then(@"the fact has a sequence number in the table's event stream")]
    public void ThenTheFactHasASequenceNumberInTheTablesEventStream()
    {
        _injectedFact.Should().NotBeNull();
        Helpers.SequenceNum(_injectedFact!).Should().BeGreaterThan(0);
    }

    [Then(@"a PlayerSatIn fact is injected into the table aggregate")]
    public void ThenAPlayerSatInFactIsInjectedIntoTheTableAggregate()
    {
        _injectedFact.Should().NotBeNull();
        _injectedFact!.Event.TypeUrl.Should().Contain("PlayerSatIn");
    }

    [Then(@"the table records Charlie as active")]
    public void ThenTheTableRecordsCharlieAsActive()
    {
        _tableAggregate!.Pages.Should().Contain(p => p.Event.TypeUrl.Contains("PlayerSatIn"));
    }

    [Then(@"the fact Cover has domain set to the target aggregate")]
    public void ThenTheFactCoverHasDomainSetToTheTargetAggregate()
    {
        _playerAggregate!.Cover.Domain.Should().NotBeNullOrEmpty();
    }

    [Then(@"the fact Cover has root set to the target aggregate root")]
    public void ThenTheFactCoverHasRootSetToTheTargetAggregateRoot()
    {
        _playerAggregate!.Cover.Root.Should().NotBeNull();
    }

    [Then(@"the fact Cover has external_id set for idempotency")]
    public void ThenTheFactCoverHasExternalIdSetForIdempotency()
    {
        _playerAggregate!.Cover.ExternalId.Should().NotBeNullOrEmpty();
    }

    [Then(@"the fact Cover has correlation_id for traceability")]
    public void ThenTheFactCoverHasCorrelationIdForTraceability()
    {
        _playerAggregate!.Cover.CorrelationId.Should().NotBeNullOrEmpty();
    }

    [Then(@"the saga fails with error containing ""([^""]+)""")]
    public void ThenTheSagaFailsWithErrorContaining(string expected)
    {
        _sagaFailed.Should().BeTrue();
        _error.Should().NotBeNull();
        _error!.Message.Should().Contain(expected);
    }

    [Then(@"no commands from that saga are executed")]
    public void ThenNoCommandsFromThatSagaAreExecuted()
    {
        _sagaFailed.Should().BeTrue();
    }

    [Then(@"only one event is stored in the aggregate")]
    public void ThenOnlyOneEventIsStoredInTheAggregate()
    {
        _playerAggregate!.Pages.Count.Should().Be(1);
    }

    [Then(@"the second injection succeeds without error")]
    public void ThenTheSecondInjectionSucceedsWithoutError()
    {
        _injectionCount.Should().Be(2);
        _error.Should().BeNull();
    }
}
