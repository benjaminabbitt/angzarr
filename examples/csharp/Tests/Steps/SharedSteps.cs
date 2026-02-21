using FluentAssertions;
using TechTalk.SpecFlow;
using Angzarr.Client;
using Tests.Support;

namespace Tests.Steps;

[Binding]
public class SharedSteps
{
    private readonly ScenarioContext _scenarioContext;
    private readonly TestContext _testContext;

    public SharedSteps(ScenarioContext scenarioContext, TestContext testContext)
    {
        _scenarioContext = scenarioContext;
        _testContext = testContext;
    }

    private Exception? GetError()
    {
        // Check all domain-specific error context keys
        if (_scenarioContext.TryGetValue("error", out CommandRejectedError? playerError) && playerError != null)
            return playerError;
        if (_scenarioContext.TryGetValue("tableError", out CommandRejectedError? tableError) && tableError != null)
            return tableError;
        // Hand uses TestContext.LastException
        if (_testContext.LastException != null)
            return _testContext.LastException;
        return null;
    }

    [Then(@"the command fails with status ""(.*)""")]
    public void ThenTheCommandFailsWithStatus(string status)
    {
        var error = GetError();
        error.Should().NotBeNull("Expected command to fail but it succeeded");
    }

    [Then(@"the error message contains ""(.*)""")]
    public void ThenTheErrorMessageContains(string text)
    {
        var error = GetError();
        error.Should().NotBeNull("Expected an error but got success");
        error!.Message.ToLower().Should().Contain(text.ToLower());
    }
}
