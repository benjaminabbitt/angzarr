#pragma once

#include <map>
#include <optional>
#include <string>
#include <vector>

#include "examples/poker_types.pb.h"

namespace pm_context {

enum class HandPhase {
    DEALING,
    POSTING_BLINDS,
    BETTING,
    DEALING_COMMUNITY,
    DRAW,
    SHOWDOWN,
    COMPLETE
};

struct PlayerPMState {
    std::string player_root;
    int32_t position = 0;
    int64_t stack = 0;
    int64_t bet_this_round = 0;
    bool has_folded = false;
    bool is_all_in = false;
    bool has_acted = false;
};

struct HandProcess {
    std::string hand_id;
    HandPhase phase = HandPhase::DEALING;
    examples::GameVariant game_variant = examples::TEXAS_HOLDEM;
    examples::BettingPhase betting_phase = examples::PREFLOP;
    int32_t dealer_position = 0;
    int32_t action_on = 0;
    int64_t small_blind = 0;
    int64_t big_blind = 0;
    int64_t current_bet = 0;
    int64_t pot_total = 0;
    int64_t min_raise = 0;
    bool small_blind_posted = false;
    bool big_blind_posted = false;
    std::vector<PlayerPMState> players;
};

struct PMTestState {
    std::optional<HandProcess> process;
    std::vector<std::pair<std::string, google::protobuf::Message*>> commands_sent;
    std::string last_output;
    bool timeout_triggered = false;

    // Track pending event for processing
    std::optional<examples::ActionType> pending_action;
    int pending_action_position = -1;

    // Track pending blind event
    std::string pending_blind_type;  // "small" or "big"

    void reset() {
        process.reset();
        for (auto& [name, msg] : commands_sent) {
            delete msg;
        }
        commands_sent.clear();
        last_output.clear();
        timeout_triggered = false;
        pending_action.reset();
        pending_action_position = -1;
        pending_blind_type.clear();
    }

    ~PMTestState() {
        for (auto& [name, msg] : commands_sent) {
            delete msg;
        }
    }
};

// Global PM state - accessible from multiple step files
extern thread_local PMTestState g_pm_state;

}  // namespace pm_context
