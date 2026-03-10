#include "end_hand_handler.hpp"

#include <google/protobuf/util/time_util.h>

#include <chrono>

#include "angzarr/errors.hpp"

namespace table {
namespace handlers {

examples::HandEnded handle_end_hand(const examples::EndHand& cmd, const TableState& state) {
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
        // Use winner_root directly as the key (it's a string-type player identifier)
        const std::string& player_root = result.winner_root();

        if (stack_changes.find(player_root) == stack_changes.end()) {
            stack_changes[player_root] = 0;
        }
        stack_changes[player_root] += result.amount();
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

}  // namespace handlers
}  // namespace table
