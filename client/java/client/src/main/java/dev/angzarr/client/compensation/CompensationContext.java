package dev.angzarr.client.compensation;

import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.CommandBook;
import dev.angzarr.Cover;
import dev.angzarr.Notification;
import dev.angzarr.RejectionNotification;
import dev.angzarr.client.Helpers;

/**
 * Extracted context from a rejection Notification.
 *
 * <p>Provides easy access to compensation-relevant fields when handling
 * saga/PM command rejections.
 *
 * <p>Usage:
 * <pre>{@code
 * @Rejected(domain = "payment", command = "ProcessPayment")
 * public FundsReleased handlePaymentRejected(Notification notification) {
 *     var ctx = CompensationContext.from(notification);
 *     logger.warn("Compensation: issuer={} reason={}",
 *         ctx.getIssuerName(), ctx.getRejectionReason());
 *
 *     return FundsReleased.newBuilder()
 *         .setAmount(getState().getReservedFunds())
 *         .build();
 * }
 * }</pre>
 */
public class CompensationContext {

    private final String issuerName;
    private final String issuerType;
    private final int sourceEventSequence;
    private final String rejectionReason;
    private final CommandBook rejectedCommand;
    private final Cover sourceAggregate;

    private CompensationContext(RejectionNotification rejection) {
        this.issuerName = rejection.getIssuerName();
        this.issuerType = rejection.getIssuerType();
        this.sourceEventSequence = rejection.getSourceEventSequence();
        this.rejectionReason = rejection.getRejectionReason();
        this.rejectedCommand = rejection.hasRejectedCommand() ? rejection.getRejectedCommand() : null;
        this.sourceAggregate = rejection.hasSourceAggregate() ? rejection.getSourceAggregate() : null;
    }

    /**
     * Extract compensation context from a Notification.
     */
    public static CompensationContext from(Notification notification) {
        if (!notification.hasPayload()) {
            return new CompensationContext(RejectionNotification.getDefaultInstance());
        }

        try {
            RejectionNotification rejection = notification.getPayload()
                .unpack(RejectionNotification.class);
            return new CompensationContext(rejection);
        } catch (InvalidProtocolBufferException e) {
            throw new RuntimeException("Failed to unpack RejectionNotification", e);
        }
    }

    /**
     * Name of the saga/PM that issued the rejected command.
     */
    public String getIssuerName() {
        return issuerName;
    }

    /**
     * Type of issuer: "saga" or "process_manager".
     */
    public String getIssuerType() {
        return issuerType;
    }

    /**
     * Sequence of the event that triggered the saga/PM flow.
     */
    public int getSourceEventSequence() {
        return sourceEventSequence;
    }

    /**
     * Why the command was rejected.
     */
    public String getRejectionReason() {
        return rejectionReason;
    }

    /**
     * The command that was rejected (if available).
     */
    public CommandBook getRejectedCommand() {
        return rejectedCommand;
    }

    /**
     * Cover of the aggregate that triggered the flow.
     */
    public Cover getSourceAggregate() {
        return sourceAggregate;
    }

    /**
     * Get the type URL of the rejected command, if available.
     */
    public String getRejectedCommandType() {
        if (rejectedCommand == null || rejectedCommand.getPagesList().isEmpty()) {
            return null;
        }
        var page = rejectedCommand.getPages(0);
        if (!page.hasCommand()) {
            return null;
        }
        return page.getCommand().getTypeUrl();
    }

    /**
     * Build dispatch key for routing: "domain/command".
     */
    public String dispatchKey() {
        if (rejectedCommand == null || !rejectedCommand.hasCover()) {
            return "";
        }
        String domain = rejectedCommand.getCover().getDomain();
        String commandType = getRejectedCommandType();
        if (commandType == null) {
            return "";
        }
        String simpleName = Helpers.typeNameFromUrl(commandType);
        // Strip package prefix if present (e.g., "myapp.ProcessPayment" -> "ProcessPayment")
        int dotIdx = simpleName.lastIndexOf('.');
        if (dotIdx >= 0) {
            simpleName = simpleName.substring(dotIdx + 1);
        }
        return domain + "/" + simpleName;
    }
}
