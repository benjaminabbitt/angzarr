#include "deal_community_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>

namespace hand {
namespace handlers {

namespace {

struct PhaseTransition {
    examples::BettingPhase next_phase;
    int cards_to_deal;
};

PhaseTransition get_next_phase(examples::GameVariant variant, examples::BettingPhase current) {
    // Five Card Draw doesn't have community cards
    if (variant == examples::FIVE_CARD_DRAW) {
        return {examples::BETTING_PHASE_UNSPECIFIED, 0};
    }

    // Standard Texas Hold'em/Omaha phase transitions
    switch (current) {
        case examples::PREFLOP:
            return {examples::FLOP, 3};
        case examples::FLOP:
            return {examples::TURN, 1};
        case examples::TURN:
            return {examples::RIVER, 1};
        default:
            return {examples::BETTING_PHASE_UNSPECIFIED, 0};
    }
}

} // anonymous namespace

examples::CommunityCardsDealt handle_deal_community(
    const examples::DealCommunityCards& cmd,
    const HandState& state) {

    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::not_found("Hand not dealt");
    }
    if (state.status == "complete") {
        throw angzarr::CommandRejectedError::precondition_failed("Hand is complete");
    }

    // Validate
    if (cmd.count() <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("Must deal at least 1 card");
    }

    if (state.game_variant == examples::FIVE_CARD_DRAW) {
        throw angzarr::CommandRejectedError::precondition_failed(
            "Five card draw doesn't have community cards");
    }

    auto transition = get_next_phase(state.game_variant, state.current_phase);
    if (transition.next_phase == examples::BETTING_PHASE_UNSPECIFIED) {
        throw angzarr::CommandRejectedError::precondition_failed("No more phases");
    }
    if (transition.cards_to_deal != cmd.count()) {
        throw angzarr::CommandRejectedError::invalid_argument(
            "Expected " + std::to_string(transition.cards_to_deal) + " cards for this phase");
    }
    if (static_cast<int>(state.remaining_deck.size()) < cmd.count()) {
        throw angzarr::CommandRejectedError::precondition_failed("Not enough cards in deck");
    }

    // Compute
    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::CommunityCardsDealt event;
    event.set_phase(transition.next_phase);
    *event.mutable_dealt_at() = timestamp;

    // Deal new cards
    for (int i = 0; i < cmd.count(); ++i) {
        const auto& card = state.remaining_deck[i];
        auto* c = event.add_cards();
        c->set_suit(card.suit);
        c->set_rank(static_cast<examples::Rank>(card.rank));
    }

    // Include all community cards (existing + new)
    for (const auto& card : state.community_cards) {
        auto* c = event.add_all_community_cards();
        c->set_suit(card.suit);
        c->set_rank(static_cast<examples::Rank>(card.rank));
    }
    for (int i = 0; i < cmd.count(); ++i) {
        const auto& card = state.remaining_deck[i];
        auto* c = event.add_all_community_cards();
        c->set_suit(card.suit);
        c->set_rank(static_cast<examples::Rank>(card.rank));
    }

    return event;
}

} // namespace handlers
} // namespace hand
