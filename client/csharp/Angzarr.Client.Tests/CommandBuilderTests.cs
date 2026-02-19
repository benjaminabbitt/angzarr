using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Xunit;

namespace Angzarr.Client.Tests;

/// <summary>
/// Tests for CommandBuilder covering the scenarios from command-builder.feature.
/// Uses Empty as a placeholder protobuf message since we need IMessage for WithCommand.
/// </summary>
public class CommandBuilderTests
{
    // Use Empty as a simple test message
    private static readonly Empty TestMessage = new Empty();

    [Fact]
    public void Build_WithExplicitFieldValues_ShouldSetAllFields()
    {
        // Given an AggregateClient connected to the coordinator (simulated with null)
        // When I build a command using CommandBuilder with explicit values
        var rootGuid = Guid.Parse("550e8400-e29b-41d4-a716-446655440000");
        var correlationId = "corr-123";
        var sequence = 5;

        var builder = new CommandBuilder(null!, "test", rootGuid)
            .WithCorrelationId(correlationId)
            .WithSequence(sequence)
            .WithCommand("type.googleapis.com/test.TestCommand", TestMessage);

        var command = builder.Build();

        // Then the resulting CommandBook should have the specified values
        command.Cover.Domain.Should().Be("test");
        Helpers.ProtoToUuid(command.Cover.Root).Should().Be(rootGuid);
        command.Cover.CorrelationId.Should().Be(correlationId);
        command.Pages[0].Sequence.Should().Be((uint)sequence);
        command.Pages[0].Command.TypeUrl.Should().Be("type.googleapis.com/test.TestCommand");
    }

    [Fact]
    public void Build_WithoutCorrelationId_ShouldAutoGenerateOne()
    {
        // When I build a command without specifying correlation_id
        var builder = new CommandBuilder(null!, "test")
            .WithCommand("type.googleapis.com/test.TestCommand", TestMessage);

        var command = builder.Build();

        // Then the resulting CommandBook should have a non-empty correlation_id
        command.Cover.CorrelationId.Should().NotBeNullOrEmpty();
        Guid.TryParse(command.Cover.CorrelationId, out _).Should().BeTrue();
    }

    [Fact]
    public void Build_ForNewAggregate_ShouldHaveNoRootUUID()
    {
        // When I build a command for domain "test" without specifying root
        var builder = new CommandBuilder(null!, "test")
            .WithCommand("type.googleapis.com/test.TestCommand", TestMessage);

        var command = builder.Build();

        // Then the resulting CommandBook should have no root UUID
        // In protobuf C#, check if Root is the default instance
        command.Cover.Root.Should().BeNull();
    }

    [Fact]
    public void Build_WithoutSequence_ShouldDefaultToZero()
    {
        // When I build a command without specifying sequence
        var builder = new CommandBuilder(null!, "test")
            .WithCommand("type.googleapis.com/test.TestCommand", TestMessage);

        var command = builder.Build();

        // Then the resulting CommandBook should have sequence 0
        command.Pages[0].Sequence.Should().Be(0u);
    }

    [Fact]
    public void MethodChaining_ShouldReturnBuilder()
    {
        // Verify method chaining returns builder for fluent composition
        var builder = new CommandBuilder(null!, "test");

        var result1 = builder.WithCorrelationId("chain-test");
        var result2 = result1.WithSequence(10);
        var result3 = result2.WithCommand("type.googleapis.com/test.TestCommand", TestMessage);

        result1.Should().BeSameAs(builder);
        result2.Should().BeSameAs(builder);
        result3.Should().BeSameAs(builder);

        var command = builder.Build();
        command.Cover.CorrelationId.Should().Be("chain-test");
        command.Pages[0].Sequence.Should().Be(10u);
    }

    [Fact]
    public void Build_WithProtobufMessage_ShouldSerializeCorrectly()
    {
        // When I build a command with a protobuf message
        var typeUrl = "type.googleapis.com/google.protobuf.Empty";

        var builder = new CommandBuilder(null!, "test")
            .WithCommand(typeUrl, TestMessage);

        var command = builder.Build();

        // Then the payload should be correctly serialized
        command.Pages[0].Command.TypeUrl.Should().Be(typeUrl);
        // Empty message serializes to empty bytes
        command.Pages[0].Command.Value.Should().BeEmpty();
    }

    [Fact]
    public void Build_WithoutCommand_ShouldThrow()
    {
        // When trying to build without setting a command
        var builder = new CommandBuilder(null!, "test");

        // Then it should throw InvalidArgumentError
        var act = () => builder.Build();
        act.Should().Throw<InvalidArgumentError>();
    }
}
