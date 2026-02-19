using FluentAssertions;
using Xunit;

namespace Angzarr.Client.Tests;

/// <summary>
/// Tests for SpeculativeClient - what-if scenario execution.
/// </summary>
public class SpeculativeClientTests
{
    [Fact]
    public void Connect_WithInvalidEndpoint_ShouldThrowConnectionError()
    {
        // Attempting to connect to an invalid endpoint should fail
        var act = () => SpeculativeClient.Connect("invalid:endpoint:format:extra");
        act.Should().Throw<ConnectionError>();
    }

    [Fact]
    public void Class_ShouldHaveConnectMethod()
    {
        // Verify Connect factory method exists
        var method = typeof(SpeculativeClient).GetMethod("Connect", new[] { typeof(string) });
        method.Should().NotBeNull();
        method!.ReturnType.Should().Be(typeof(SpeculativeClient));
    }

    [Fact]
    public void Class_ShouldHaveFromEnvMethod()
    {
        // Verify FromEnv factory method exists
        var method = typeof(SpeculativeClient).GetMethod("FromEnv", new[] { typeof(string), typeof(string) });
        method.Should().NotBeNull();
        method!.ReturnType.Should().Be(typeof(SpeculativeClient));
    }

    [Fact]
    public void Class_ShouldHaveAggregateMethod()
    {
        // Verify Aggregate method exists
        var method = typeof(SpeculativeClient).GetMethod("Aggregate");
        method.Should().NotBeNull();
        method!.ReturnType.Should().Be(typeof(CommandResponse));
    }

    [Fact]
    public void Class_ShouldHaveProjectorMethod()
    {
        // Verify Projector method exists
        var method = typeof(SpeculativeClient).GetMethod("Projector");
        method.Should().NotBeNull();
        method!.ReturnType.Should().Be(typeof(Projection));
    }

    [Fact]
    public void Class_ShouldHaveSagaMethod()
    {
        // Verify Saga method exists
        var method = typeof(SpeculativeClient).GetMethod("Saga");
        method.Should().NotBeNull();
        method!.ReturnType.Should().Be(typeof(SagaResponse));
    }

    [Fact]
    public void Class_ShouldHaveProcessManagerMethod()
    {
        // Verify ProcessManager method exists
        var method = typeof(SpeculativeClient).GetMethod("ProcessManager");
        method.Should().NotBeNull();
        method!.ReturnType.Should().Be(typeof(ProcessManagerHandleResponse));
    }

    [Fact]
    public void Class_ShouldImplementIDisposable()
    {
        // SpeculativeClient should be disposable for resource cleanup
        typeof(SpeculativeClient).Should().Implement<IDisposable>();
    }
}
