#include "end_hand_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>
#include <sstream>
#include <iomanip>

namespace table {
namespace handlers {

examples::HandEnded handle_end_hand(
    const examples::EndHand& cmd,
    const TableState& state) {

    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::not_found("Table does not exist");
    }
    if (state.status != "in_hand") {
        throw angzarr::CommandRejectedError::precondition_failed("No hand in progress");
    }
    if (cmd.hand_root() != state.current_hand_root) {
        throw angzarr::CommandRejectedError::invalid_argument("Hand root mismatch");
    }

    // Compute stack changes from results
    std::map<std::string, int64_t> stack_changes;
    for (const auto& result : cmd.results()) {
        // Convert winner_root bytes to hex string
        std::stringstream ss;
        for (unsigned char c : result.winner_root()) {
            ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
        }
        std::string player_hex = ss.str();

        if (stack_changes.find(player_hex) == stack_changes.end()) {
            stack_changes[player_hex] = 0;
        }
        stack_changes[player_hex] += result.amount();
    }

    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::HandEnded event;
    event.set_hand_root(cmd.hand_root());
    *event.mutable_ended_at() = timestamp;

    // Copy stack changes
    for (const auto& [player_hex, delta] : stack_changes) {
        (*event.mutable_stack_changes())[player_hex] = delta;
    }

    // Copy results
    for (const auto& result : cmd.results()) {
        *event.add_results() = result;
    }

    return event;
}

} // namespace handlers
} // namespace table
