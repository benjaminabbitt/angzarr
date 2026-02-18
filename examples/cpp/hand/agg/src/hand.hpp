#pragma once

#include "hand_state.hpp"
#include "angzarr/aggregate.hpp"
#include "angzarr/errors.hpp"
#include "examples/hand.pb.h"
#include <utility>

namespace hand {

/// Hand aggregate - OO style implementation.
/// Uses CRTP pattern with Aggregate base class.
class Hand : public angzarr::Aggregate<Hand, HandState> {
public:
    static constexpr const char* DOMAIN = "hand";

    // State accessors
    bool exists() const { return state_.exists(); }
    const std::string& hand_id() const { return state_.hand_id; }
    const std::string& table_root() const { return state_.table_root; }
    int64_t hand_number() const { return state_.hand_number; }
    examples::GameVariant game_variant() const { return state_.game_variant; }
    const std::string& status() const { return state_.status; }
    examples::BettingPhase current_phase() const { return state_.current_phase; }
    int64_t current_bet() const { return state_.current_bet; }
    int64_t min_raise() const { return state_.min_raise; }
    int64_t small_blind() const { return state_.small_blind; }
    int64_t big_blind() const { return state_.big_blind; }
    int64_t get_pot_total() const { return state_.get_pot_total(); }

    const PlayerHandInfo* get_player(const std::string& player_root) const {
        return state_.get_player(player_root);
    }

    std::vector<const PlayerHandInfo*> get_active_players() const {
        return state_.get_active_players();
    }

    std::vector<const PlayerHandInfo*> get_players_in_hand() const {
        return state_.get_players_in_hand();
    }

    // Command handlers (OO style)
    examples::CardsDealt deal(const examples::DealCards& cmd);
    examples::BlindPosted post_blind(const examples::PostBlind& cmd);
    examples::ActionTaken action(const examples::PlayerAction& cmd);
    examples::CommunityCardsDealt deal_community(const examples::DealCommunityCards& cmd);
    std::pair<examples::PotAwarded, examples::HandComplete> award_pot(const examples::AwardPot& cmd);

protected:
    friend class angzarr::Aggregate<Hand, HandState>;

    HandState create_empty_state() const {
        return HandState{};
    }

    void apply_event_impl(HandState& state, const google::protobuf::Any& event_any) {
        HandState::apply_event(state, event_any);
    }
};

} // namespace hand
