namespace Angzarr.Client;

/// <summary>
/// Extracted context from a rejection Notification.
///
/// <para>When a saga or process manager issues a command that gets rejected, the framework
/// sends a Notification containing rejection details. CompensationContext extracts
/// this information into a developer-friendly structure.</para>
///
/// <para><b>Why this matters:</b></para>
/// <list type="bullet">
///   <item>Debugging - understand which component issued the failing command</item>
///   <item>Compensation logic - decide whether to retry, rollback, or escalate</item>
///   <item>Observability - log structured rejection data for monitoring</item>
///   <item>Business rules - different compensation for different rejection reasons</item>
/// </list>
///
/// <example>
/// <code>
/// public Angzarr.BusinessResponse HandleRejection(Angzarr.Notification notification)
/// {
///     var ctx = CompensationContext.FromNotification(notification);
///
///     if (ctx.IssuerName == "saga-order-fulfill")
///     {
///         // Emit compensation events
///         var cancelEvent = new OrderCancelled { Reason = ctx.RejectionReason };
///         return EmitCompensationEvents(NewEventBook(cancelEvent));
///     }
///
///     // Delegate to framework
///     return DelegateToFramework($"No handler for {ctx.IssuerName}");
/// }
/// </code>
/// </example>
/// </summary>
public class CompensationContext
{
    /// <summary>
    /// Name of the saga/PM that issued the rejected command.
    /// </summary>
    public string IssuerName { get; init; } = "";

    /// <summary>
    /// Type of issuer: "saga" or "process_manager".
    /// </summary>
    public string IssuerType { get; init; } = "";

    /// <summary>
    /// Sequence of the event that triggered the saga/PM flow.
    /// </summary>
    public uint SourceEventSequence { get; init; }

    /// <summary>
    /// Why the command was rejected.
    /// </summary>
    public string RejectionReason { get; init; } = "";

    /// <summary>
    /// The command that was rejected (if available).
    /// </summary>
    public Angzarr.CommandBook? RejectedCommand { get; init; }

    /// <summary>
    /// Cover of the aggregate that triggered the flow.
    /// </summary>
    public Angzarr.Cover? SourceAggregate { get; init; }

    /// <summary>
    /// Extract compensation context from a Notification.
    /// </summary>
    /// <param name="notification">The notification containing rejection details</param>
    /// <returns>A new CompensationContext</returns>
    public static CompensationContext FromNotification(Angzarr.Notification notification)
    {
        var rejection = new Angzarr.RejectionNotification();

        if (notification.Payload != null)
        {
            try
            {
                rejection = notification.Payload.Unpack<Angzarr.RejectionNotification>();
            }
            catch
            {
                // Payload doesn't contain a RejectionNotification, use defaults
            }
        }

        return new CompensationContext
        {
            IssuerName = rejection.IssuerName ?? "",
            IssuerType = rejection.IssuerType ?? "",
            SourceEventSequence = rejection.SourceEventSequence,
            RejectionReason = rejection.RejectionReason ?? "",
            RejectedCommand = rejection.RejectedCommand,
            SourceAggregate = rejection.SourceAggregate
        };
    }

    /// <summary>
    /// Get the type URL of the rejected command, if available.
    ///
    /// <para>Compensation handlers are often keyed by command type:
    /// "If ReserveStock was rejected, release the hold."</para>
    /// </summary>
    /// <returns>The type URL suffix (e.g., "ReserveStock") or null</returns>
    public string? RejectedCommandType
    {
        get
        {
            if (RejectedCommand == null || RejectedCommand.Pages.Count == 0)
                return null;

            var page = RejectedCommand.Pages[0];
            var cmd = page.Command;
            if (cmd == null)
                return null;

            return Helpers.TypeNameFromUrl(cmd.TypeUrl);
        }
    }

    /// <summary>
    /// Build a dispatch key for routing rejection handlers.
    /// </summary>
    /// <returns>A key in format "domain/command" or empty string</returns>
    public string DispatchKey
    {
        get
        {
            if (RejectedCommand == null)
                return "";

            var domain = RejectedCommand.Cover?.Domain ?? "";
            var cmdType = RejectedCommandType ?? "";

            return string.IsNullOrEmpty(domain) || string.IsNullOrEmpty(cmdType)
                ? ""
                : $"{domain}/{cmdType}";
        }
    }
}
