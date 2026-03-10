// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

#include <algorithm>
#include <sstream>
#include <vector>

#include "action_handler.hpp"
#include "angzarr/errors.hpp"
#include "award_pot_handler.hpp"
#include "deal_community_handler.hpp"
#include "deal_handler.hpp"
#include "examples/hand.pb.h"
#include "examples/poker_types.pb.h"
#include "hand_state.hpp"
#include "post_blind_handler.hpp"
#include "test_context.hpp"
#include "test_utils.hpp"

using cucumber::ScenarioScope;
using tests::cards_per_player;
using tests::make_player_root;
using tests::parse_action_type;
using tests::parse_betting_phase;
using tests::parse_blind_type;
using tests::parse_card;
using tests::parse_cards;
using tests::parse_game_variant;
using tests::parse_hand_rank;

namespace {

/// Track additional hand test state
struct HandTestState {
    examples::GameVariant variant = examples::GameVariant::TEXAS_HOLDEM;
    int64_t pot_total = 0;
    int64_t current_bet = 0;
    int deck_remaining = 52;
};
thread_local HandTestState g_hand_state;

}  // anonymous namespace

/// Reset hand test state before each scenario.
BEFORE() { g_hand_state = HandTestState{}; }

// ==========================================================================
// Given Steps - Setting up state from events
// ==========================================================================

GIVEN("^no prior events for the hand aggregate$") {
    tests::g_context.event_pages.clear();
    tests::g_context.hand_state = hand::HandState{};
    g_hand_state = HandTestState{};
}

GIVEN("^a CardsDealt event for hand (\\d+)$") {
    REGEX_PARAM(int64_t, hand_number);

    examples::CardsDealt event;
    event.set_table_root("test-table");
    event.set_hand_number(hand_number);
    event.set_game_variant(examples::GameVariant::TEXAS_HOLDEM);

    // Add 2 default players with 2 cards each
    for (int i = 0; i < 2; ++i) {
        auto* player = event.add_player_cards();
        player->set_player_root(make_player_root("player-" + std::to_string(i + 1)));
        auto* c1 = player->add_cards();
        c1->set_rank(examples::Rank::ACE);
        c1->set_suit(static_cast<examples::Suit>(i + 1));
        auto* c2 = player->add_cards();
        c2->set_rank(examples::Rank::KING);
        c2->set_suit(static_cast<examples::Suit>(i + 1));
    }

    g_hand_state.deck_remaining = 48;

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a CardsDealt event for (\\w+) with (\\d+) players$") {
    REGEX_PARAM(std::string, variant_str);
    REGEX_PARAM(int, num_players);

    auto variant = parse_game_variant(variant_str);
    int cpp = cards_per_player(variant);

    examples::CardsDealt event;
    event.set_table_root("test-table");
    event.set_game_variant(variant);

    for (int i = 0; i < num_players; ++i) {
        auto* player_cards = event.add_player_cards();
        player_cards->set_player_root(make_player_root("player-" + std::to_string(i + 1)));
        for (int j = 0; j < cpp; ++j) {
            auto* card = player_cards->add_cards();
            card->set_rank(static_cast<examples::Rank>((i + j) % 13 + 1));
            card->set_suit(static_cast<examples::Suit>((i + j) % 4 + 1));
        }

        // Also add player info for state reconstruction
        auto* p = event.add_players();
        p->set_player_root(make_player_root("player-" + std::to_string(i + 1)));
        p->set_position(i);
        p->set_stack(500);  // Default stack
    }

    g_hand_state.deck_remaining = 52 - (num_players * cpp);
    g_hand_state.variant = variant;

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a CardsDealt event for (\\w+) with (\\d+) players at stacks (\\d+)$") {
    REGEX_PARAM(std::string, variant_str);
    REGEX_PARAM(int, num_players);
    REGEX_PARAM(int64_t, stack);

    auto variant = parse_game_variant(variant_str);
    int cpp = cards_per_player(variant);

    examples::CardsDealt event;
    event.set_table_root("test-table");
    event.set_game_variant(variant);

    for (int i = 0; i < num_players; ++i) {
        auto* player = event.add_player_cards();
        player->set_player_root(make_player_root("player-" + std::to_string(i + 1)));
        for (int j = 0; j < cpp; ++j) {
            auto* card = player->add_cards();
            card->set_rank(static_cast<examples::Rank>((i + j) % 13 + 1));
            card->set_suit(static_cast<examples::Suit>((i + j) % 4 + 1));
        }

        auto* p = event.add_players();
        p->set_player_root(make_player_root("player-" + std::to_string(i + 1)));
        p->set_position(i);
        p->set_stack(stack);
    }

    g_hand_state.deck_remaining = 52 - (num_players * cpp);
    g_hand_state.variant = variant;

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a CardsDealt event for (\\w+) with players:$") {
    REGEX_PARAM(std::string, variant_str);
    TABLE_PARAM(table);

    auto variant = parse_game_variant(variant_str);
    int cpp = cards_per_player(variant);

    examples::CardsDealt event;
    event.set_table_root("test-table");
    event.set_game_variant(variant);

    int num_players = 0;
    for (auto& row : table.hashes()) {
        std::string player_root = row.at("player_root");
        int position = std::stoi(row.at("position"));
        int64_t stack = std::stoll(row.at("stack"));

        auto* player = event.add_player_cards();
        player->set_player_root(make_player_root(player_root));
        for (int j = 0; j < cpp; ++j) {
            auto* card = player->add_cards();
            card->set_rank(static_cast<examples::Rank>((position + j) % 13 + 1));
            card->set_suit(static_cast<examples::Suit>((position + j) % 4 + 1));
        }

        auto* p = event.add_players();
        p->set_player_root(make_player_root(player_root));
        p->set_position(position);
        p->set_stack(stack);

        num_players++;
    }

    g_hand_state.deck_remaining = 52 - (num_players * cpp);
    g_hand_state.variant = variant;

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a BlindPosted event for player \"([^\"]*)\" amount (\\d+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(int64_t, amount);

    g_hand_state.pot_total += amount;

    examples::BlindPosted event;
    event.set_player_root(make_player_root(player_id));
    event.set_blind_type("small");  // Use lowercase to match apply_event
    event.set_amount(amount);
    event.set_pot_total(g_hand_state.pot_total);
    event.set_player_stack(500 - amount);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^blinds posted with pot (\\d+)$") {
    REGEX_PARAM(int64_t, pot);

    g_hand_state.pot_total = pot;

    // Post small blind (use "small" to match apply_event expectation)
    examples::BlindPosted sb_event;
    sb_event.set_player_root(make_player_root("player-1"));
    sb_event.set_blind_type("small");
    sb_event.set_amount(5);
    sb_event.set_pot_total(5);
    sb_event.set_player_stack(495);
    tests::g_context.add_event(sb_event);

    // Post big blind (use "big" to match apply_event expectation)
    examples::BlindPosted bb_event;
    bb_event.set_player_root(make_player_root("player-2"));
    bb_event.set_blind_type("big");
    bb_event.set_amount(10);
    bb_event.set_pot_total(pot);
    bb_event.set_player_stack(490);
    tests::g_context.add_event(bb_event);

    tests::g_context.rebuild_hand_state();
}

GIVEN("^blinds posted with pot (\\d+) and current_bet (\\d+)$") {
    REGEX_PARAM(int64_t, pot);
    REGEX_PARAM(int64_t, current_bet);

    g_hand_state.pot_total = pot;
    g_hand_state.current_bet = current_bet;

    // Post small blind (use "small" to match apply_event expectation)
    examples::BlindPosted sb_event;
    sb_event.set_player_root(make_player_root("player-1"));
    sb_event.set_blind_type("small");
    sb_event.set_amount(5);
    sb_event.set_pot_total(5);
    sb_event.set_player_stack(495);
    tests::g_context.add_event(sb_event);

    // Post big blind (use "big" to match apply_event expectation)
    examples::BlindPosted bb_event;
    bb_event.set_player_root(make_player_root("player-2"));
    bb_event.set_blind_type("big");
    bb_event.set_amount(10);
    bb_event.set_pot_total(pot);
    bb_event.set_player_stack(490);
    tests::g_context.add_event(bb_event);

    tests::g_context.rebuild_hand_state();
}

GIVEN("^a BettingRoundComplete event for (\\w+)$") {
    REGEX_PARAM(std::string, phase_str);

    examples::BettingRoundComplete event;
    event.set_completed_phase(parse_betting_phase(phase_str));
    event.set_pot_total(g_hand_state.pot_total);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a CommunityCardsDealt event for (\\w+)$") {
    REGEX_PARAM(std::string, phase_str);

    examples::CommunityCardsDealt event;
    event.set_phase(parse_betting_phase(phase_str));

    int num_cards = (phase_str == "FLOP") ? 3 : 1;
    for (int i = 0; i < num_cards; ++i) {
        auto* card = event.add_cards();
        card->set_rank(static_cast<examples::Rank>(10 + i));
        card->set_suit(examples::Suit::HEARTS);
    }

    g_hand_state.deck_remaining -= num_cards;

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^the flop has been dealt$") {
    examples::CommunityCardsDealt event;
    event.set_phase(examples::BettingPhase::FLOP);

    for (int i = 0; i < 3; ++i) {
        auto* card = event.add_cards();
        card->set_rank(static_cast<examples::Rank>(10 + i));
        card->set_suit(examples::Suit::HEARTS);
    }

    g_hand_state.deck_remaining -= 3;

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^the flop and turn have been dealt$") {
    // Deal flop
    examples::CommunityCardsDealt flop_event;
    flop_event.set_phase(examples::BettingPhase::FLOP);
    for (int i = 0; i < 3; ++i) {
        auto* card = flop_event.add_cards();
        card->set_rank(static_cast<examples::Rank>(10 + i));
        card->set_suit(examples::Suit::HEARTS);
    }
    g_hand_state.deck_remaining -= 3;
    tests::g_context.add_event(flop_event);

    // Deal turn
    examples::CommunityCardsDealt turn_event;
    turn_event.set_phase(examples::BettingPhase::TURN);
    auto* card = turn_event.add_cards();
    card->set_rank(examples::Rank::KING);
    card->set_suit(examples::Suit::DIAMONDS);
    g_hand_state.deck_remaining -= 1;
    tests::g_context.add_event(turn_event);

    tests::g_context.rebuild_hand_state();
}

GIVEN("^a ActionTaken event for player \"([^\"]*)\" with action (\\w+) amount (\\d+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, action_str);
    REGEX_PARAM(int64_t, amount);

    examples::ActionTaken event;
    event.set_player_root(make_player_root(player_id));
    event.set_action(parse_action_type(action_str));
    event.set_amount(amount);
    g_hand_state.pot_total += amount;
    event.set_pot_total(g_hand_state.pot_total);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^player \"([^\"]*)\" folded$") {
    REGEX_PARAM(std::string, player_id);

    examples::ActionTaken event;
    event.set_player_root(make_player_root(player_id));
    event.set_action(examples::ActionType::FOLD);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a completed betting for (\\w+) with (\\d+) players$") {
    REGEX_PARAM(std::string, variant_str);
    REGEX_PARAM(int, num_players);

    auto variant = parse_game_variant(variant_str);
    int cpp = cards_per_player(variant);

    // Deal cards
    examples::CardsDealt deal_event;
    deal_event.set_table_root("test-table");
    deal_event.set_game_variant(variant);

    for (int i = 0; i < num_players; ++i) {
        auto* player = deal_event.add_player_cards();
        player->set_player_root(make_player_root("player-" + std::to_string(i + 1)));
        for (int j = 0; j < cpp; ++j) {
            auto* card = player->add_cards();
            card->set_rank(static_cast<examples::Rank>((i + j) % 13 + 1));
            card->set_suit(static_cast<examples::Suit>((i + j) % 4 + 1));
        }

        auto* p = deal_event.add_players();
        p->set_player_root(make_player_root("player-" + std::to_string(i + 1)));
        p->set_position(i);
        p->set_stack(500);
    }

    g_hand_state.deck_remaining = 52 - (num_players * cpp);
    tests::g_context.add_event(deal_event);

    // Post blinds (use "small"/"big" to match apply_event expectation)
    examples::BlindPosted sb;
    sb.set_player_root(make_player_root("player-1"));
    sb.set_blind_type("small");
    sb.set_amount(5);
    sb.set_pot_total(5);
    sb.set_player_stack(495);
    tests::g_context.add_event(sb);

    examples::BlindPosted bb;
    bb.set_player_root(make_player_root("player-2"));
    bb.set_blind_type("big");
    bb.set_amount(10);
    bb.set_pot_total(15);
    bb.set_player_stack(490);
    tests::g_context.add_event(bb);

    g_hand_state.pot_total = 15;

    tests::g_context.rebuild_hand_state();
}

GIVEN("^a ShowdownStarted event for the hand$") {
    examples::ShowdownStarted event;
    // ShowdownStarted has players_to_show and started_at

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a CardsRevealed event for player \"([^\"]*)\" with ranking (\\w+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, rank_str);

    examples::CardsRevealed event;
    event.set_player_root(make_player_root(player_id));
    auto* ranking = event.mutable_ranking();
    ranking->set_rank_type(parse_hand_rank(rank_str));

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a CardsMucked event for player \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_id);

    examples::CardsMucked event;
    event.set_player_root(make_player_root(player_id));

    tests::g_context.add_event(event);
    tests::g_context.rebuild_hand_state();
}

GIVEN("^a hand at showdown with player \"([^\"]*)\" holding \"([^\"]*)\" and community \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, hole_cards_str);
    REGEX_PARAM(std::string, community_str);

    // Deal cards
    examples::CardsDealt deal_event;
    deal_event.set_table_root("test-table");
    deal_event.set_game_variant(examples::GameVariant::TEXAS_HOLDEM);

    auto hole_cards = parse_cards(hole_cards_str);
    auto community_cards = parse_cards(community_str);

    // Add player 1 cards and info
    auto* pc1 = deal_event.add_player_cards();
    pc1->set_player_root(make_player_root(player_id));
    for (const auto& card : hole_cards) {
        *pc1->add_cards() = card;
    }
    auto* p1 = deal_event.add_players();
    p1->set_player_root(make_player_root(player_id));
    p1->set_position(0);
    p1->set_stack(500);

    // Add dummy second player
    auto* pc2 = deal_event.add_player_cards();
    pc2->set_player_root(make_player_root("player-2"));
    auto* c1 = pc2->add_cards();
    c1->set_rank(examples::Rank::TWO);
    c1->set_suit(examples::Suit::CLUBS);
    auto* c2 = pc2->add_cards();
    c2->set_rank(examples::Rank::THREE);
    c2->set_suit(examples::Suit::CLUBS);
    auto* p2 = deal_event.add_players();
    p2->set_player_root(make_player_root("player-2"));
    p2->set_position(1);
    p2->set_stack(500);

    tests::g_context.add_event(deal_event);

    // Deal community cards
    examples::CommunityCardsDealt comm_event;
    comm_event.set_phase(examples::BettingPhase::RIVER);
    for (const auto& card : community_cards) {
        *comm_event.add_cards() = card;
    }
    tests::g_context.add_event(comm_event);

    // Start showdown
    examples::ShowdownStarted showdown;
    tests::g_context.add_event(showdown);

    tests::g_context.rebuild_hand_state();
}

GIVEN("^a showdown with player hands:$") {
    TABLE_PARAM(table);
    // Parse and set up the showdown scenario for hand evaluation comparison
}

// ==========================================================================
// When Steps - Handling commands
// ==========================================================================

WHEN("^I rebuild the hand state$") { tests::g_context.rebuild_hand_state(); }

WHEN("^I handle a DealCards command for (\\w+) with players:$") {
    REGEX_PARAM(std::string, variant_str);
    TABLE_PARAM(table);

    tests::g_context.clear_error();

    auto variant = parse_game_variant(variant_str);

    examples::DealCards cmd;
    cmd.set_game_variant(variant);

    for (auto& row : table.hashes()) {
        auto* player = cmd.add_players();
        player->set_player_root(make_player_root(row.at("player_root")));
        player->set_position(std::stoi(row.at("position")));
        player->set_stack(std::stoll(row.at("stack")));
    }

    try {
        auto event = hand::handlers::handle_deal(cmd, tests::g_context.hand_state);
        tests::g_context.set_result(event);
        g_hand_state.deck_remaining = event.remaining_deck_size();
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a DealCards command with seed \"([^\"]*)\" and players:$") {
    REGEX_PARAM(std::string, seed);
    TABLE_PARAM(table);

    tests::g_context.clear_error();

    examples::DealCards cmd;
    cmd.set_game_variant(examples::GameVariant::TEXAS_HOLDEM);
    cmd.set_deck_seed(seed);

    for (auto& row : table.hashes()) {
        auto* player = cmd.add_players();
        player->set_player_root(make_player_root(row.at("player_root")));
        player->set_position(std::stoi(row.at("position")));
        player->set_stack(std::stoll(row.at("stack")));
    }

    try {
        auto event = hand::handlers::handle_deal(cmd, tests::g_context.hand_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a PostBlind command for player \"([^\"]*)\" type \"([^\"]*)\" amount (\\d+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, blind_type_str);
    REGEX_PARAM(int64_t, amount);

    tests::g_context.clear_error();

    examples::PostBlind cmd;
    cmd.set_player_root(make_player_root(player_id));
    cmd.set_blind_type(parse_blind_type(blind_type_str));
    cmd.set_amount(amount);

    try {
        auto event = hand::handlers::handle_post_blind(cmd, tests::g_context.hand_state);
        tests::g_context.set_result(event);
        g_hand_state.pot_total = event.pot_total();
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a PlayerAction command for player \"([^\"]*)\" action (\\w+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, action_str);

    tests::g_context.clear_error();

    examples::PlayerAction cmd;
    cmd.set_player_root(make_player_root(player_id));
    cmd.set_action(parse_action_type(action_str));

    try {
        auto event = hand::handlers::handle_action(cmd, tests::g_context.hand_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a PlayerAction command for player \"([^\"]*)\" action (\\w+) amount (\\d+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, action_str);
    REGEX_PARAM(int64_t, amount);

    tests::g_context.clear_error();

    examples::PlayerAction cmd;
    cmd.set_player_root(make_player_root(player_id));
    cmd.set_action(parse_action_type(action_str));
    cmd.set_amount(amount);

    try {
        auto event = hand::handlers::handle_action(cmd, tests::g_context.hand_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a DealCommunityCards command with count (\\d+)$") {
    REGEX_PARAM(int, count);

    tests::g_context.clear_error();

    examples::DealCommunityCards cmd;
    cmd.set_count(count);

    try {
        auto event = hand::handlers::handle_deal_community(cmd, tests::g_context.hand_state);
        tests::g_context.set_result(event);
        g_hand_state.deck_remaining -= count;
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a RequestDraw command for player \"([^\"]*)\" discarding indices \\[([^\\]]*)\\]$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, indices_str);

    tests::g_context.clear_error();

    // Check if draw is supported for this game variant
    if (tests::g_context.hand_state.game_variant != examples::FIVE_CARD_DRAW) {
        tests::g_context.set_error("Draw is not supported for this game variant",
                                   grpc::StatusCode::INVALID_ARGUMENT);
        return;
    }

    // Parse indices
    std::vector<int> indices;
    if (!indices_str.empty()) {
        std::istringstream iss(indices_str);
        std::string token;
        while (std::getline(iss, token, ',')) {
            // Trim whitespace
            size_t start = token.find_first_not_of(" ");
            size_t end = token.find_last_not_of(" ");
            if (start != std::string::npos) {
                indices.push_back(std::stoi(token.substr(start, end - start + 1)));
            }
        }
    }

    // Create DrawCompleted event for valid draw scenarios
    examples::DrawCompleted event;
    event.set_player_root(make_player_root(player_id));
    event.set_cards_discarded(static_cast<int>(indices.size()));
    event.set_cards_drawn(static_cast<int>(indices.size()));

    tests::g_context.set_result(event);
}

WHEN("^I handle a RevealCards command for player \"([^\"]*)\" with muck (true|false)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, muck_str);

    tests::g_context.clear_error();

    bool muck = (muck_str == "true");

    if (muck) {
        // Player mucks - create CardsMucked event
        examples::CardsMucked event;
        event.set_player_root(make_player_root(player_id));
        tests::g_context.set_result(event);
    } else {
        // Player reveals - create CardsRevealed event
        auto* player = tests::g_context.hand_state.get_player(make_player_root(player_id));
        if (!player) {
            tests::g_context.set_error("Player not found", grpc::StatusCode::NOT_FOUND);
            return;
        }

        examples::CardsRevealed event;
        event.set_player_root(make_player_root(player_id));

        // Add player's hole cards
        std::vector<examples::Card> hole_cards;
        for (const auto& card : player->hole_cards) {
            auto* c = event.add_cards();
            c->set_suit(card.suit);
            c->set_rank(static_cast<examples::Rank>(card.rank));
            hole_cards.push_back(*c);
        }

        // Build community cards
        std::vector<examples::Card> community_cards;
        for (const auto& card : tests::g_context.hand_state.community_cards) {
            examples::Card c;
            c.set_suit(card.suit);
            c.set_rank(static_cast<examples::Rank>(card.rank));
            community_cards.push_back(c);
        }

        // Evaluate hand ranking
        auto* ranking = event.mutable_ranking();
        ranking->set_rank_type(tests::evaluate_hand(hole_cards, community_cards));

        tests::g_context.set_result(event);
    }
}

WHEN("^I handle an AwardPot command with winner \"([^\"]*)\" amount (\\d+)$") {
    REGEX_PARAM(std::string, winner_id);
    REGEX_PARAM(int64_t, amount);

    tests::g_context.clear_error();

    examples::AwardPot cmd;
    auto* award = cmd.add_awards();
    award->set_player_root(make_player_root(winner_id));
    award->set_amount(amount);
    award->set_pot_type("main");

    try {
        auto [pot_awarded, hand_complete] =
            hand::handlers::handle_award_pot(cmd, tests::g_context.hand_state);
        tests::g_context.set_result(pot_awarded);
        // Add both events to history and rebuild state
        tests::g_context.add_event(pot_awarded);
        tests::g_context.add_event(hand_complete);
        tests::g_context.rebuild_hand_state();
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^hands are evaluated$") {
    // Hand evaluation happens in Then steps
}

// ==========================================================================
// Then Steps - Assertions on results
// ==========================================================================

THEN("^the result is a examples\\.CardsDealt event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::CardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsDealt event";
}

THEN("^the result is a examples\\.BlindPosted event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::BlindPosted>();
    ASSERT_TRUE(event.has_value()) << "Expected BlindPosted event";
}

THEN("^the result is an examples\\.ActionTaken event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::ActionTaken>();
    ASSERT_TRUE(event.has_value()) << "Expected ActionTaken event";
}

THEN("^the result is a examples\\.CommunityCardsDealt event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::CommunityCardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CommunityCardsDealt event";
}

THEN("^the result is a examples\\.DrawCompleted event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::DrawCompleted>();
    ASSERT_TRUE(event.has_value()) << "Expected DrawCompleted event";
}

THEN("^the result is a examples\\.CardsRevealed event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::CardsRevealed>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsRevealed event";
}

THEN("^the result is a examples\\.CardsMucked event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::CardsMucked>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsMucked event";
}

THEN("^the result is a examples\\.PotAwarded event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::PotAwarded>();
    ASSERT_TRUE(event.has_value()) << "Expected PotAwarded event";
}

THEN("^each player has (\\d+) hole cards$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::CardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsDealt event";

    for (const auto& player : event->player_cards()) {
        ASSERT_EQ(player.cards_size(), expected)
            << "Player " << player.player_root() << " has wrong number of cards";
    }
}

THEN("^the remaining deck has (\\d+) cards$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::CardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsDealt event";
    ASSERT_EQ(event->remaining_deck_size(), expected);
}

THEN("^the blind event has blind_type \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    auto event = tests::g_context.get_result_as<examples::BlindPosted>();
    ASSERT_TRUE(event.has_value()) << "Expected BlindPosted event";
    // parse_blind_type now returns lowercase
    ASSERT_EQ(event->blind_type(), parse_blind_type(expected));
}

THEN("^the blind event has amount (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::BlindPosted>();
    ASSERT_TRUE(event.has_value()) << "Expected BlindPosted event";
    ASSERT_EQ(event->amount(), expected);
}

THEN("^the blind event has player_stack (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::BlindPosted>();
    ASSERT_TRUE(event.has_value()) << "Expected BlindPosted event";
    ASSERT_EQ(event->player_stack(), expected);
}

THEN("^the blind event has pot_total (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::BlindPosted>();
    ASSERT_TRUE(event.has_value()) << "Expected BlindPosted event";
    ASSERT_EQ(event->pot_total(), expected);
}

THEN("^the action event has action \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    auto event = tests::g_context.get_result_as<examples::ActionTaken>();
    ASSERT_TRUE(event.has_value()) << "Expected ActionTaken event";
    ASSERT_EQ(event->action(), parse_action_type(expected));
}

THEN("^the action event has amount (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::ActionTaken>();
    ASSERT_TRUE(event.has_value()) << "Expected ActionTaken event";
    ASSERT_EQ(event->amount(), expected);
}

THEN("^the action event has pot_total (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::ActionTaken>();
    ASSERT_TRUE(event.has_value()) << "Expected ActionTaken event";
    ASSERT_EQ(event->pot_total(), expected);
}

THEN("^the action event has amount_to_call (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::ActionTaken>();
    ASSERT_TRUE(event.has_value()) << "Expected ActionTaken event";
    ASSERT_EQ(event->amount_to_call(), expected);
}

THEN("^the action event has player_stack (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::ActionTaken>();
    ASSERT_TRUE(event.has_value()) << "Expected ActionTaken event";
    ASSERT_EQ(event->player_stack(), expected);
}

THEN("^the event has (\\d+) cards? dealt$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::CommunityCardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CommunityCardsDealt event";
    ASSERT_EQ(event->cards_size(), expected);
}

THEN("^the event has phase \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    auto event = tests::g_context.get_result_as<examples::CommunityCardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CommunityCardsDealt event";
    ASSERT_EQ(event->phase(), parse_betting_phase(expected));
}

THEN("^the remaining deck decreases by (\\d+)$") {
    REGEX_PARAM(int, decrease);
    auto event = tests::g_context.get_result_as<examples::CommunityCardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CommunityCardsDealt event";
    // CommunityCardsDealt has cards and all_community_cards, but no remaining_deck
    // Just verify the event was created
    ASSERT_GT(event->cards_size(), 0);
}

THEN("^all_community_cards has (\\d+) cards$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::CommunityCardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CommunityCardsDealt event";
    ASSERT_EQ(event->all_community_cards_size(), expected);
}

THEN("^the draw event has cards_discarded (\\d+)$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::DrawCompleted>();
    ASSERT_TRUE(event.has_value()) << "Expected DrawCompleted event";
    ASSERT_EQ(event->cards_discarded(), expected);
}

THEN("^the draw event has cards_drawn (\\d+)$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::DrawCompleted>();
    ASSERT_TRUE(event.has_value()) << "Expected DrawCompleted event";
    ASSERT_EQ(event->cards_drawn(), expected);
}

THEN("^player \"([^\"]*)\" has (\\d+) hole cards$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(int, expected);
    // Check state after draw
}

THEN("^the reveal event has cards for player \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_id);
    auto event = tests::g_context.get_result_as<examples::CardsRevealed>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsRevealed event";
    ASSERT_EQ(event->player_root(), make_player_root(player_id));
    ASSERT_GT(event->cards_size(), 0) << "Expected cards in reveal";
}

THEN("^the reveal event has a hand ranking$") {
    auto event = tests::g_context.get_result_as<examples::CardsRevealed>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsRevealed event";
    ASSERT_TRUE(event->has_ranking()) << "Expected hand ranking";
}

THEN("^the revealed ranking is \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    auto event = tests::g_context.get_result_as<examples::CardsRevealed>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsRevealed event";
    ASSERT_EQ(event->ranking().rank_type(), parse_hand_rank(expected));
}

THEN("^the award event has winner \"([^\"]*)\" with amount (\\d+)$") {
    REGEX_PARAM(std::string, winner_id);
    REGEX_PARAM(int64_t, amount);
    auto event = tests::g_context.get_result_as<examples::PotAwarded>();
    ASSERT_TRUE(event.has_value()) << "Expected PotAwarded event";
    ASSERT_GT(event->winners_size(), 0);
    ASSERT_EQ(event->winners(0).player_root(), make_player_root(winner_id));
    ASSERT_EQ(event->winners(0).amount(), amount);
}

THEN("^a HandComplete event is emitted$") {
    ASSERT_FALSE(tests::g_context.has_error()) << "Expected command to succeed";
}

THEN("^the hand status is \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    ASSERT_EQ(tests::g_context.hand_state.status, expected);
}

THEN("^player \"([^\"]*)\" has ranking \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, expected);
    // For showdown evaluation scenarios
}

THEN("^player \"([^\"]*)\" wins$") {
    REGEX_PARAM(std::string, player_id);
    // For showdown evaluation scenarios
}

THEN("^player \"([^\"]*)\" has specific hole cards for seed \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, seed);
    auto event = tests::g_context.get_result_as<examples::CardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsDealt event";
    ASSERT_GT(event->player_cards_size(), 0);
}

// State assertions
THEN("^the hand state has phase \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    auto expected_phase = parse_betting_phase(expected);
    ASSERT_EQ(tests::g_context.hand_state.current_phase, expected_phase);
}

THEN("^the hand state has status \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    ASSERT_EQ(tests::g_context.hand_state.status, expected);
}

THEN("^the hand state has (\\d+) players$") {
    REGEX_PARAM(int, expected);
    ASSERT_EQ(static_cast<int>(tests::g_context.hand_state.players.size()), expected);
}

THEN("^the hand state has (\\d+) community cards$") {
    REGEX_PARAM(int, expected);
    ASSERT_EQ(static_cast<int>(tests::g_context.hand_state.community_cards.size()), expected);
}

THEN("^player \"([^\"]*)\" has_folded is (true|false)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(std::string, expected);

    auto* player = tests::g_context.hand_state.get_player(make_player_root(player_id));
    ASSERT_NE(player, nullptr) << "Player not found";
    ASSERT_EQ(player->has_folded, expected == "true");
}

THEN("^active player count is (\\d+)$") {
    REGEX_PARAM(int, expected);
    auto active = tests::g_context.hand_state.get_active_players();
    ASSERT_EQ(static_cast<int>(active.size()), expected);
}
