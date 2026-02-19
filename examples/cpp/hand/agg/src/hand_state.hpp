#pragma once

#include <string>
#include <vector>
#include <unordered_map>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/poker_types.pb.h"

namespace hand {

struct Card {
    examples::Suit suit = examples::SUIT_UNSPECIFIED;
    int rank = 0;

    bool operator==(const Card& other) const {
        return suit == other.suit && rank == other.rank;
    }
};

struct PlayerHandInfo {
    std::string player_root;
    int position = 0;
    std::vector<Card> hole_cards;
    int64_t stack = 0;
    int64_t bet_this_round = 0;
    int64_t total_invested = 0;
    bool has_acted = false;
    bool has_folded = false;
    bool is_all_in = false;
};

struct PotInfo {
    int64_t amount = 0;
    std::vector<std::string> eligible_players;
    std::string pot_type = "main";
};

struct HandState {
    std::string hand_id;
    std::string table_root;
    int64_t hand_number = 0;
    examples::GameVariant game_variant = examples::GAME_VARIANT_UNSPECIFIED;
    std::vector<Card> remaining_deck;
    std::unordered_map<int, PlayerHandInfo> players;
    std::vector<Card> community_cards;
    examples::BettingPhase current_phase = examples::BETTING_PHASE_UNSPECIFIED;
    int action_on_position = -1;
    int64_t current_bet = 0;
    int64_t min_raise = 0;
    std::vector<PotInfo> pots;
    int dealer_position = 0;
    int small_blind_position = 0;
    int big_blind_position = 0;
    int64_t small_blind = 0;
    int64_t big_blind = 0;
    std::string status;

    bool exists() const { return !status.empty(); }
    int64_t get_pot_total() const;
    const PlayerHandInfo* get_player(const std::string& player_root) const;
    PlayerHandInfo* get_player_mut(const std::string& player_root);
    std::vector<const PlayerHandInfo*> get_active_players() const;
    std::vector<const PlayerHandInfo*> get_players_in_hand() const;

    static HandState from_event_book(const angzarr::EventBook& event_book);
    static void apply_event(HandState& state, const google::protobuf::Any& event_any);
};

} // namespace hand
