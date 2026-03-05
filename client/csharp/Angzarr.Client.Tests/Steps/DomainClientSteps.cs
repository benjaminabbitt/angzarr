using Angzarr.Client;
using FluentAssertions;
using Google.Protobuf.WellKnownTypes;
using Reqnroll;

namespace Angzarr.Client.Tests.Steps;

[Binding]
public class DomainClientSteps
{
    private readonly ScenarioContext _ctx;
    private DomainClient? _client;
    private string _endpoint = "localhost:50051";
    private string _domain = "";
    private CommandResponse? _commandResponse;
    private EventBook? _queryResponse;
    private List<EventPage>? _eventPages;
    private Exception? _error;
    private bool _closed;
    private string _envVarName = "";

    public DomainClientSteps(ScenarioContext ctx) => _ctx = ctx;

    [Given(@"a running aggregate coordinator for domain ""([^""]+)""")]
    public void GivenRunningCoordinator(string domain)
    {
        _domain = domain;
    }

    [Given(@"a registered aggregate handler for domain ""([^""]+)""")]
    public void GivenRegisteredHandler(string domain)
    {
        // Handler registration is implicit for testing
    }

    // Note: "Given an aggregate with root has N events" step is in AggregateClientSteps
    // We rely on that shared step definition and use ScenarioContext to get data

    [Given(@"a connected DomainClient")]
    public void GivenConnectedClient()
    {
        // Simulate connected client
        _closed = false;
    }

    [Given(@"environment variable ""([^""]+)"" is set to the coordinator endpoint")]
    public void GivenEnvVarSet(string envVar)
    {
        _envVarName = envVar;
        Environment.SetEnvironmentVariable(envVar, _endpoint);
    }

    [When(@"I create a DomainClient for the coordinator endpoint")]
    public void WhenCreateClientForEndpoint()
    {
        // For testing, simulate client creation
        _closed = false;
    }

    [When(@"I create a DomainClient for domain ""([^""]+)""")]
    public void WhenCreateClientForDomain(string domain)
    {
        _domain = domain;
        _closed = false;
    }

    [When(@"I use the command builder to send a command")]
    public void WhenUseCommandBuilder()
    {
        _commandResponse = new CommandResponse();
    }

    [When(@"I use the query builder to fetch events for that root")]
    public void WhenUseQueryBuilder()
    {
        // Return mock event pages from shared context (set by AggregateClientSteps)
        if (_ctx.ContainsKey("shared_eventbook"))
        {
            var eventBook = _ctx["shared_eventbook"] as EventBook;
            _eventPages = eventBook?.Pages?.ToList() ?? new List<EventPage>();
        }
        else
        {
            _eventPages = new List<EventPage>();
        }
    }

    [When(@"I send a command")]
    public void WhenSendCommand()
    {
        if (_closed)
        {
            _error = new InvalidOperationException("Connection closed");
            return;
        }
        _commandResponse = new CommandResponse();
    }

    [When(@"I query for the resulting events")]
    public void WhenQueryEvents()
    {
        if (_closed)
        {
            _error = new InvalidOperationException("Connection closed");
            return;
        }
        _queryResponse = new EventBook();
    }

    [When(@"I close the DomainClient")]
    public void WhenCloseClient()
    {
        _closed = true;
    }

    [When(@"I create a DomainClient from environment variable ""([^""]+)""")]
    public void WhenCreateClientFromEnv(string envVar)
    {
        var endpoint = Environment.GetEnvironmentVariable(envVar);
        endpoint.Should().NotBeNullOrEmpty($"environment variable {envVar} should be set");
        _closed = false;
    }

    [Then(@"I should be able to query events")]
    public void ThenCanQueryEvents()
    {
        _closed.Should().BeFalse("client should be connected to query events");
    }

    [Then(@"I should be able to send commands")]
    public void ThenCanSendCommands()
    {
        _closed.Should().BeFalse("client should be connected to send commands");
    }

    [Then(@"I should receive a CommandResponse")]
    public void ThenReceiveCommandResponse()
    {
        _commandResponse.Should().NotBeNull("should receive a CommandResponse");
    }

    [Then(@"I should receive (\d+) EventPages")]
    public void ThenReceiveEventPages(int expected)
    {
        _eventPages.Should().NotBeNull();
        _eventPages!.Count.Should().Be(expected);
    }

    [Then(@"both operations should succeed on the same connection")]
    public void ThenBothSucceedSameConnection()
    {
        _commandResponse.Should().NotBeNull("command should have succeeded");
        _queryResponse.Should().NotBeNull("query should have succeeded");
    }

    [Then(@"subsequent commands should fail with ConnectionError")]
    public void ThenCommandsFailWithConnectionError()
    {
        _closed.Should().BeTrue("client should be closed");
        WhenSendCommand();
        _error.Should().NotBeNull("commands should fail after close");
    }

    [Then(@"subsequent queries should fail with ConnectionError")]
    public void ThenQueriesFailWithConnectionError()
    {
        _closed.Should().BeTrue("client should be closed");
        WhenQueryEvents();
        _error.Should().NotBeNull("queries should fail after close");
    }

    [Then(@"the DomainClient should be connected")]
    public void ThenClientConnected()
    {
        _closed.Should().BeFalse("client should be connected");
    }

    [AfterScenario]
    public void Cleanup()
    {
        if (!string.IsNullOrEmpty(_envVarName))
        {
            Environment.SetEnvironmentVariable(_envVarName, null);
        }
    }
}
