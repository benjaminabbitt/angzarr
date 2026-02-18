#include "deal_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>
#include <google/protobuf/util/time_util.h>
#include <random>
#include <algorithm>

namespace hand {
namespace handlers {

examples::CardsDealt handle_deal(
    const examples::DealCards& cmd,
    const HandState& state) {

    // Guard
    if (state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Hand already dealt");
    }

    // Validate
    if (cmd.players().empty()) {
        throw angzarr::CommandRejectedError::invalid_argument("No players in hand");
    }
    if (cmd.players_size() < 2) {
        throw angzarr::CommandRejectedError::invalid_argument("Need at least 2 players");
    }

    // Compute - build deck and deal cards
    std::vector<Card> deck;
    std::vector<examples::Suit> suits = {
        examples::CLUBS, examples::DIAMONDS,
        examples::HEARTS, examples::SPADES
    };
    for (auto suit : suits) {
        for (int rank = 2; rank <= 14; ++rank) {
            deck.push_back(Card{suit, rank});
        }
    }

    // Shuffle with seed if provided
    if (!cmd.deck_seed().empty()) {
        // Convert bytes to seed - use first 8 bytes as uint64
        uint64_t seed = 0;
        const std::string& seed_bytes = cmd.deck_seed();
        for (size_t i = 0; i < std::min(seed_bytes.size(), size_t(8)); ++i) {
            seed = (seed << 8) | static_cast<uint8_t>(seed_bytes[i]);
        }
        std::mt19937 g(seed);
        std::shuffle(deck.begin(), deck.end(), g);
    } else {
        std::random_device rd;
        std::mt19937 g(rd());
        std::shuffle(deck.begin(), deck.end(), g);
    }

    // Determine cards per player based on game variant
    int cards_per_player = 2;  // Texas Hold'em default
    if (cmd.game_variant() == examples::OMAHA) {
        cards_per_player = 4;
    } else if (cmd.game_variant() == examples::FIVE_CARD_DRAW) {
        cards_per_player = 5;
    }

    auto now = std::chrono::system_clock::now();
    auto timestamp = google::protobuf::util::TimeUtil::TimeTToTimestamp(
        std::chrono::system_clock::to_time_t(now));

    examples::CardsDealt event;
    event.set_table_root(cmd.table_root());
    event.set_hand_number(cmd.hand_number());
    event.set_game_variant(cmd.game_variant());
    event.set_dealer_position(cmd.dealer_position());
    *event.mutable_dealt_at() = timestamp;

    // Copy players
    for (const auto& player : cmd.players()) {
        *event.add_players() = player;
    }

    // Deal hole cards
    int deck_idx = 0;
    for (const auto& player : cmd.players()) {
        auto* pc = event.add_player_cards();
        pc->set_player_root(player.player_root());
        for (int i = 0; i < cards_per_player && deck_idx < static_cast<int>(deck.size()); ++i) {
            auto* card = pc->add_cards();
            card->set_suit(deck[deck_idx].suit);
            card->set_rank(static_cast<examples::Rank>(deck[deck_idx].rank));
            ++deck_idx;
        }
    }

    return event;
}

} // namespace handlers
} // namespace hand
