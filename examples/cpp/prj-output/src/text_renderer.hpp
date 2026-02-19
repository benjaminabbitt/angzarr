#pragma once

#include <string>
#include <unordered_map>
#include <functional>
#include <google/protobuf/message.h>
#include "examples/player.pb.h"
#include "examples/table.pb.h"
#include "examples/hand.pb.h"
#include "examples/poker_types.pb.h"

namespace projector {

/// Text renderer for poker events.
class TextRenderer {
public:
    TextRenderer() = default;

    /// Set display name for a player.
    void set_player_name(const std::string& player_root, const std::string& name);

    /// Get display name for a player.
    std::string get_player_name(const std::string& player_root) const;

    /// Render a card as text.
    static std::string render_card(const examples::Card& card);

    /// Render an action type as text.
    static std::string render_action(examples::ActionType action);

    // Event renderers
    std::string render_player_registered(const examples::PlayerRegistered& event);
    std::string render_funds_deposited(const examples::FundsDeposited& event);
    std::string render_funds_withdrawn(const examples::FundsWithdrawn& event);
    std::string render_funds_reserved(const examples::FundsReserved& event);
    std::string render_funds_released(const examples::FundsReleased& event);

    std::string render_table_created(const examples::TableCreated& event);
    std::string render_player_joined(const examples::PlayerJoined& event);
    std::string render_player_left(const examples::PlayerLeft& event);
    std::string render_hand_started(const examples::HandStarted& event);
    std::string render_hand_ended(const examples::HandEnded& event);

    std::string render_cards_dealt(const examples::CardsDealt& event);
    std::string render_blind_posted(const examples::BlindPosted& event);
    std::string render_action_taken(const examples::ActionTaken& event);
    std::string render_community_cards_dealt(const examples::CommunityCardsDealt& event);
    std::string render_pot_awarded(const examples::PotAwarded& event);
    std::string render_hand_complete(const examples::HandComplete& event);

private:
    std::unordered_map<std::string, std::string> player_names_;
};

} // namespace projector
