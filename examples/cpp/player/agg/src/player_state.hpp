#pragma once

#include <string>
#include <unordered_map>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "examples/player.pb.h"
#include "examples/types.pb.h"

namespace player {

/// Player aggregate state.
struct PlayerState {
    std::string player_id;
    std::string display_name;
    std::string email;
    examples::PlayerType player_type = examples::PlayerType::PLAYER_TYPE_UNSPECIFIED;
    std::string ai_model_id;
    int64_t bankroll = 0;
    int64_t reserved_funds = 0;
    std::unordered_map<std::string, int64_t> table_reservations;
    std::string status;

    bool exists() const { return !player_id.empty(); }
    int64_t available_balance() const { return bankroll - reserved_funds; }
    bool is_ai() const { return player_type == examples::PlayerType::AI; }

    /// Build state from an EventBook by applying all events.
    static PlayerState from_event_book(const angzarr::EventBook& event_book);

    /// Apply a single event to the state.
    static void apply_event(PlayerState& state, const google::protobuf::Any& event_any);
};

} // namespace player
