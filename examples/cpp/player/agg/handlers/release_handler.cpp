#include "release_handler.hpp"
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

examples::FundsReleased handle_release(const examples::ReleaseFunds& cmd, const PlayerState& state) {
    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Player does not exist");
    }

    std::string table_key = bytes_to_hex(cmd.table_root());
    auto it = state.table_reservations.find(table_key);
    if (it == state.table_reservations.end() || it->second == 0) {
        throw angzarr::CommandRejectedError::precondition_failed("No funds reserved for this table");
    }

    // Compute
    int64_t reserved_for_table = it->second;
    int64_t new_reserved = state.reserved_funds - reserved_for_table;
    int64_t new_available = state.bankroll - new_reserved;

    examples::FundsReleased event;
    event.mutable_amount()->set_amount(reserved_for_table);
    event.mutable_amount()->set_currency_code("CHIPS");
    event.set_table_root(cmd.table_root());
    event.mutable_new_available_balance()->set_amount(new_available);
    event.mutable_new_available_balance()->set_currency_code("CHIPS");
    event.mutable_new_reserved_balance()->set_amount(new_reserved);
    event.mutable_new_reserved_balance()->set_currency_code("CHIPS");

    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
    event.mutable_released_at()->set_seconds(seconds);

    return event;
}

} // namespace handlers
} // namespace player
