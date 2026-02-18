#include "hand.hpp"
#include "deal_handler.hpp"
#include "post_blind_handler.hpp"
#include "action_handler.hpp"
#include "deal_community_handler.hpp"
#include "award_pot_handler.hpp"

namespace hand {

examples::CardsDealt Hand::deal(const examples::DealCards& cmd) {
    return handlers::handle_deal(cmd, state_);
}

examples::BlindPosted Hand::post_blind(const examples::PostBlind& cmd) {
    return handlers::handle_post_blind(cmd, state_);
}

examples::ActionTaken Hand::action(const examples::PlayerAction& cmd) {
    return handlers::handle_action(cmd, state_);
}

examples::CommunityCardsDealt Hand::deal_community(const examples::DealCommunityCards& cmd) {
    return handlers::handle_deal_community(cmd, state_);
}

std::pair<examples::PotAwarded, examples::HandComplete> Hand::award_pot(const examples::AwardPot& cmd) {
    return handlers::handle_award_pot(cmd, state_);
}

} // namespace hand
