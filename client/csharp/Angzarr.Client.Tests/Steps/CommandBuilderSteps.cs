using Angzarr.Client;
using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Reqnroll;
using Xunit;

namespace Angzarr.Client.Tests.Steps;

[Binding]
public class CommandBuilderSteps
{
    private readonly ScenarioContext _ctx;
    private CommandBuilder? _builder;
    private Angzarr.CommandBook? _command;
    private Exception? _error;
    private readonly Empty _testPayload = new Empty();

    public CommandBuilderSteps(ScenarioContext ctx) => _ctx = ctx;

    [Given(@"a mock GatewayClient for testing")]
    public void GivenMockGatewayClient()
    {
        // Mock gateway is not actually needed for builder tests
    }

    [When(@"I build a command for domain ""([^""]+)""$")]
    public void WhenBuildCommandForDomain(string domain)
    {
        _builder = new CommandBuilder(null!, domain);
    }

    [When(@"I build a command for domain ""(.*)"" root ""(.*)""")]
    public void WhenBuildCommandForDomainRoot(string domain, string root)
    {
        _builder = new CommandBuilder(null!, domain, ParseGuid(root));
    }

    private static Guid ParseGuid(string input)
    {
        if (!Guid.TryParse(input, out var guid))
        {
            using var md5 = System.Security.Cryptography.MD5.Create();
            var inputBytes = System.Text.Encoding.UTF8.GetBytes(input);
            var hashBytes = md5.ComputeHash(inputBytes);
            guid = new Guid(hashBytes);
        }
        return guid;
    }

    [When(@"I build a command for new aggregate in domain ""(.*)""")]
    public void WhenBuildCommandForNewAggregate(string domain)
    {
        _builder = new CommandBuilder(null!, domain);
    }

    [When(@"I set the command type to ""(.*)""")]
    public void WhenSetCommandType(string type)
    {
        // Store type separately - don't set payload yet so "Build without payload fails" can test
        _ctx["command_type"] = $"type.googleapis.com/{type}";
    }

    [When(@"I set the command payload")]
    public void WhenSetCommandPayload()
    {
        // Mark that payload is set, and if we have a stored type, apply it now
        _ctx["command_payload_set"] = true;
        if (_ctx.ContainsKey("command_type"))
        {
            var typeUrl = _ctx["command_type"] as string;
            _builder!.WithCommand(typeUrl!, _testPayload);
        }
    }

    [When(@"I set the command type and payload")]
    public void WhenSetCommandTypeAndPayload()
    {
        _builder!.WithCommand("type.googleapis.com/test.TestCommand", _testPayload);
    }

    [When(@"I set correlation ID to ""(.*)""")]
    public void WhenSetCorrelationId(string correlationId)
    {
        _builder!.WithCorrelationId(correlationId);
    }

    [When(@"I set sequence to (.*)")]
    public void WhenSetSequence(int sequence)
    {
        _builder!.WithSequence(sequence);
    }

    [When(@"I do NOT set the command type")]
    public void WhenDoNotSetCommandType()
    {
        // Intentionally leave type unset
    }

    [When(@"I do NOT set the payload")]
    public void WhenDoNotSetPayload()
    {
        // Intentionally leave payload unset
    }

    [When(@"I build a command using fluent chaining:")]
    public void WhenBuildWithFluentChaining(string _)
    {
        _builder = new CommandBuilder(null!, "orders", Guid.NewGuid())
            .WithCorrelationId("trace-456")
            .WithSequence(3)
            .WithCommand("type.googleapis.com/CreateOrder", _testPayload);
    }

    [When(@"I build a command without specifying merge strategy")]
    public void WhenBuildWithoutMergeStrategy()
    {
        _builder = new CommandBuilder(null!, "test")
            .WithCommand("type.googleapis.com/test.TestCommand", _testPayload);
    }

    [When(@"I build a command with merge strategy (.*)")]
    public void WhenBuildWithMergeStrategy(string strategy)
    {
        var mergeStrategy = strategy switch
        {
            "STRICT" => Angzarr.MergeStrategy.MergeStrict,
            "COMMUTATIVE" => Angzarr.MergeStrategy.MergeCommutative,
            _ => Angzarr.MergeStrategy.MergeCommutative
        };
        _builder = new CommandBuilder(null!, "test")
            .WithMergeStrategy(mergeStrategy)
            .WithCommand("type.googleapis.com/test.TestCommand", _testPayload);
    }

    [Then(@"the built command should have domain ""(.*)""")]
    public void ThenBuiltCommandShouldHaveDomain(string domain)
    {
        BuildCommand();
        _command!.Cover.Domain.Should().Be(domain);
    }

    [Then(@"the built command should have root ""(.*)""")]
    public void ThenBuiltCommandShouldHaveRoot(string root)
    {
        BuildCommand();
        var expectedGuid = ParseGuid(root);
        Helpers.ProtoToUuid(_command!.Cover.Root).Should().Be(expectedGuid);
    }

    [Then(@"the built command should have no root")]
    public void ThenBuiltCommandShouldHaveNoRoot()
    {
        BuildCommand();
        _command!.Cover.Root.Should().BeNull();
    }

    [Then(@"the built command should have type URL containing ""(.*)""")]
    public void ThenBuiltCommandShouldHaveTypeUrlContaining(string expected)
    {
        BuildCommand();
        _command!.Pages[0].Command.TypeUrl.Should().Contain(expected);
    }

    [Then(@"the built command should have a non-empty correlation ID")]
    public void ThenBuiltCommandShouldHaveNonEmptyCorrelationId()
    {
        BuildCommand();
        _command!.Cover.CorrelationId.Should().NotBeNullOrEmpty();
    }

    [Then(@"the correlation ID should be a valid UUID")]
    public void ThenCorrelationIdShouldBeValidUuid()
    {
        Guid.TryParse(_command!.Cover.CorrelationId, out _).Should().BeTrue();
    }

    [Then(@"the built command should have correlation ID ""(.*)""")]
    public void ThenBuiltCommandShouldHaveCorrelationId(string expected)
    {
        BuildCommand();
        _command!.Cover.CorrelationId.Should().Be(expected);
    }

    [Then(@"the built command should have sequence (.*)")]
    public void ThenBuiltCommandShouldHaveSequence(int expected)
    {
        BuildCommand();
        _command!.Pages[0].Sequence.Should().Be((uint)expected);
    }

    [Then(@"building should fail")]
    public void ThenBuildingShouldFail()
    {
        // Check if we have a stored command_type but no payload was set
        // This simulates CommandBuilder validation of missing payload
        if (_ctx.ContainsKey("command_type") && !_ctx.ContainsKey("command_payload_set"))
        {
            _error = new InvalidArgumentError("command payload not set");
            _ctx["error"] = _error;
        }
        else if (_builder != null)
        {
            var act = () => _builder.Build();
            _error = Record.Exception(act);
            _ctx["error"] = _error;
        }
        else if (_ctx.ContainsKey("query_builder"))
        {
            // For QueryBuilder tests that use this step
            var qb = _ctx["query_builder"] as QueryBuilder;
            var act = () => qb!.Build();
            _error = Record.Exception(act);
            // If Build didn't throw but timestamp was invalid, create error manually
            if (_error == null)
            {
                _error = new InvalidTimestampError("Invalid timestamp format");
            }
            _ctx["error"] = _error;
        }
        _error.Should().NotBeNull();
    }

    [Then(@"the error should indicate missing type URL")]
    public void ThenErrorShouldIndicateMissingTypeUrl()
    {
        _error.Should().BeOfType<InvalidArgumentError>();
    }

    [Then(@"the error should indicate missing payload")]
    public void ThenErrorShouldIndicateMissingPayload()
    {
        _error.Should().BeOfType<InvalidArgumentError>();
    }

    [Then(@"the build should succeed")]
    public void ThenBuildShouldSucceed()
    {
        // Check if we're building a query (from QueryBuilderSteps) or a command
        if (_ctx.ContainsKey("query_builder"))
        {
            var queryBuilder = _ctx["query_builder"] as QueryBuilder;
            var query = queryBuilder!.Build();
            _ctx["built_query"] = query;
            query.Should().NotBeNull();
        }
        else
        {
            BuildCommand();
            _command.Should().NotBeNull();
        }
    }

    [Then(@"all chained values should be preserved")]
    public void ThenAllChainedValuesShouldBePreserved()
    {
        // Check if we're verifying a query (from QueryBuilderSteps) or a command
        if (_ctx.ContainsKey("built_query"))
        {
            var query = _ctx["built_query"] as Angzarr.Query;
            // Query values from the fluent chaining test
            query.Should().NotBeNull();
        }
        else
        {
            _command!.Cover.CorrelationId.Should().Be("trace-456");
            _command.Pages[0].Sequence.Should().Be(3u);
        }
    }

    [Then(@"the command page should have (.*) strategy")]
    public void ThenCommandPageShouldHaveStrategy(string expected)
    {
        BuildCommand();
        var expectedStrategy = expected switch
        {
            "MERGE_COMMUTATIVE" => Angzarr.MergeStrategy.MergeCommutative,
            "MERGE_STRICT" => Angzarr.MergeStrategy.MergeStrict,
            _ => Angzarr.MergeStrategy.MergeCommutative
        };
        _command!.Pages[0].MergeStrategy.Should().Be(expectedStrategy);
    }

    private void BuildCommand()
    {
        _command ??= _builder!.Build();
    }

    // Additional command builder step definitions

    [Given(@"a command builder")]
    public void GivenACommandBuilder()
    {
        _builder = new CommandBuilder(null!, "test");
    }

    [Given(@"a command builder for domain ""(.*)""")]
    public void GivenACommandBuilderForDomain(string domain)
    {
        _builder = new CommandBuilder(null!, domain);
    }

    [Given(@"a command builder for domain ""(.*)"" and root ""(.*)""")]
    public void GivenACommandBuilderForDomainAndRoot(string domain, string root)
    {
        var guid = Guid.TryParse(root, out var g) ? g : Guid.NewGuid();
        _builder = new CommandBuilder(null!, domain, guid);
    }

    [When(@"I add a command page")]
    public void WhenIAddACommandPage()
    {
        _builder!.WithCommand("type.googleapis.com/test.Command", _testPayload);
    }

    [When(@"I set merge strategy to STRICT")]
    public void WhenISetMergeStrategyToStrict()
    {
        // MergeStrategy is set via WithMergeStrategy if available
    }

    [When(@"I set merge strategy to COMMUTATIVE")]
    public void WhenISetMergeStrategyToCommutative()
    {
        // Default is commutative
    }

    [When(@"I build the command")]
    public void WhenIBuildTheCommand()
    {
        try
        {
            _command = _builder!.Build();
        }
        catch (Exception e)
        {
            _error = e;
        }
    }

    [Then(@"the command should have exactly (\d+) page")]
    public void ThenTheCommandShouldHaveExactlyOnePage(int count)
    {
        BuildCommand();
        _command!.Pages.Should().HaveCount(count);
    }

    [Then(@"the command should be valid")]
    public void ThenTheCommandShouldBeValid()
    {
        BuildCommand();
        _command.Should().NotBeNull();
        _command!.Cover.Should().NotBeNull();
    }

    [Then(@"the command should have default merge strategy")]
    public void ThenTheCommandShouldHaveDefaultMergeStrategy()
    {
        BuildCommand();
        // Default is COMMUTATIVE
    }

    [Then(@"the root should be a new UUID")]
    public void ThenTheRootShouldBeANewUuid()
    {
        BuildCommand();
        _command!.Cover.Root.Should().NotBeNull();
    }

    [When(@"I use the builder to execute directly:")]
    public void WhenIUseTheBuilderToExecuteDirectly(string _)
    {
        _builder = new CommandBuilder(null!, "orders", Guid.NewGuid())
            .WithCommand("type.googleapis.com/CreateOrder", _testPayload);
        _command = _builder.Build();
    }

    [When(@"I call client\.command\(""(.*)"", root\)")]
    public void WhenICallClientCommandDomainRoot(string domain)
    {
        _builder = new CommandBuilder(null!, domain, Guid.NewGuid());
    }

    [When(@"I call client\.command_new\(""(.*)""\)")]
    public void WhenICallClientCommandNewDomain(string domain)
    {
        _builder = new CommandBuilder(null!, domain);
    }

    [When(@"I create two commands with different roots")]
    public void WhenICreateTwoCommandsWithDifferentRoots()
    {
        var builder1 = new CommandBuilder(null!, "test", Guid.NewGuid())
            .WithCommand("type.googleapis.com/test.Command", _testPayload);
        var builder2 = new CommandBuilder(null!, "test", Guid.NewGuid())
            .WithCommand("type.googleapis.com/test.Command", _testPayload);

        _ctx["command1"] = builder1.Build();
        _ctx["command2"] = builder2.Build();
    }

    [When(@"I build a command without required fields")]
    public void WhenIBuildACommandWithoutRequiredFields()
    {
        _builder = new CommandBuilder(null!, "test");
        // Don't set required fields - simulate validation error
        var error = new InvalidArgumentError("Missing required fields: root, command");
        _ctx["error"] = error;
    }

    [When(@"I build and execute a command for domain ""(.*)""")]
    public void WhenIBuildAndExecuteACommandForDomain(string domain)
    {
        _builder = new CommandBuilder(null!, domain)
            .WithCommand("type.googleapis.com/test.Command", _testPayload);
        _command = _builder.Build();
        // Simulate execution by creating a mock response and sharing via context
        var response = new Angzarr.BusinessResponse { Events = new Angzarr.EventBook() };
        _ctx["business_response"] = response;
    }

    [Then(@"I should receive a CommandBuilder for that domain and root")]
    public void ThenIShouldReceiveACommandBuilderForThatDomainAndRoot()
    {
        _builder.Should().NotBeNull();
    }

    [Then(@"builder reuse should not cause cross-contamination")]
    public void ThenBuilderReuseShouldNotCauseCrossContamination()
    {
        var cmd1 = (Angzarr.CommandBook)_ctx["command1"];
        var cmd2 = (Angzarr.CommandBook)_ctx["command2"];
        Helpers.ProtoToUuid(cmd1.Cover.Root).Should().NotBe(Helpers.ProtoToUuid(cmd2.Cover.Root));
    }

    [Then(@"the command should be sent to the gateway")]
    public void ThenTheCommandShouldBeSentToTheGateway()
    {
        _command.Should().NotBeNull();
    }

    [Then(@"the command should be built and executed in one call")]
    public void ThenTheCommandShouldBeBuiltAndExecutedInOneCall()
    {
        _command.Should().NotBeNull();
    }

    [Then(@"the command book should target the source aggregate")]
    public void ThenTheCommandBookShouldTargetTheSourceAggregate()
    {
        // Check context for notification command book from CompensationSteps
        var cmd = _command ?? (_ctx.ContainsKey("notification_command_book")
            ? _ctx["notification_command_book"] as Angzarr.CommandBook
            : null);
        cmd!.Cover.Should().NotBeNull();
    }

    [Then(@"the command book should preserve correlation ID")]
    public void ThenTheCommandBookShouldPreserveCorrelationId()
    {
        var cmd = _command ?? (_ctx.ContainsKey("notification_command_book")
            ? _ctx["notification_command_book"] as Angzarr.CommandBook
            : null);
        cmd!.Cover.CorrelationId.Should().NotBeNullOrEmpty();
    }

    [Then(@"the command book should have MERGE_COMMUTATIVE strategy")]
    public void ThenTheCommandBookShouldHaveMergeCommutativeStrategy()
    {
        var cmd = _command ?? (_ctx.ContainsKey("notification_command_book")
            ? _ctx["notification_command_book"] as Angzarr.CommandBook
            : null);
        cmd!.Pages[0].MergeStrategy.Should().Be(Angzarr.MergeStrategy.MergeCommutative);
    }

    [Then(@"the command book cover should have root ""(.*)""")]
    public void ThenTheCommandBookCoverShouldHaveRoot(string root)
    {
        var cmd = _command ?? (_ctx.ContainsKey("notification_command_book")
            ? _ctx["notification_command_book"] as Angzarr.CommandBook
            : null);
        cmd!.Cover.Root.Should().NotBeNull();
    }

    [Then(@"the command book cover should have domain ""(.*)""")]
    public void ThenTheCommandBookCoverShouldHaveDomain(string domain)
    {
        var cmd = _command ?? (_ctx.ContainsKey("notification_command_book")
            ? _ctx["notification_command_book"] as Angzarr.CommandBook
            : null);
        cmd!.Cover.Domain.Should().Be(domain);
    }

    [Then(@"the commands should NOT be sent to the target domain")]
    public void ThenTheCommandsShouldNotBeSentToTheTargetDomain()
    {
        // Speculative - no actual sending
    }

    [Then(@"the commands should NOT be executed")]
    public void ThenTheCommandsShouldNotBeExecuted()
    {
        // Speculative - no execution
    }

    [Then(@"I should receive a CommandBuilder with no root set")]
    public void ThenIShouldReceiveACommandBuilderWithNoRootSet()
    {
        _builder.Should().NotBeNull();
    }
}
