#pragma once

#include <algorithm>
#include <map>
#include <string>
#include <vector>

#include "examples/poker_types.pb.h"

namespace tests {

/// Helper to parse a game variant string to enum.
inline examples::GameVariant parse_game_variant(const std::string& variant) {
    if (variant == "TEXAS_HOLDEM") return examples::GameVariant::TEXAS_HOLDEM;
    if (variant == "OMAHA") return examples::GameVariant::OMAHA;
    if (variant == "FIVE_CARD_DRAW") return examples::GameVariant::FIVE_CARD_DRAW;
    return examples::GameVariant::GAME_VARIANT_UNSPECIFIED;
}

/// Helper to convert game variant enum to string.
inline std::string variant_to_string(examples::GameVariant variant) {
    switch (variant) {
        case examples::GameVariant::TEXAS_HOLDEM:
            return "TEXAS_HOLDEM";
        case examples::GameVariant::OMAHA:
            return "OMAHA";
        case examples::GameVariant::FIVE_CARD_DRAW:
            return "FIVE_CARD_DRAW";
        default:
            return "UNSPECIFIED";
    }
}

/// Generate player root from player ID string.
inline std::string make_player_root(const std::string& player_id) { return player_id; }

/// Generate table root for tests.
inline std::string make_table_root() { return "test-table"; }

/// Generate hand root from table root and hand number.
inline std::string generate_hand_root(const std::string& table_root, int64_t hand_number) {
    return table_root + "-hand-" + std::to_string(hand_number);
}

/// Get cards per player for a variant.
inline int cards_per_player(examples::GameVariant variant) {
    switch (variant) {
        case examples::GameVariant::TEXAS_HOLDEM:
            return 2;
        case examples::GameVariant::OMAHA:
            return 4;
        case examples::GameVariant::FIVE_CARD_DRAW:
            return 5;
        default:
            return 2;
    }
}

/// Parse action type from string.
inline examples::ActionType parse_action_type(const std::string& action) {
    if (action == "FOLD") return examples::ActionType::FOLD;
    if (action == "CHECK") return examples::ActionType::CHECK;
    if (action == "CALL") return examples::ActionType::CALL;
    if (action == "BET") return examples::ActionType::BET;
    if (action == "RAISE") return examples::ActionType::RAISE;
    if (action == "ALL_IN") return examples::ActionType::ALL_IN;
    return examples::ActionType::ACTION_UNSPECIFIED;
}

/// Parse blind type string - returns lowercase to match apply_event expectations.
inline std::string parse_blind_type(const std::string& blind_type) {
    if (blind_type == "small" || blind_type == "SMALL_BLIND") return "small";
    if (blind_type == "big" || blind_type == "BIG_BLIND") return "big";
    return blind_type;
}

/// Parse betting phase from string.
inline examples::BettingPhase parse_betting_phase(const std::string& phase) {
    std::string lower = phase;
    std::transform(lower.begin(), lower.end(), lower.begin(), ::tolower);
    if (lower == "preflop") return examples::BettingPhase::PREFLOP;
    if (lower == "flop") return examples::BettingPhase::FLOP;
    if (lower == "turn") return examples::BettingPhase::TURN;
    if (lower == "river") return examples::BettingPhase::RIVER;
    if (lower == "showdown") return examples::BettingPhase::SHOWDOWN;
    return examples::BettingPhase::BETTING_PHASE_UNSPECIFIED;
}

/// Parse hand rank type from string.
inline examples::HandRankType parse_hand_rank(const std::string& rank) {
    if (rank == "ROYAL_FLUSH") return examples::HandRankType::ROYAL_FLUSH;
    if (rank == "STRAIGHT_FLUSH") return examples::HandRankType::STRAIGHT_FLUSH;
    if (rank == "FOUR_OF_A_KIND") return examples::HandRankType::FOUR_OF_A_KIND;
    if (rank == "FULL_HOUSE") return examples::HandRankType::FULL_HOUSE;
    if (rank == "FLUSH") return examples::HandRankType::FLUSH;
    if (rank == "STRAIGHT") return examples::HandRankType::STRAIGHT;
    if (rank == "THREE_OF_A_KIND") return examples::HandRankType::THREE_OF_A_KIND;
    if (rank == "TWO_PAIR") return examples::HandRankType::TWO_PAIR;
    if (rank == "PAIR") return examples::HandRankType::PAIR;
    if (rank == "HIGH_CARD") return examples::HandRankType::HIGH_CARD;
    return examples::HandRankType::HAND_RANK_UNSPECIFIED;
}

/// Parse a card string like "As" to a Card.
inline examples::Card parse_card(const std::string& s) {
    examples::Card card;
    if (s.size() < 2) return card;

    std::string rank_str = s.substr(0, s.size() - 1);
    char suit_char = s.back();

    // Parse rank
    if (rank_str == "A")
        card.set_rank(examples::Rank::ACE);
    else if (rank_str == "K")
        card.set_rank(examples::Rank::KING);
    else if (rank_str == "Q")
        card.set_rank(examples::Rank::QUEEN);
    else if (rank_str == "J")
        card.set_rank(examples::Rank::JACK);
    else if (rank_str == "T" || rank_str == "10")
        card.set_rank(examples::Rank::TEN);
    else if (rank_str == "9")
        card.set_rank(examples::Rank::NINE);
    else if (rank_str == "8")
        card.set_rank(examples::Rank::EIGHT);
    else if (rank_str == "7")
        card.set_rank(examples::Rank::SEVEN);
    else if (rank_str == "6")
        card.set_rank(examples::Rank::SIX);
    else if (rank_str == "5")
        card.set_rank(examples::Rank::FIVE);
    else if (rank_str == "4")
        card.set_rank(examples::Rank::FOUR);
    else if (rank_str == "3")
        card.set_rank(examples::Rank::THREE);
    else if (rank_str == "2")
        card.set_rank(examples::Rank::TWO);

    // Parse suit
    switch (suit_char) {
        case 's':
            card.set_suit(examples::Suit::SPADES);
            break;
        case 'h':
            card.set_suit(examples::Suit::HEARTS);
            break;
        case 'd':
            card.set_suit(examples::Suit::DIAMONDS);
            break;
        case 'c':
            card.set_suit(examples::Suit::CLUBS);
            break;
        default:
            break;
    }

    return card;
}

/// Parse cards string like "As Ks" to vector.
inline std::vector<examples::Card> parse_cards(const std::string& s) {
    std::vector<examples::Card> cards;
    std::istringstream iss(s);
    std::string token;
    while (iss >> token) {
        cards.push_back(parse_card(token));
    }
    return cards;
}

/// Evaluate the best hand from hole cards + community cards.
/// Returns the hand rank type.
inline examples::HandRankType evaluate_hand(const std::vector<examples::Card>& hole_cards,
                                            const std::vector<examples::Card>& community_cards) {
    // Combine all cards
    std::vector<examples::Card> all_cards;
    for (const auto& c : hole_cards) all_cards.push_back(c);
    for (const auto& c : community_cards) all_cards.push_back(c);

    // Count ranks and suits
    std::map<int, int> rank_counts;
    std::map<int, int> suit_counts;
    std::map<int, std::vector<examples::Card>> cards_by_suit;

    for (const auto& c : all_cards) {
        rank_counts[c.rank()]++;
        suit_counts[c.suit()]++;
        cards_by_suit[c.suit()].push_back(c);
    }

    // Check for flush (5 cards of same suit)
    bool has_flush = false;
    int flush_suit = 0;
    for (const auto& [suit, count] : suit_counts) {
        if (count >= 5) {
            has_flush = true;
            flush_suit = suit;
            break;
        }
    }

    // Helper to check for straight in a set of ranks
    auto check_straight = [](std::vector<int> ranks) -> int {
        std::sort(ranks.begin(), ranks.end());
        ranks.erase(std::unique(ranks.begin(), ranks.end()), ranks.end());

        // Check for wheel (A-2-3-4-5)
        bool has_ace = std::find(ranks.begin(), ranks.end(), 14) != ranks.end();
        if (has_ace) {
            ranks.push_back(1);  // Ace can be low
            std::sort(ranks.begin(), ranks.end());
        }

        int consecutive = 1;
        int best_high = 0;
        for (size_t i = 1; i < ranks.size(); ++i) {
            if (ranks[i] == ranks[i - 1] + 1) {
                consecutive++;
                if (consecutive >= 5) {
                    best_high = ranks[i];
                }
            } else {
                consecutive = 1;
            }
        }
        return best_high;
    };

    // Get all ranks
    std::vector<int> all_ranks;
    for (const auto& c : all_cards) {
        all_ranks.push_back(c.rank());
    }
    int straight_high = check_straight(all_ranks);
    bool has_straight = (straight_high > 0);

    // Check for straight flush / royal flush
    if (has_flush) {
        std::vector<int> flush_ranks;
        for (const auto& c : cards_by_suit[flush_suit]) {
            flush_ranks.push_back(c.rank());
        }
        int sf_high = check_straight(flush_ranks);
        if (sf_high > 0) {
            if (sf_high == 14) {  // A-high straight flush = royal flush
                return examples::HandRankType::ROYAL_FLUSH;
            }
            return examples::HandRankType::STRAIGHT_FLUSH;
        }
    }

    // Count pairs, trips, quads
    int pairs = 0, trips = 0, quads = 0;
    for (const auto& [rank, count] : rank_counts) {
        if (count == 4) quads++;
        else if (count == 3) trips++;
        else if (count == 2) pairs++;
    }

    if (quads > 0) return examples::HandRankType::FOUR_OF_A_KIND;
    if (trips > 0 && pairs > 0) return examples::HandRankType::FULL_HOUSE;
    if (trips >= 2) return examples::HandRankType::FULL_HOUSE;  // Two trips makes a full house
    if (has_flush) return examples::HandRankType::FLUSH;
    if (has_straight) return examples::HandRankType::STRAIGHT;
    if (trips > 0) return examples::HandRankType::THREE_OF_A_KIND;
    if (pairs >= 2) return examples::HandRankType::TWO_PAIR;
    if (pairs == 1) return examples::HandRankType::PAIR;

    return examples::HandRankType::HIGH_CARD;
}

}  // namespace tests
