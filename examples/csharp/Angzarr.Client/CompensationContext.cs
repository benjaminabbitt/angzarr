using Angzarr;
using Google.Protobuf;

namespace Angzarr.Client;

/// <summary>
/// Context for compensation handling, extracted from a Notification.
/// </summary>
public class CompensationContext
{
    public string IssuerName { get; init; } = "";
    public string IssuerType { get; init; } = "";
    public uint SourceEventSequence { get; init; }
    public string RejectionReason { get; init; } = "";
    public string Domain { get; init; } = "";
    public string CommandType { get; init; } = "";
    public CommandBook? RejectedCommand { get; init; }

    /// <summary>
    /// Extract compensation context from a Notification.
    /// </summary>
    public static CompensationContext From(Notification notification)
    {
        // Extract RejectionNotification from the payload
        RejectionNotification? rejection = null;
        if (notification.Payload?.TypeUrl?.Contains("RejectionNotification") == true)
        {
            rejection = notification.Payload.Unpack<RejectionNotification>();
        }

        var typeUrl = rejection?.RejectedCommand?.Pages.FirstOrDefault()?.Command?.TypeUrl ?? "";
        var commandType = typeUrl.Contains('.') ? typeUrl.Split('.').Last() : typeUrl;

        return new CompensationContext
        {
            IssuerName = rejection?.IssuerName ?? "",
            IssuerType = rejection?.IssuerType ?? "",
            SourceEventSequence = rejection?.SourceEventSequence ?? 0,
            RejectionReason = rejection?.RejectionReason ?? "",
            Domain = rejection?.RejectedCommand?.Cover?.Domain ?? "",
            CommandType = commandType,
            RejectedCommand = rejection?.RejectedCommand,
        };
    }
}
