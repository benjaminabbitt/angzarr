#include "start_hand_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>
#include <algorithm>
#include <vector>
#include <random>
#include <sstream>
#include <iomanip>

namespace table {
namespace handlers {

namespace {

// Generate a deterministic hand root from table_id and hand_number
std::string generate_hand_root(const std::string& table_id, int64_t hand_number) {
    // Simple hash-based generation for determinism
    std::hash<std::string> hasher;
    std::string input = table_id + "." + std::to_string(hand_number);
    size_t hash1 = hasher(input);
    size_t hash2 = hasher(input + ".salt");

    std::stringstream ss;
    ss << std::hex << std::setfill('0');
    ss << std::setw(16) << hash1;
    ss << std::setw(16) << hash2;
    return ss.str().substr(0, 16);  // 16 bytes as hex string
}

} // anonymous namespace

examples::HandStarted handle_start_hand(
    const examples::StartHand& cmd,
    const TableState& state) {

    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::not_found("Table does not exist");
    }
    if (state.status == "in_hand") {
        throw angzarr::CommandRejectedError::precondition_failed("Hand already in progress");
    }
    if (state.active_player_count() < 2) {
        throw angzarr::CommandRejectedError::precondition_failed("Not enough players to start hand");
    }

    // Compute
    int64_t hand_number = state.hand_count + 1;
    std::string hand_root = generate_hand_root(state.table_id, hand_number);

    int dealer_position = state.next_dealer_position();

    // Get active player positions
    std::vector<int> active_positions;
    for (const auto& [pos, seat] : state.seats) {
        if (!seat.is_sitting_out) {
            active_positions.push_back(pos);
        }
    }
    std::sort(active_positions.begin(), active_positions.end());

    // Find dealer index
    int dealer_idx = 0;
    for (size_t i = 0; i < active_positions.size(); ++i) {
        if (active_positions[i] == dealer_position) {
            dealer_idx = static_cast<int>(i);
            break;
        }
    }

    // Find blind positions
    int sb_position, bb_position;
    if (active_positions.size() == 2) {
        // Heads-up: dealer is SB
        sb_position = active_positions[dealer_idx];
        bb_position = active_positions[(dealer_idx + 1) % 2];
    } else {
        sb_position = active_positions[(dealer_idx + 1) % active_positions.size()];
        bb_position = active_positions[(dealer_idx + 2) % active_positions.size()];
    }

    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::HandStarted event;
    event.set_hand_root(hand_root);
    event.set_hand_number(hand_number);
    event.set_dealer_position(dealer_position);
    event.set_small_blind_position(sb_position);
    event.set_big_blind_position(bb_position);
    event.set_game_variant(state.game_variant);
    event.set_small_blind(state.small_blind);
    event.set_big_blind(state.big_blind);
    *event.mutable_started_at() = timestamp;

    // Build active players list
    for (int pos : active_positions) {
        const SeatState* seat = state.get_seat(pos);
        if (seat) {
            auto* snapshot = event.add_active_players();
            snapshot->set_position(pos);
            snapshot->set_player_root(seat->player_root);
            snapshot->set_stack(seat->stack);
        }
    }

    return event;
}

} // namespace handlers
} // namespace table
