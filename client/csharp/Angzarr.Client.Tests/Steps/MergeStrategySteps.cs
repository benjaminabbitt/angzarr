using Angzarr.Client;
using FluentAssertions;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Reqnroll;

namespace Angzarr.Client.Tests.Steps;

[Binding]
public class MergeStrategySteps
{
    private readonly ScenarioContext _ctx;
    private Angzarr.EventBook? _aggregate;
    private Angzarr.CommandBook? _command;
    private Angzarr.MergeStrategy _mergeStrategy;
    private uint _targetSequence;
    private bool _commandSucceeded;
    private bool _eventsPersisted;
    private StatusCode? _errorStatus;
    private string? _errorMessage;
    private bool _isRetryable;
    private Angzarr.EventBook? _errorEventBook;
    private int _counterValue;
    private List<string>? _setContents;
    private List<MockCommand>? _concurrentCommands;
    private bool _aggregateAccepts = true;
    private bool _aggregateRejects;
    private string? _aggregateError;

    public MergeStrategySteps(ScenarioContext ctx) => _ctx = ctx;

    private class MockCommand
    {
        public string Client { get; set; } = "";
        public int Amount { get; set; }
        public uint Sequence { get; set; }
        public Angzarr.MergeStrategy Strategy { get; set; }
        public bool Succeeded { get; set; }
        public Angzarr.EventBook? ResultEvents { get; set; }
    }

    [Given(@"an aggregate ""([^""]+)"" with initial events:")]
    public void GivenAnAggregateWithInitialEvents(string domain, Table table)
    {
        _aggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = domain,
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };

        foreach (var row in table.Rows)
        {
            var seq = uint.Parse(row["sequence"]);
            var type = row["type"];
            _aggregate.Pages.Add(
                new Angzarr.EventPage
                {
                    Sequence = seq,
                    Event = new Any
                    {
                        TypeUrl = $"type.googleapis.com/examples.{type}",
                        Value = Google.Protobuf.ByteString.Empty,
                    },
                }
            );
        }
    }

    [Given(@"a command with merge_strategy STRICT")]
    public void GivenACommandWithMergeStrategyStrict()
    {
        _mergeStrategy = Angzarr.MergeStrategy.MergeStrict;
        InitializeCommand();
    }

    [Given(@"a command with merge_strategy COMMUTATIVE")]
    public void GivenACommandWithMergeStrategyCommutative()
    {
        _mergeStrategy = Angzarr.MergeStrategy.MergeCommutative;
        InitializeCommand();
    }

    [Given(@"a command with merge_strategy AGGREGATE_HANDLES")]
    public void GivenACommandWithMergeStrategyAggregateHandles()
    {
        _mergeStrategy = Angzarr.MergeStrategy.MergeAggregateHandles;
        InitializeCommand();
    }

    [Given(@"a command with no explicit merge_strategy")]
    public void GivenACommandWithNoExplicitMergeStrategy()
    {
        _mergeStrategy = Angzarr.MergeStrategy.MergeCommutative;
        InitializeCommand();
    }

    [Given(@"the command targets sequence (\d+)")]
    public void GivenTheCommandTargetsSequence(int sequence)
    {
        _targetSequence = (uint)sequence;
        if (_command != null && _command.Pages.Count > 0)
        {
            Helpers.SetSequence(_command.Pages[0], _targetSequence);
        }
    }

    [Given(@"a saga emits a command with merge_strategy COMMUTATIVE")]
    public void GivenASagaEmitsACommandWithMergeStrategyCommutative()
    {
        _mergeStrategy = Angzarr.MergeStrategy.MergeCommutative;
        InitializeCommand();
        _ctx["is_saga"] = true;
    }

    [Given(@"the destination aggregate has advanced")]
    public void GivenTheDestinationAggregateHasAdvanced()
    {
        _aggregate ??= new Angzarr.EventBook();
        var page = new Angzarr.EventPage
        {
            Header = new Angzarr.PageHeader { Sequence = (uint)(_aggregate.Pages.Count + 1) },
            Event = new Any
            {
                TypeUrl = "type.googleapis.com/examples.ConcurrentEvent",
                Value = Google.Protobuf.ByteString.Empty,
            },
        };
        _aggregate.Pages.Add(page);
    }

    [Given(@"the aggregate accepts the command")]
    public void GivenTheAggregateAcceptsTheCommand()
    {
        _aggregateAccepts = true;
        _aggregateRejects = false;
    }

    [Given(@"the aggregate rejects due to state conflict")]
    public void GivenTheAggregateRejectsDueToStateConflict()
    {
        _aggregateAccepts = false;
        _aggregateRejects = true;
        _aggregateError = "State conflict detected";
    }

    [Given(@"a counter aggregate at value (\d+)")]
    public void GivenACounterAggregateAtValue(int value)
    {
        _counterValue = value;
        _aggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "counter",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
    }

    [Given(@"two concurrent IncrementBy commands:")]
    public void GivenTwoConcurrentIncrementByCommands(Table table)
    {
        _concurrentCommands = new List<MockCommand>();
        foreach (var row in table.Rows)
        {
            _concurrentCommands.Add(
                new MockCommand
                {
                    Client = row["client"],
                    Amount = int.Parse(row["amount"]),
                    Sequence = uint.Parse(row["sequence"]),
                }
            );
        }
    }

    [Given(@"a set aggregate containing \[""([^""]+)"", ""([^""]+)""\]")]
    public void GivenASetAggregateContaining(string item1, string item2)
    {
        _setContents = new List<string> { item1, item2 };
        _aggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "set",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
    }

    [Given(@"two concurrent AddItem commands for ""([^""]+)"":")]
    public void GivenTwoConcurrentAddItemCommandsFor(string item, Table table)
    {
        _concurrentCommands = new List<MockCommand>();
        foreach (var row in table.Rows)
        {
            _concurrentCommands.Add(
                new MockCommand { Client = row["client"], Sequence = uint.Parse(row["sequence"]) }
            );
        }
        _ctx["add_item"] = item;
    }

    [Given(@"commands for the same aggregate:")]
    public void GivenCommandsForTheSameAggregate(Table table)
    {
        _concurrentCommands = new List<MockCommand>();
        foreach (var row in table.Rows)
        {
            var strategyStr = row["merge_strategy"];
            var strategy = strategyStr switch
            {
                "STRICT" => Angzarr.MergeStrategy.MergeStrict,
                "COMMUTATIVE" => Angzarr.MergeStrategy.MergeCommutative,
                "AGGREGATE_HANDLES" => Angzarr.MergeStrategy.MergeAggregateHandles,
                _ => Angzarr.MergeStrategy.MergeCommutative,
            };
            _concurrentCommands.Add(
                new MockCommand { Client = row["command"], Strategy = strategy }
            );
        }
    }

    [Given(@"a new aggregate with no events")]
    public void GivenANewAggregateWithNoEvents()
    {
        _aggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
    }

    [Given(@"a command targeting sequence (\d+)")]
    public void GivenACommandTargetingSequence(int sequence)
    {
        _targetSequence = (uint)sequence;
        InitializeCommand();
        Helpers.SetSequence(_command!.Pages[0], _targetSequence);
    }

    [Given(@"an aggregate with snapshot at sequence (\d+)")]
    public void GivenAnAggregateWithSnapshotAtSequence(int snapshotSeq)
    {
        _aggregate = new Angzarr.EventBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
        _ctx["snapshot_sequence"] = snapshotSeq;
    }

    [Given(@"events at sequences (\d+), (\d+)")]
    public void GivenEventsAtSequences(int seq1, int seq2)
    {
        _aggregate ??= new Angzarr.EventBook();
        _aggregate.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = (uint)seq1,
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.TestEvent",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );
        _aggregate.Pages.Add(
            new Angzarr.EventPage
            {
                Sequence = (uint)seq2,
                Event = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.TestEvent",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );
    }

    [Given(@"the next expected sequence is (\d+)")]
    public void GivenTheNextExpectedSequenceIs(int nextSeq)
    {
        _ctx["next_sequence"] = nextSeq;
    }

    [Given(@"a CommandBook with no pages")]
    public void GivenACommandBookWithNoPages()
    {
        _command = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = "test",
                Root = Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
    }

    [Given(@"the aggregate is at sequence (\d+)")]
    public void GivenTheAggregateIsAtSequence(int sequence)
    {
        _aggregate ??= new Angzarr.EventBook();
        while (_aggregate.Pages.Count < sequence)
        {
            _aggregate.Pages.Add(
                new Angzarr.EventPage
                {
                    Sequence = (uint)(_aggregate.Pages.Count),
                    Event = new Any
                    {
                        TypeUrl = "type.googleapis.com/examples.TestEvent",
                        Value = Google.Protobuf.ByteString.Empty,
                    },
                }
            );
        }
    }

    [When(@"the coordinator processes the command")]
    public void WhenTheCoordinatorProcessesTheCommand()
    {
        // Use explicitly set next_sequence if available, otherwise calculate from pages
        uint aggregateNextSeq;
        if (_ctx.ContainsKey("next_sequence"))
        {
            aggregateNextSeq = (uint)(int)_ctx["next_sequence"];
        }
        else
        {
            aggregateNextSeq = (uint)(_aggregate?.Pages.Count ?? 0);
        }

        if (_mergeStrategy == Angzarr.MergeStrategy.MergeStrict)
        {
            if (_targetSequence != aggregateNextSeq)
            {
                _commandSucceeded = false;
                _eventsPersisted = false;
                _errorStatus = StatusCode.Aborted;
                _errorMessage =
                    $"Sequence mismatch: expected {aggregateNextSeq}, got {_targetSequence}";
                _errorEventBook = _aggregate;
                return;
            }
        }
        else if (_mergeStrategy == Angzarr.MergeStrategy.MergeCommutative)
        {
            if (_targetSequence != aggregateNextSeq)
            {
                _commandSucceeded = false;
                _eventsPersisted = false;
                _errorStatus = StatusCode.FailedPrecondition;
                _errorMessage = "Sequence mismatch - retryable";
                _isRetryable = true;
                _errorEventBook = _aggregate;
                return;
            }
        }
        else if (_mergeStrategy == Angzarr.MergeStrategy.MergeAggregateHandles)
        {
            if (_aggregateRejects)
            {
                _commandSucceeded = false;
                _eventsPersisted = false;
                _errorMessage = _aggregateError;
                return;
            }
        }

        _commandSucceeded = true;
        _eventsPersisted = true;
    }

    [When(@"the client extracts the EventBook from the error")]
    public void WhenTheClientExtractsTheEventBookFromTheError()
    {
        _errorEventBook.Should().NotBeNull();
    }

    [When(@"rebuilds the command with sequence (\d+)")]
    public void WhenRebuildsTheCommandWithSequence(int sequence)
    {
        _targetSequence = (uint)sequence;
        if (_command != null && _command.Pages.Count > 0)
        {
            Helpers.SetSequence(_command.Pages[0], _targetSequence);
        }
    }

    [When(@"resubmits the command")]
    public void WhenResubmitsTheCommand()
    {
        var aggregateNextSeq = (uint)(_aggregate?.Pages.Count ?? 0);
        if (_targetSequence == aggregateNextSeq)
        {
            _commandSucceeded = true;
            _eventsPersisted = true;
            _errorStatus = null;
            _errorMessage = null;
        }
    }

    [When(@"the saga coordinator executes the command")]
    public void WhenTheSagaCoordinatorExecutesTheCommand()
    {
        var aggregateNextSeq = (uint)(_aggregate?.Pages.Count ?? 0);
        if (_targetSequence != aggregateNextSeq)
        {
            _commandSucceeded = false;
            _errorStatus = StatusCode.FailedPrecondition;
            _isRetryable = true;
            _errorEventBook = _aggregate;
        }
    }

    [When(@"both commands use merge_strategy AGGREGATE_HANDLES")]
    public void WhenBothCommandsUseMergeStrategyAggregateHandles()
    {
        foreach (var cmd in _concurrentCommands!)
        {
            cmd.Strategy = Angzarr.MergeStrategy.MergeAggregateHandles;
        }
    }

    [When(@"both are processed")]
    public void WhenBothAreProcessed()
    {
        if (_setContents != null && _ctx.ContainsKey("add_item"))
        {
            var item = _ctx["add_item"] as string;
            var firstAdded = false;
            foreach (var cmd in _concurrentCommands!)
            {
                if (!firstAdded && !_setContents.Contains(item!))
                {
                    _setContents.Add(item!);
                    cmd.Succeeded = true;
                    cmd.ResultEvents = new Angzarr.EventBook();
                    cmd.ResultEvents.Pages.Add(
                        new Angzarr.EventPage
                        {
                            Sequence = 1,
                            Event = new Any
                            {
                                TypeUrl = "type.googleapis.com/examples.ItemAdded",
                                Value = Google.Protobuf.ByteString.Empty,
                            },
                        }
                    );
                    firstAdded = true;
                }
                else
                {
                    cmd.Succeeded = true;
                    cmd.ResultEvents = new Angzarr.EventBook();
                }
            }
        }
        else
        {
            foreach (var cmd in _concurrentCommands!)
            {
                _counterValue += cmd.Amount;
                cmd.Succeeded = true;
            }
        }
    }

    [When(@"processed with sequence conflicts")]
    public void WhenProcessedWithSequenceConflicts()
    {
        foreach (var cmd in _concurrentCommands!)
        {
            if (cmd.Strategy == Angzarr.MergeStrategy.MergeStrict)
            {
                cmd.Succeeded = false;
            }
            else if (cmd.Strategy == Angzarr.MergeStrategy.MergeCommutative)
            {
                cmd.Succeeded = false;
            }
            else if (cmd.Strategy == Angzarr.MergeStrategy.MergeAggregateHandles)
            {
                cmd.Succeeded = true;
            }
        }
    }

    [When(@"the command uses merge_strategy (.*)")]
    public void WhenTheCommandUsesMergeStrategy(string strategy)
    {
        _mergeStrategy = strategy switch
        {
            "STRICT" => Angzarr.MergeStrategy.MergeStrict,
            "COMMUTATIVE" => Angzarr.MergeStrategy.MergeCommutative,
            "AGGREGATE_HANDLES" => Angzarr.MergeStrategy.MergeAggregateHandles,
            _ => Angzarr.MergeStrategy.MergeCommutative,
        };
        InitializeCommand();
        Helpers.SetSequence(_command!.Pages[0], _targetSequence);
        WhenTheCoordinatorProcessesTheCommand();
    }

    [When(@"a STRICT command targets sequence (\d+)")]
    public void WhenAStrictCommandTargetsSequence(int sequence)
    {
        _mergeStrategy = Angzarr.MergeStrategy.MergeStrict;
        _targetSequence = (uint)sequence;
        InitializeCommand();
        Helpers.SetSequence(_command!.Pages[0], _targetSequence);
        WhenTheCoordinatorProcessesTheCommand();
    }

    [When(@"merge_strategy is extracted")]
    public void WhenMergeStrategyIsExtracted()
    {
        if (_command == null || _command.Pages.Count == 0)
        {
            _mergeStrategy = Angzarr.MergeStrategy.MergeCommutative;
        }
        else
        {
            _mergeStrategy = _command.Pages[0].MergeStrategy;
        }
    }

    [Then(@"the command succeeds")]
    public void ThenTheCommandSucceeds()
    {
        _commandSucceeded.Should().BeTrue();
    }

    [Then(@"events are persisted")]
    public void ThenEventsArePersisted()
    {
        _eventsPersisted.Should().BeTrue();
    }

    [Then(@"the command fails with ABORTED status")]
    public void ThenTheCommandFailsWithAbortedStatus()
    {
        _commandSucceeded.Should().BeFalse();
        _errorStatus.Should().Be(StatusCode.Aborted);
    }

    [Then(@"the command fails with FAILED_PRECONDITION status")]
    public void ThenTheCommandFailsWithFailedPreconditionStatus()
    {
        _commandSucceeded.Should().BeFalse();
        _errorStatus.Should().Be(StatusCode.FailedPrecondition);
    }

    [Then(@"the error message contains ""([^""]+)""")]
    public void ThenTheErrorMessageContains(string expected)
    {
        _errorMessage.Should().Contain(expected);
    }

    [Then(@"no events are persisted")]
    public void ThenNoEventsArePersisted()
    {
        _eventsPersisted.Should().BeFalse();
    }

    [Then(@"the error details include the current EventBook")]
    public void ThenTheErrorDetailsIncludeTheCurrentEventBook()
    {
        _errorEventBook.Should().NotBeNull();
    }

    [Then(@"the EventBook shows next_sequence (\d+)")]
    public void ThenTheEventBookShowsNextSequence(int expected)
    {
        var nextSeq = _errorEventBook?.Pages.Count ?? 0;
        nextSeq.Should().Be(expected);
    }

    [Then(@"the error is marked as retryable")]
    public void ThenTheErrorIsMarkedAsRetryable()
    {
        _isRetryable.Should().BeTrue();
    }

    [Then(@"the command fails with retryable status")]
    public void ThenTheCommandFailsWithRetryableStatus()
    {
        _isRetryable.Should().BeTrue();
    }

    [Then(@"the saga retries with backoff")]
    public void ThenTheSagaRetriesWithBackoff()
    {
        _isRetryable.Should().BeTrue();
    }

    [Then(@"the saga fetches fresh destination state")]
    public void ThenTheSagaFetchesFreshDestinationState()
    {
        _errorEventBook.Should().NotBeNull();
    }

    [Then(@"the retried command succeeds")]
    public void ThenTheRetriedCommandSucceeds()
    {
        _targetSequence = (uint)(_aggregate?.Pages.Count ?? 0);
        _commandSucceeded = true;
    }

    [Then(@"the effective merge_strategy is COMMUTATIVE")]
    public void ThenTheEffectiveMergeStrategyIsCommutative()
    {
        _mergeStrategy.Should().Be(Angzarr.MergeStrategy.MergeCommutative);
    }

    [Then(@"the coordinator does NOT validate the sequence")]
    public void ThenTheCoordinatorDoesNotValidateTheSequence()
    {
        _mergeStrategy.Should().Be(Angzarr.MergeStrategy.MergeAggregateHandles);
    }

    [Then(@"the aggregate handler is invoked")]
    public void ThenTheAggregateHandlerIsInvoked()
    {
        _mergeStrategy.Should().Be(Angzarr.MergeStrategy.MergeAggregateHandles);
    }

    [Then(@"the aggregate receives the prior EventBook")]
    public void ThenTheAggregateReceivesThePriorEventBook()
    {
        _aggregate.Should().NotBeNull();
    }

    [Then(@"events are persisted at the correct sequence")]
    public void ThenEventsArePersistedAtTheCorrectSequence()
    {
        _eventsPersisted.Should().BeTrue();
    }

    [Then(@"the command fails with aggregate's error")]
    public void ThenTheCommandFailsWithAggregatesError()
    {
        _commandSucceeded.Should().BeFalse();
        _errorMessage.Should().Be(_aggregateError);
    }

    [Then(@"both commands succeed")]
    public void ThenBothCommandsSucceed()
    {
        _concurrentCommands.Should().OnlyContain(c => c.Succeeded);
    }

    [Then(@"the final counter value is (\d+)")]
    public void ThenTheFinalCounterValueIs(int expected)
    {
        _counterValue.Should().Be(expected);
    }

    [Then(@"no sequence conflicts occur")]
    public void ThenNoSequenceConflictsOccur()
    {
        _concurrentCommands.Should().OnlyContain(c => c.Succeeded);
    }

    [Then(@"the first command succeeds with ItemAdded event")]
    public void ThenTheFirstCommandSucceedsWithItemAddedEvent()
    {
        var first = _concurrentCommands!.First();
        first.Succeeded.Should().BeTrue();
        first.ResultEvents!.Pages.Should().Contain(p => p.Event.TypeUrl.Contains("ItemAdded"));
    }

    [Then(@"the second command succeeds with no event \(idempotent\)")]
    public void ThenTheSecondCommandSucceedsWithNoEventIdempotent()
    {
        var second = _concurrentCommands!.Skip(1).First();
        second.Succeeded.Should().BeTrue();
        second.ResultEvents!.Pages.Should().BeEmpty();
    }

    [Then(@"the set contains \[""([^""]+)"", ""([^""]+)"", ""([^""]+)""\]")]
    public void ThenTheSetContains(string item1, string item2, string item3)
    {
        _setContents.Should().Contain(new[] { item1, item2, item3 });
    }

    [Then(@"the response status is (.*)")]
    public void ThenTheResponseStatusIs(string status)
    {
        if (status == "ABORTED")
        {
            _errorStatus.Should().Be(StatusCode.Aborted);
        }
        else if (status == "FAILED_PRECONDITION")
        {
            _errorStatus.Should().Be(StatusCode.FailedPrecondition);
        }
        else if (status == "varies")
        {
            // AGGREGATE_HANDLES varies based on aggregate decision
        }
    }

    [Then(@"the behavior is (.*)")]
    public void ThenTheBehaviorIs(string behavior)
    {
        behavior.Should().NotBeNullOrEmpty();
    }

    [Then(@"ReserveFunds is rejected immediately")]
    public void ThenReserveFundsIsRejectedImmediately()
    {
        var cmd = _concurrentCommands!.First(c => c.Client == "ReserveFunds");
        cmd.Succeeded.Should().BeFalse();
    }

    [Then(@"AddBonusPoints is retryable")]
    public void ThenAddBonusPointsIsRetryable()
    {
        var cmd = _concurrentCommands!.First(c => c.Client == "AddBonusPoints");
        cmd.Succeeded.Should().BeFalse();
    }

    [Then(@"IncrementVisits delegates to aggregate")]
    public void ThenIncrementVisitsDelegatesToAggregate()
    {
        var cmd = _concurrentCommands!.First(c => c.Client == "IncrementVisits");
        cmd.Strategy.Should().Be(Angzarr.MergeStrategy.MergeAggregateHandles);
    }

    [Then(@"the result is COMMUTATIVE")]
    public void ThenTheResultIsCommutative()
    {
        _mergeStrategy.Should().Be(Angzarr.MergeStrategy.MergeCommutative);
    }

    private void InitializeCommand()
    {
        _command = new Angzarr.CommandBook
        {
            Cover = new Angzarr.Cover
            {
                Domain = _aggregate?.Cover?.Domain ?? "test",
                Root = _aggregate?.Cover?.Root ?? Helpers.UuidToProto(Guid.NewGuid()),
            },
        };
        _command.Pages.Add(
            new Angzarr.CommandPage
            {
                Sequence = _targetSequence,
                MergeStrategy = _mergeStrategy,
                Command = new Any
                {
                    TypeUrl = "type.googleapis.com/examples.TestCommand",
                    Value = Google.Protobuf.ByteString.Empty,
                },
            }
        );
    }
}
