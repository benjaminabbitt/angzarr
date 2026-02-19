#pragma once

#include <string>
#include <vector>
#include <unordered_map>
#include <functional>
#include <optional>
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/table.pb.h"
#include "examples/poker_types.pb.h"

namespace hand_flow {

/// Internal state machine phases for hand orchestration.
enum class HandPhase {
    WAITING_FOR_START,
    DEALING,
    POSTING_BLINDS,
    BETTING,
    DEALING_COMMUNITY,
    DRAW,
    SHOWDOWN,
    AWARDING_POT,
    COMPLETE
};

/// Tracks a player's state within the process manager.
struct PlayerState {
    std::string player_root;
    int position = 0;
    int64_t stack = 0;
    int64_t bet_this_round = 0;
    int64_t total_invested = 0;
    bool has_acted = false;
    bool has_folded = false;
    bool is_all_in = false;
};

/// Process manager state for a single hand.
struct HandProcess {
    std::string hand_id;
    std::string table_root;
    int64_t hand_number = 0;
    examples::GameVariant game_variant = examples::GAME_VARIANT_UNSPECIFIED;

    // State machine
    HandPhase phase = HandPhase::WAITING_FOR_START;
    examples::BettingPhase betting_phase = examples::BETTING_PHASE_UNSPECIFIED;

    // Player tracking
    std::unordered_map<int, PlayerState> players;
    std::vector<int> active_positions;

    // Position tracking
    int dealer_position = 0;
    int small_blind_position = 0;
    int big_blind_position = 0;
    int action_on = -1;
    int last_aggressor = -1;

    // Betting state
    int64_t small_blind = 0;
    int64_t big_blind = 0;
    int64_t current_bet = 0;
    int64_t min_raise = 0;
    int64_t pot_total = 0;

    // Blind posting progress
    bool small_blind_posted = false;
    bool big_blind_posted = false;

    // Timeout handling
    int action_timeout_seconds = 30;
};

/// Callback type for sending commands.
using CommandSender = std::function<void(const angzarr::CommandBook&)>;

/// Orchestrates the flow of a poker hand.
class HandProcessManager {
public:
    explicit HandProcessManager(CommandSender command_sender);

    /// Get process state for a hand.
    HandProcess* get_process(const std::string& hand_id);

    /// Initialize process for a new hand (from HandStarted event).
    std::optional<angzarr::CommandBook> start_hand(const examples::HandStarted& event);

    /// Handle CardsDealt event.
    std::optional<angzarr::CommandBook> handle_cards_dealt(const examples::CardsDealt& event);

    /// Handle BlindPosted event.
    std::optional<angzarr::CommandBook> handle_blind_posted(const examples::BlindPosted& event);

    /// Handle ActionTaken event.
    std::optional<angzarr::CommandBook> handle_action_taken(const examples::ActionTaken& event);

    /// Handle CommunityCardsDealt event.
    std::optional<angzarr::CommandBook> handle_community_cards_dealt(
        const examples::CommunityCardsDealt& event);

    /// Handle ShowdownStarted event.
    std::optional<angzarr::CommandBook> handle_showdown_started(
        const examples::ShowdownStarted& event);

    /// Handle PotAwarded event.
    void handle_pot_awarded(const examples::PotAwarded& event);

private:
    std::optional<angzarr::CommandBook> post_next_blind(HandProcess& process);
    angzarr::CommandBook build_post_blind_cmd(
        const HandProcess& process,
        const PlayerState& player,
        const std::string& blind_type,
        int64_t amount);
    void start_betting(HandProcess& process);
    void advance_action(HandProcess& process);
    int find_next_active(const HandProcess& process, int after_position) const;
    bool is_betting_complete(const HandProcess& process) const;
    std::optional<angzarr::CommandBook> end_betting_round(HandProcess& process);
    angzarr::CommandBook build_deal_community_cmd(const HandProcess& process, int count);
    angzarr::CommandBook build_award_pot_cmd(const HandProcess& process);

    CommandSender command_sender_;
    std::unordered_map<std::string, HandProcess> processes_;
};

} // namespace hand_flow
