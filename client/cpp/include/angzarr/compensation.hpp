#pragma once

#include <string>
#include <optional>
#include "angzarr/types.pb.h"
#include "helpers.hpp"

namespace angzarr {

/**
 * Extracted context from a rejection Notification.
 *
 * When a saga or process manager issues a command that gets rejected, the framework
 * sends a Notification containing rejection details. CompensationContext extracts
 * this information into a developer-friendly structure.
 *
 * **Why this matters:**
 * - Debugging: Understand which component issued the failing command
 * - Compensation logic: Decide whether to retry, rollback, or escalate
 * - Observability: Log structured rejection data for monitoring
 * - Business rules: Different compensation for different rejection reasons
 *
 * Without CompensationContext, developers must manually unpack nested protobuf
 * messages (Notification -> Any -> RejectionNotification -> fields), which is
 * error-prone and obscures the business logic in boilerplate.
 *
 * Example:
 *   auto ctx = CompensationContext::from_notification(notification);
 *   if (ctx.issuer_name() == "saga-order-fulfill") {
 *       // Emit compensation events
 *       OrderCancelled cancel;
 *       cancel.set_reason(ctx.rejection_reason());
 *       return emit_compensation_events(new_event_book(cancel));
 *   }
 */
class CompensationContext {
public:
    /**
     * Extract compensation context from a Notification.
     *
     * If the notification payload is not a RejectionNotification or cannot
     * be unpacked, returns a context with default/empty values.
     *
     * @param notification The notification containing rejection details
     * @return A new CompensationContext
     */
    static CompensationContext from_notification(const Notification& notification) {
        CompensationContext ctx;

        if (notification.has_payload()) {
            RejectionNotification rejection;
            if (notification.payload().UnpackTo(&rejection)) {
                ctx.issuer_name_ = rejection.issuer_name();
                ctx.issuer_type_ = rejection.issuer_type();
                ctx.source_event_sequence_ = rejection.source_event_sequence();
                ctx.rejection_reason_ = rejection.rejection_reason();

                if (rejection.has_rejected_command()) {
                    ctx.rejected_command_ = rejection.rejected_command();
                }
                if (rejection.has_source_aggregate()) {
                    ctx.source_aggregate_ = rejection.source_aggregate();
                }
            }
        }

        return ctx;
    }

    /**
     * Name of the saga/PM that issued the rejected command.
     */
    const std::string& issuer_name() const { return issuer_name_; }

    /**
     * Type of issuer: "saga" or "process_manager".
     */
    const std::string& issuer_type() const { return issuer_type_; }

    /**
     * Sequence of the event that triggered the saga/PM flow.
     */
    uint32_t source_event_sequence() const { return source_event_sequence_; }

    /**
     * Why the command was rejected.
     */
    const std::string& rejection_reason() const { return rejection_reason_; }

    /**
     * The command that was rejected (if available).
     */
    const std::optional<CommandBook>& rejected_command() const { return rejected_command_; }

    /**
     * Cover of the aggregate that triggered the flow.
     */
    const std::optional<Cover>& source_aggregate() const { return source_aggregate_; }

    /**
     * Get the type URL of the rejected command, if available.
     *
     * Compensation handlers are often keyed by command type:
     * "If ReserveStock was rejected, release the hold."
     *
     * @return The type name suffix (e.g., "ReserveStock") or empty string
     */
    std::string rejected_command_type() const {
        if (!rejected_command_.has_value() || rejected_command_->pages_size() == 0) {
            return "";
        }

        const auto& page = rejected_command_->pages(0);
        if (!page.has_command()) {
            return "";
        }

        return helpers::type_name_from_url(page.command().type_url());
    }

    /**
     * Build a dispatch key for routing rejection handlers.
     *
     * @return A key in format "domain/command" or empty string
     */
    std::string dispatch_key() const {
        if (!rejected_command_.has_value()) {
            return "";
        }

        std::string domain = rejected_command_->has_cover()
            ? rejected_command_->cover().domain() : "";
        std::string cmd_type = rejected_command_type();

        if (domain.empty() || cmd_type.empty()) {
            return "";
        }

        return domain + "/" + cmd_type;
    }

private:
    std::string issuer_name_;
    std::string issuer_type_;
    uint32_t source_event_sequence_ = 0;
    std::string rejection_reason_;
    std::optional<CommandBook> rejected_command_;
    std::optional<Cover> source_aggregate_;
};

} // namespace angzarr
