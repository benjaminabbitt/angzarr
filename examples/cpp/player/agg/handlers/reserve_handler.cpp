#include "reserve_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <iomanip>
#include <sstream>

namespace player {
namespace handlers {

namespace {
std::string bytes_to_hex(const std::string& bytes) {
    std::ostringstream ss;
    for (unsigned char c : bytes) {
        ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
    }
    return ss.str();
}
} // anonymous namespace

examples::FundsReserved handle_reserve(const examples::ReserveFunds& cmd, const PlayerState& state) {
    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Player does not exist");
    }

    // Validate
    int64_t amount = cmd.has_amount() ? cmd.amount().amount() : 0;
    if (amount <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("amount must be positive");
    }

    std::string table_key = bytes_to_hex(cmd.table_root());
    if (state.table_reservations.count(table_key) > 0) {
        throw angzarr::CommandRejectedError::precondition_failed("Funds already reserved for this table");
    }
    if (amount > state.available_balance()) {
        throw angzarr::CommandRejectedError::precondition_failed("Insufficient funds");
    }

    // Compute
    int64_t new_reserved = state.reserved_funds + amount;
    int64_t new_available = state.bankroll - new_reserved;

    examples::FundsReserved event;
    event.mutable_amount()->CopyFrom(cmd.amount());
    event.set_table_root(cmd.table_root());
    event.mutable_new_available_balance()->set_amount(new_available);
    event.mutable_new_available_balance()->set_currency_code("CHIPS");
    event.mutable_new_reserved_balance()->set_amount(new_reserved);
    event.mutable_new_reserved_balance()->set_currency_code("CHIPS");

    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
    event.mutable_reserved_at()->set_seconds(seconds);

    return event;
}

} // namespace handlers
} // namespace player
