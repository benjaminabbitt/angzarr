using FluentAssertions;
using Grpc.Core;
using Xunit;

namespace Angzarr.Client.Tests;

/// <summary>
/// Tests for error introspection methods.
/// </summary>
public class ErrorIntrospectionTests
{
    // =========================================================================
    // IsNotFound Tests
    // =========================================================================

    [Fact]
    public void GrpcError_WithNotFound_ShouldReturnTrueForIsNotFound()
    {
        var error = new GrpcError("not found", StatusCode.NotFound);
        error.IsNotFound().Should().BeTrue();
    }

    [Fact]
    public void GrpcError_WithOtherCode_ShouldReturnFalseForIsNotFound()
    {
        var error = new GrpcError("internal error", StatusCode.Internal);
        error.IsNotFound().Should().BeFalse();
    }

    // =========================================================================
    // IsPreconditionFailed Tests
    // =========================================================================

    [Fact]
    public void GrpcError_WithFailedPrecondition_ShouldReturnTrueForIsPreconditionFailed()
    {
        var error = new GrpcError("precondition failed", StatusCode.FailedPrecondition);
        error.IsPreconditionFailed().Should().BeTrue();
    }

    [Fact]
    public void GrpcError_WithOtherCode_ShouldReturnFalseForIsPreconditionFailed()
    {
        var error = new GrpcError("internal error", StatusCode.Internal);
        error.IsPreconditionFailed().Should().BeFalse();
    }

    [Fact]
    public void CommandRejectedError_ShouldReturnTrueForIsPreconditionFailed()
    {
        var error = new CommandRejectedError("rejected");
        error.IsPreconditionFailed().Should().BeTrue();
    }

    // =========================================================================
    // IsInvalidArgument Tests
    // =========================================================================

    [Fact]
    public void GrpcError_WithInvalidArgument_ShouldReturnTrueForIsInvalidArgument()
    {
        var error = new GrpcError("invalid argument", StatusCode.InvalidArgument);
        error.IsInvalidArgument().Should().BeTrue();
    }

    [Fact]
    public void InvalidArgumentError_ShouldReturnTrueForIsInvalidArgument()
    {
        var error = new InvalidArgumentError("bad input");
        error.IsInvalidArgument().Should().BeTrue();
    }

    [Fact]
    public void GrpcError_WithOtherCode_ShouldReturnFalseForIsInvalidArgument()
    {
        var error = new GrpcError("internal error", StatusCode.Internal);
        error.IsInvalidArgument().Should().BeFalse();
    }

    // =========================================================================
    // IsConnectionError Tests
    // =========================================================================

    [Fact]
    public void ConnectionError_ShouldReturnTrueForIsConnectionError()
    {
        var error = new ConnectionError("connection refused");
        error.IsConnectionError().Should().BeTrue();
    }

    [Fact]
    public void TransportError_ShouldReturnTrueForIsConnectionError()
    {
        var error = new TransportError("transport failed");
        error.IsConnectionError().Should().BeTrue();
    }

    [Fact]
    public void GrpcError_WithUnavailable_ShouldReturnTrueForIsConnectionError()
    {
        var error = new GrpcError("unavailable", StatusCode.Unavailable);
        error.IsConnectionError().Should().BeTrue();
    }

    [Fact]
    public void GrpcError_WithOtherCode_ShouldReturnFalseForIsConnectionError()
    {
        var error = new GrpcError("internal error", StatusCode.Internal);
        error.IsConnectionError().Should().BeFalse();
    }

    // =========================================================================
    // Base Class Default Behavior Tests
    // =========================================================================

    [Fact]
    public void ClientError_ShouldHaveDefaultFalseForAllIntrospectionMethods()
    {
        var error = new ClientError("generic error");
        error.IsNotFound().Should().BeFalse();
        error.IsPreconditionFailed().Should().BeFalse();
        error.IsInvalidArgument().Should().BeFalse();
        error.IsConnectionError().Should().BeFalse();
    }
}
