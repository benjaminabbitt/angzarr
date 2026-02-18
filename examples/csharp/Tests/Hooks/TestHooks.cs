using TechTalk.SpecFlow;

namespace Tests.Hooks;

[Binding]
public class TestHooks
{
    private readonly ScenarioContext _context;

    public TestHooks(ScenarioContext context)
    {
        _context = context;
    }

    [BeforeScenario]
    public void BeforeScenario()
    {
        // ScenarioContext is automatically cleared between scenarios
        // No explicit reset needed as each scenario gets a fresh context
    }

    [AfterScenario]
    public void AfterScenario()
    {
        // Clean up any resources if needed
    }
}
