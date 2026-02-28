#include "rejected_handler.hpp"

#include <chrono>

#include "angzarr/compensation.hpp"
#include "examples/poker_types.pb.h"

namespace player {
namespace handlers {

// docs:start:rejected_handler
examples::FundsReleased handle_join_rejected(const angzarr::Notification& notification,
                                             const PlayerState& state) {
    // Extract compensation context from the notification
    auto ctx = angzarr::CompensationContext::from_notification(notification);

    // Get table_root from the notification cover (the target aggregate)
    std::string table_key;
    if (notification.has_cover() && !notification.cover().root().empty()) {
        // Convert root bytes to hex string for lookup
        const std::string& root = notification.cover().root();
        table_key.reserve(root.size() * 2);
        for (unsigned char c : root) {
            static const char hex[] = "0123456789abcdef";
            table_key.push_back(hex[c >> 4]);
            table_key.push_back(hex[c & 0x0f]);
        }
    }

    // Get the amount reserved for this table
    int64_t reserved_amount = 0;
    auto it = state.table_reservations.find(table_key);
    if (it != state.table_reservations.end()) {
        reserved_amount = it->second;
    }

    // Compute new balances after release
    int64_t new_reserved = state.reserved_funds - reserved_amount;
    int64_t new_available = state.bankroll - new_reserved;

    // Build FundsReleased event
    examples::FundsReleased event;
    event.mutable_amount()->set_amount(reserved_amount);
    event.mutable_amount()->set_currency_code("CHIPS");

    if (notification.has_cover()) {
        event.set_table_root(notification.cover().root());
    }

    event.mutable_new_available_balance()->set_amount(new_available);
    event.mutable_new_available_balance()->set_currency_code("CHIPS");

    event.mutable_new_reserved_balance()->set_amount(new_reserved);
    event.mutable_new_reserved_balance()->set_currency_code("CHIPS");

    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
    event.mutable_released_at()->set_seconds(seconds);

    return event;
}
// docs:end:rejected_handler

}  // namespace handlers
}  // namespace player
