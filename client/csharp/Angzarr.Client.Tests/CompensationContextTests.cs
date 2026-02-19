using FluentAssertions;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Xunit;

namespace Angzarr.Client.Tests;

/// <summary>
/// Tests for CompensationContext covering the scenarios from compensation-context.feature.
/// </summary>
public class CompensationContextTests
{
    [Fact]
    public void FromNotification_WithAllFields_ShouldExtractAllDetails()
    {
        // Given a Notification containing a RejectionNotification with all fields
        var rejectionNotification = new RejectionNotification
        {
            IssuerName = "saga-order-fulfill",
            IssuerType = "saga",
            SourceEventSequence = 7,
            RejectionReason = "out of stock"
        };

        var notification = CreateNotificationWith(rejectionNotification);

        // When I create a CompensationContext from the Notification
        var context = CompensationContext.FromNotification(notification);

        // Then the CompensationContext should have all fields set correctly
        context.IssuerName.Should().Be("saga-order-fulfill");
        context.IssuerType.Should().Be("saga");
        context.SourceEventSequence.Should().Be(7);
        context.RejectionReason.Should().Be("out of stock");
    }

    [Fact]
    public void FromNotification_WithRejectedCommand_ShouldExtractCommandType()
    {
        // Given a Notification with a rejected command of type "ReserveStock"
        var rejectionNotification = new RejectionNotification
        {
            IssuerName = "saga-test",
            IssuerType = "saga",
            RejectionReason = "invalid"
        };

        var command = new CommandBook();
        command.Pages.Add(new CommandPage
        {
            Command = new Any
            {
                TypeUrl = "type.googleapis.com/inventory.ReserveStock",
                Value = ByteString.Empty
            }
        });
        rejectionNotification.RejectedCommand = command;

        var notification = CreateNotificationWith(rejectionNotification);

        // When I create a CompensationContext from the Notification
        var context = CompensationContext.FromNotification(notification);

        // Then the rejected_command_type should end with "ReserveStock"
        context.RejectedCommandType.Should().NotBeNull();
        context.RejectedCommandType.Should().EndWith("ReserveStock");
    }

    [Fact]
    public void FromNotification_WithSourceAggregate_ShouldExtractDomain()
    {
        // Given a Notification with source_aggregate cover for domain "inventory"
        var rejectionNotification = new RejectionNotification
        {
            IssuerName = "saga-test",
            IssuerType = "saga",
            RejectionReason = "test",
            SourceAggregate = new Cover
            {
                Domain = "inventory"
            }
        };

        var notification = CreateNotificationWith(rejectionNotification);

        // When I create a CompensationContext from the Notification
        var context = CompensationContext.FromNotification(notification);

        // Then the source_aggregate should have domain "inventory"
        context.SourceAggregate.Should().NotBeNull();
        context.SourceAggregate!.Domain.Should().Be("inventory");
    }

    [Fact]
    public void FromNotification_WithoutRejectedCommand_ShouldReturnNullForCommand()
    {
        // Given a Notification without a rejected command
        var rejectionNotification = new RejectionNotification
        {
            IssuerName = "saga-test",
            IssuerType = "saga",
            RejectionReason = "timeout"
            // No RejectedCommand set
        };

        var notification = CreateNotificationWith(rejectionNotification);

        // When I create a CompensationContext from the Notification
        var context = CompensationContext.FromNotification(notification);

        // Then rejected_command should be null
        context.RejectedCommand.Should().BeNull();
        // And rejected_command_type should return null
        context.RejectedCommandType.Should().BeNull();
    }

    [Fact]
    public void FromNotification_WithEmptyPayload_ShouldReturnDefaultValues()
    {
        // Given a Notification with empty payload
        var notification = new Notification
        {
            Payload = new Any()
        };

        // When I create a CompensationContext from the Notification
        var context = CompensationContext.FromNotification(notification);

        // Then all fields should have default/empty values
        context.IssuerName.Should().BeEmpty();
        context.IssuerType.Should().BeEmpty();
        context.SourceEventSequence.Should().Be(0);
        context.RejectionReason.Should().BeEmpty();
        context.RejectedCommand.Should().BeNull();
        context.SourceAggregate.Should().BeNull();
    }

    [Fact]
    public void FromNotification_WithNoPayload_ShouldReturnDefaultValues()
    {
        // Given a Notification without any payload
        var notification = new Notification();

        // When I create a CompensationContext from the Notification
        var context = CompensationContext.FromNotification(notification);

        // Then all fields should have default/empty values
        context.IssuerName.Should().BeEmpty();
        context.IssuerType.Should().BeEmpty();
        context.SourceEventSequence.Should().Be(0);
        context.RejectionReason.Should().BeEmpty();
    }

    private static Notification CreateNotificationWith(RejectionNotification rejection)
    {
        var notification = new Notification();
        notification.Payload = Any.Pack(rejection);
        return notification;
    }
}
