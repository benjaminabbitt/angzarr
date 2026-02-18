#pragma once

#include "table_state.hpp"
#include "angzarr/aggregate.hpp"
#include "angzarr/errors.hpp"
#include "examples/table.pb.h"

namespace table {

/// Table aggregate - OO style implementation.
/// Uses CRTP pattern with Aggregate base class.
class Table : public angzarr::Aggregate<Table, TableState> {
public:
    static constexpr const char* DOMAIN = "table";

    // State accessors
    bool exists() const { return state_.exists(); }
    const std::string& table_id() const { return state_.table_id; }
    const std::string& table_name() const { return state_.table_name; }
    examples::GameVariant game_variant() const { return state_.game_variant; }
    int64_t small_blind() const { return state_.small_blind; }
    int64_t big_blind() const { return state_.big_blind; }
    int64_t min_buy_in() const { return state_.min_buy_in; }
    int64_t max_buy_in() const { return state_.max_buy_in; }
    int max_players() const { return state_.max_players; }
    int player_count() const { return state_.player_count(); }
    int active_player_count() const { return state_.active_player_count(); }
    bool is_full() const { return state_.is_full(); }
    int dealer_position() const { return state_.dealer_position; }
    int64_t hand_count() const { return state_.hand_count; }
    const std::string& current_hand_root() const { return state_.current_hand_root; }
    const std::string& status() const { return state_.status; }

    const SeatState* get_seat(int position) const {
        return state_.get_seat(position);
    }

    const SeatState* find_player_seat(const std::string& player_root) const {
        return state_.find_player_seat(player_root);
    }

    // Command handlers (OO style)
    examples::TableCreated create(const examples::CreateTable& cmd);
    examples::PlayerJoined join(const examples::JoinTable& cmd);
    examples::PlayerLeft leave(const examples::LeaveTable& cmd);
    examples::HandStarted start_hand(const examples::StartHand& cmd);
    examples::HandEnded end_hand(const examples::EndHand& cmd);

protected:
    friend class angzarr::Aggregate<Table, TableState>;

    TableState create_empty_state() const {
        return TableState{};
    }

    void apply_event_impl(TableState& state, const google::protobuf::Any& event_any) {
        TableState::apply_event(state, event_any);
    }
};

} // namespace table
