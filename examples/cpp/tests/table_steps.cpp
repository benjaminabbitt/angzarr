// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

#include <regex>

#include "angzarr/errors.hpp"
#include "create_handler.hpp"
#include "end_hand_handler.hpp"
#include "examples/table.pb.h"
#include "join_handler.hpp"
#include "leave_handler.hpp"
#include "start_hand_handler.hpp"
#include "table_state.hpp"
#include "test_context.hpp"
#include "test_utils.hpp"

using cucumber::ScenarioScope;
using tests::generate_hand_root;
using tests::make_player_root;
using tests::parse_game_variant;

// ==========================================================================
// Given Steps - Setting up state from events
// ==========================================================================

GIVEN("^no prior events for the table aggregate$") {
    tests::g_context.event_pages.clear();
    tests::g_context.table_state = table::TableState{};
}

GIVEN("^a TableCreated event for \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, table_name);

    examples::TableCreated event;
    event.set_table_name(table_name);
    event.set_game_variant(examples::GameVariant::TEXAS_HOLDEM);
    event.set_small_blind(5);
    event.set_big_blind(10);
    event.set_min_buy_in(200);
    event.set_max_buy_in(2000);
    event.set_max_players(9);
    event.set_action_timeout_seconds(30);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

GIVEN("^a TableCreated event for \"([^\"]*)\" with min_buy_in (\\d+)$") {
    REGEX_PARAM(std::string, table_name);
    REGEX_PARAM(int64_t, min_buy_in);

    examples::TableCreated event;
    event.set_table_name(table_name);
    event.set_game_variant(examples::GameVariant::TEXAS_HOLDEM);
    event.set_small_blind(5);
    event.set_big_blind(10);
    event.set_min_buy_in(min_buy_in);
    event.set_max_buy_in(2000);
    event.set_max_players(9);
    event.set_action_timeout_seconds(30);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

GIVEN("^a TableCreated event for \"([^\"]*)\" with max_players (\\d+)$") {
    REGEX_PARAM(std::string, table_name);
    REGEX_PARAM(int, max_players);

    examples::TableCreated event;
    event.set_table_name(table_name);
    event.set_game_variant(examples::GameVariant::TEXAS_HOLDEM);
    event.set_small_blind(5);
    event.set_big_blind(10);
    event.set_min_buy_in(200);
    event.set_max_buy_in(2000);
    event.set_max_players(max_players);
    event.set_action_timeout_seconds(30);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

GIVEN("^a PlayerJoined event for player \"([^\"]*)\" at seat (\\d+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(int, seat);

    examples::PlayerJoined event;
    event.set_player_root(make_player_root(player_id));
    event.set_seat_position(seat);
    event.set_buy_in_amount(500);
    event.set_stack(500);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

GIVEN("^a PlayerJoined event for player \"([^\"]*)\" at seat (\\d+) with stack (\\d+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(int, seat);
    REGEX_PARAM(int64_t, stack);

    examples::PlayerJoined event;
    event.set_player_root(make_player_root(player_id));
    event.set_seat_position(seat);
    event.set_buy_in_amount(stack);
    event.set_stack(stack);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

GIVEN("^a HandStarted event for hand (\\d+)$") {
    REGEX_PARAM(int64_t, hand_number);

    examples::HandStarted event;
    event.set_hand_root(generate_hand_root("test-table", hand_number));
    event.set_hand_number(hand_number);
    event.set_dealer_position(0);
    event.set_small_blind_position(0);
    event.set_big_blind_position(1);
    event.set_game_variant(examples::GameVariant::TEXAS_HOLDEM);
    event.set_small_blind(5);
    event.set_big_blind(10);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

GIVEN("^a HandStarted event for hand (\\d+) with dealer at seat (\\d+)$") {
    REGEX_PARAM(int64_t, hand_number);
    REGEX_PARAM(int, dealer_seat);

    examples::HandStarted event;
    event.set_hand_root(generate_hand_root("test-table", hand_number));
    event.set_hand_number(hand_number);
    event.set_dealer_position(dealer_seat);
    event.set_small_blind_position((dealer_seat + 1) % 9);
    event.set_big_blind_position((dealer_seat + 2) % 9);
    event.set_game_variant(examples::GameVariant::TEXAS_HOLDEM);
    event.set_small_blind(5);
    event.set_big_blind(10);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

GIVEN("^a HandEnded event for hand (\\d+)$") {
    REGEX_PARAM(int64_t, hand_number);

    examples::HandEnded event;
    event.set_hand_root(generate_hand_root("test-table", hand_number));

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

// ==========================================================================
// When Steps - Handling commands
// ==========================================================================

WHEN("^I rebuild the table state$") { tests::g_context.rebuild_table_state(); }

WHEN("^I handle a CreateTable command with name \"([^\"]*)\" and variant \"([^\"]*)\":$") {
    REGEX_PARAM(std::string, table_name);
    REGEX_PARAM(std::string, variant_str);

    TABLE_PARAM(table);

    // Parse table data
    int64_t small_blind = 5;
    int64_t big_blind = 10;
    int64_t min_buy_in = 200;
    int64_t max_buy_in = 1000;
    int max_players = 9;

    for (auto& row : table.hashes()) {
        if (row.count("small_blind")) small_blind = std::stoll(row.at("small_blind"));
        if (row.count("big_blind")) big_blind = std::stoll(row.at("big_blind"));
        if (row.count("min_buy_in")) min_buy_in = std::stoll(row.at("min_buy_in"));
        if (row.count("max_buy_in")) max_buy_in = std::stoll(row.at("max_buy_in"));
        if (row.count("max_players")) max_players = std::stoi(row.at("max_players"));
    }

    tests::g_context.clear_error();

    examples::CreateTable cmd;
    cmd.set_table_name(table_name);
    cmd.set_game_variant(parse_game_variant(variant_str));
    cmd.set_small_blind(small_blind);
    cmd.set_big_blind(big_blind);
    cmd.set_min_buy_in(min_buy_in);
    cmd.set_max_buy_in(max_buy_in);
    cmd.set_max_players(max_players);
    cmd.set_action_timeout_seconds(30);

    try {
        auto event = table::handlers::handle_create(cmd, tests::g_context.table_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a JoinTable command for player \"([^\"]*)\" at seat (-?\\d+) with buy-in (\\d+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(int, seat);
    REGEX_PARAM(int64_t, buy_in);

    tests::g_context.clear_error();

    examples::JoinTable cmd;
    cmd.set_player_root(make_player_root(player_id));
    cmd.set_preferred_seat(seat);
    cmd.set_buy_in_amount(buy_in);

    try {
        auto event = table::handlers::handle_join(cmd, tests::g_context.table_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a LeaveTable command for player \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_id);

    tests::g_context.clear_error();

    examples::LeaveTable cmd;
    cmd.set_player_root(make_player_root(player_id));

    try {
        auto event = table::handlers::handle_leave(cmd, tests::g_context.table_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a StartHand command$") {
    tests::g_context.clear_error();

    examples::StartHand cmd;

    try {
        auto event = table::handlers::handle_start_hand(cmd, tests::g_context.table_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle an EndHand command with winner \"([^\"]*)\" winning (\\d+)$") {
    REGEX_PARAM(std::string, winner_id);
    REGEX_PARAM(int64_t, amount);

    tests::g_context.clear_error();

    examples::EndHand cmd;
    cmd.set_hand_root(tests::g_context.table_state.current_hand_root);

    auto* result = cmd.add_results();
    result->set_winner_root(make_player_root(winner_id));
    result->set_amount(amount);
    result->set_pot_type("main");

    try {
        auto event = table::handlers::handle_end_hand(cmd, tests::g_context.table_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle an EndHand command with results:$") {
    TABLE_PARAM(table);

    tests::g_context.clear_error();

    examples::EndHand cmd;
    cmd.set_hand_root(tests::g_context.table_state.current_hand_root);

    for (auto& row : table.hashes()) {
        auto* result = cmd.add_results();
        result->set_winner_root(make_player_root(row.at("player")));
        result->set_amount(std::stoll(row.at("change")));
        result->set_pot_type("main");
    }

    try {
        auto event = table::handlers::handle_end_hand(cmd, tests::g_context.table_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

// ==========================================================================
// Then Steps - Assertions on results
// ==========================================================================

THEN("^the result is a examples\\.TableCreated event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::TableCreated>();
    ASSERT_TRUE(event.has_value()) << "Expected TableCreated event";
}

THEN("^the result is a examples\\.PlayerJoined event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::PlayerJoined>();
    ASSERT_TRUE(event.has_value()) << "Expected PlayerJoined event";
}

THEN("^the result is a examples\\.PlayerLeft event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::PlayerLeft>();
    ASSERT_TRUE(event.has_value()) << "Expected PlayerLeft event";
}

THEN("^the result is a examples\\.HandStarted event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::HandStarted>();
    ASSERT_TRUE(event.has_value()) << "Expected HandStarted event";
}

THEN("^the result is a examples\\.HandEnded event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::HandEnded>();
    ASSERT_TRUE(event.has_value()) << "Expected HandEnded event";
}

THEN("^the table event has table_name \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    auto event = tests::g_context.get_result_as<examples::TableCreated>();
    ASSERT_TRUE(event.has_value()) << "Expected TableCreated event";
    ASSERT_EQ(event->table_name(), expected);
}

THEN("^the table event has game_variant \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    auto expected_variant = parse_game_variant(expected);

    // Could be TableCreated or HandStarted
    auto tc = tests::g_context.get_result_as<examples::TableCreated>();
    if (tc.has_value()) {
        ASSERT_EQ(tc->game_variant(), expected_variant);
        return;
    }

    auto hs = tests::g_context.get_result_as<examples::HandStarted>();
    if (hs.has_value()) {
        ASSERT_EQ(hs->game_variant(), expected_variant);
        return;
    }

    FAIL() << "Expected TableCreated or HandStarted event";
}

THEN("^the table event has small_blind (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::TableCreated>();
    ASSERT_TRUE(event.has_value()) << "Expected TableCreated event";
    ASSERT_EQ(event->small_blind(), expected);
}

THEN("^the table event has big_blind (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::TableCreated>();
    ASSERT_TRUE(event.has_value()) << "Expected TableCreated event";
    ASSERT_EQ(event->big_blind(), expected);
}

THEN("^the table event has seat_position (\\d+)$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::PlayerJoined>();
    ASSERT_TRUE(event.has_value()) << "Expected PlayerJoined event";
    ASSERT_EQ(event->seat_position(), expected);
}

THEN("^the table event has buy_in_amount (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::PlayerJoined>();
    ASSERT_TRUE(event.has_value()) << "Expected PlayerJoined event";
    ASSERT_EQ(event->buy_in_amount(), expected);
}

THEN("^the table event has chips_cashed_out (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::PlayerLeft>();
    ASSERT_TRUE(event.has_value()) << "Expected PlayerLeft event";
    ASSERT_EQ(event->chips_cashed_out(), expected);
}

THEN("^the table event has hand_number (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    auto event = tests::g_context.get_result_as<examples::HandStarted>();
    ASSERT_TRUE(event.has_value()) << "Expected HandStarted event";
    ASSERT_EQ(event->hand_number(), expected);
}

THEN("^the table event has (\\d+) active_players$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::HandStarted>();
    ASSERT_TRUE(event.has_value()) << "Expected HandStarted event";
    ASSERT_EQ(event->active_players_size(), expected);
}

THEN("^the table event has dealer_position (\\d+)$") {
    REGEX_PARAM(int, expected);
    auto event = tests::g_context.get_result_as<examples::HandStarted>();
    ASSERT_TRUE(event.has_value()) << "Expected HandStarted event";
    ASSERT_EQ(event->dealer_position(), expected);
}

THEN("^player \"([^\"]*)\" stack change is (-?\\d+)$") {
    REGEX_PARAM(std::string, player_id);
    REGEX_PARAM(int64_t, expected);

    auto event = tests::g_context.get_result_as<examples::HandEnded>();
    ASSERT_TRUE(event.has_value()) << "Expected HandEnded event";

    std::string player_root = make_player_root(player_id);
    auto& stack_changes = event->stack_changes();
    auto it = stack_changes.find(player_root);
    if (it != stack_changes.end()) {
        ASSERT_EQ(it->second, expected) << "Stack change mismatch for " << player_id;
    } else {
        FAIL() << "No stack change found for player " << player_id;
    }
}

// State assertions
THEN("^the table state has (\\d+) players$") {
    REGEX_PARAM(int, expected);
    ASSERT_EQ(tests::g_context.table_state.player_count(), expected);
}

THEN("^the table state has seat (\\d+) occupied by \"([^\"]*)\"$") {
    REGEX_PARAM(int, seat);
    REGEX_PARAM(std::string, player_id);

    auto* seat_state = tests::g_context.table_state.get_seat(seat);
    ASSERT_NE(seat_state, nullptr) << "Seat " << seat << " not found";
    ASSERT_EQ(seat_state->player_root, make_player_root(player_id));
}

THEN("^the table state has status \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    ASSERT_EQ(tests::g_context.table_state.status, expected);
}

THEN("^the table state has hand_count (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    ASSERT_EQ(tests::g_context.table_state.hand_count, expected);
}
