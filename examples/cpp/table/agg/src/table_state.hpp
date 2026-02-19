#pragma once

#include <string>
#include <unordered_map>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "examples/table.pb.h"
#include "examples/poker_types.pb.h"

namespace table {

struct SeatState {
    int position = 0;
    std::string player_root;
    int64_t stack = 0;
    bool is_active = true;
    bool is_sitting_out = false;
};

struct TableState {
    std::string table_id;
    std::string table_name;
    examples::GameVariant game_variant = examples::GameVariant::GAME_VARIANT_UNSPECIFIED;
    int64_t small_blind = 0;
    int64_t big_blind = 0;
    int64_t min_buy_in = 0;
    int64_t max_buy_in = 0;
    int max_players = 9;
    int action_timeout_seconds = 30;
    std::unordered_map<int, SeatState> seats;
    int dealer_position = 0;
    int64_t hand_count = 0;
    std::string current_hand_root;
    std::string status;

    bool exists() const { return !table_id.empty(); }
    int player_count() const { return static_cast<int>(seats.size()); }
    int active_player_count() const;
    bool is_full() const { return static_cast<int>(seats.size()) >= max_players; }

    const SeatState* get_seat(int position) const;
    const SeatState* find_player_seat(const std::string& player_root) const;
    int find_available_seat(int preferred = -1) const;
    int next_dealer_position() const;

    static TableState from_event_book(const angzarr::EventBook& event_book);
    static void apply_event(TableState& state, const google::protobuf::Any& event_any);
};

} // namespace table
