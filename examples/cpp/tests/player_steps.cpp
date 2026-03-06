// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

#include "angzarr/errors.hpp"
#include "deposit_handler.hpp"
#include "examples/player.pb.h"
#include "examples/poker_types.pb.h"
#include "player_state.hpp"
#include "register_handler.hpp"
#include "release_handler.hpp"
#include "reserve_handler.hpp"
#include "test_context.hpp"
#include "withdraw_handler.hpp"

using cucumber::ScenarioScope;

namespace {

/// Helper to create a table root ID from a string.
std::string make_table_root(const std::string& table_id) {
    // In production, this would be a proper UUID. For tests, use the string as-is.
    return table_id;
}

}  // anonymous namespace

// ==========================================================================
// Given Steps - Setting up state from events
// ==========================================================================

GIVEN("^no prior events for the player aggregate$") {
    tests::g_context.event_pages.clear();
    tests::g_context.player_state = player::PlayerState{};
}

GIVEN("^a PlayerRegistered event for \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, name);

    examples::PlayerRegistered event;
    event.set_display_name(name);
    event.set_email(name + "@example.com");
    event.set_player_type(examples::PlayerType::HUMAN);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_player_state();
}

GIVEN("^a FundsDeposited event with amount (\\d+)$") {
    REGEX_PARAM(int64_t, amount);

    // Calculate new balance based on current state
    int64_t new_balance = tests::g_context.player_state.bankroll + amount;

    examples::FundsDeposited event;
    event.mutable_amount()->set_amount(amount);
    event.mutable_amount()->set_currency_code("CHIPS");
    event.mutable_new_balance()->set_amount(new_balance);
    event.mutable_new_balance()->set_currency_code("CHIPS");

    tests::g_context.add_event(event);
    tests::g_context.rebuild_player_state();
}

GIVEN("^a FundsReserved event with amount (\\d+) for table \"([^\"]*)\"$") {
    REGEX_PARAM(int64_t, amount);
    REGEX_PARAM(std::string, table_id);

    int64_t new_reserved = tests::g_context.player_state.reserved_funds + amount;

    examples::FundsReserved event;
    event.set_table_root(make_table_root(table_id));
    event.mutable_amount()->set_amount(amount);
    event.mutable_amount()->set_currency_code("CHIPS");
    event.mutable_new_reserved_balance()->set_amount(new_reserved);
    event.mutable_new_reserved_balance()->set_currency_code("CHIPS");
    event.mutable_new_available_balance()->set_amount(
        tests::g_context.player_state.bankroll - new_reserved);
    event.mutable_new_available_balance()->set_currency_code("CHIPS");

    tests::g_context.add_event(event);
    tests::g_context.rebuild_player_state();
}

// ==========================================================================
// When Steps - Handling commands
// ==========================================================================

WHEN("^I handle a RegisterPlayer command with name \"([^\"]*)\" and email \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, name);
    REGEX_PARAM(std::string, email);

    tests::g_context.clear_error();

    examples::RegisterPlayer cmd;
    cmd.set_display_name(name);
    cmd.set_email(email);
    cmd.set_player_type(examples::PlayerType::HUMAN);

    try {
        auto event = player::handlers::handle_register(cmd, tests::g_context.player_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a RegisterPlayer command with name \"([^\"]*)\" and email \"([^\"]*)\" as AI$") {
    REGEX_PARAM(std::string, name);
    REGEX_PARAM(std::string, email);

    tests::g_context.clear_error();

    examples::RegisterPlayer cmd;
    cmd.set_display_name(name);
    cmd.set_email(email);
    cmd.set_player_type(examples::PlayerType::AI);
    cmd.set_ai_model_id("gpt-4");

    try {
        auto event = player::handlers::handle_register(cmd, tests::g_context.player_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a DepositFunds command with amount (\\d+)$") {
    REGEX_PARAM(int64_t, amount);

    tests::g_context.clear_error();

    examples::DepositFunds cmd;
    cmd.mutable_amount()->set_amount(amount);
    cmd.mutable_amount()->set_currency_code("CHIPS");

    try {
        auto event = player::handlers::handle_deposit(cmd, tests::g_context.player_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a WithdrawFunds command with amount (\\d+)$") {
    REGEX_PARAM(int64_t, amount);

    tests::g_context.clear_error();

    examples::WithdrawFunds cmd;
    cmd.mutable_amount()->set_amount(amount);
    cmd.mutable_amount()->set_currency_code("CHIPS");

    try {
        auto event = player::handlers::handle_withdraw(cmd, tests::g_context.player_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a ReserveFunds command with amount (\\d+) for table \"([^\"]*)\"$") {
    REGEX_PARAM(int64_t, amount);
    REGEX_PARAM(std::string, table_id);

    tests::g_context.clear_error();

    examples::ReserveFunds cmd;
    cmd.set_table_root(make_table_root(table_id));
    cmd.mutable_amount()->set_amount(amount);
    cmd.mutable_amount()->set_currency_code("CHIPS");

    try {
        auto event = player::handlers::handle_reserve(cmd, tests::g_context.player_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I handle a ReleaseFunds command for table \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, table_id);

    tests::g_context.clear_error();

    examples::ReleaseFunds cmd;
    cmd.set_table_root(make_table_root(table_id));

    try {
        auto event = player::handlers::handle_release(cmd, tests::g_context.player_state);
        tests::g_context.set_result(event);
    } catch (const angzarr::CommandRejectedError& e) {
        tests::g_context.set_error(e.what(), e.status_code);
    }
}

WHEN("^I rebuild the player state$") { tests::g_context.rebuild_player_state(); }

// ==========================================================================
// Then Steps - Assertions on results
// ==========================================================================

THEN("^the result is a examples\\.PlayerRegistered event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::PlayerRegistered>();
    ASSERT_TRUE(event.has_value()) << "Expected PlayerRegistered event";
}

THEN("^the result is a examples\\.FundsDeposited event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::FundsDeposited>();
    ASSERT_TRUE(event.has_value()) << "Expected FundsDeposited event";
}

THEN("^the result is a examples\\.FundsWithdrawn event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::FundsWithdrawn>();
    ASSERT_TRUE(event.has_value()) << "Expected FundsWithdrawn event";
}

THEN("^the result is a examples\\.FundsReserved event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::FundsReserved>();
    ASSERT_TRUE(event.has_value()) << "Expected FundsReserved event";
}

THEN("^the result is a examples\\.FundsReleased event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::FundsReleased>();
    ASSERT_TRUE(event.has_value()) << "Expected FundsReleased event";
}

THEN("^the player event has display_name \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected_name);

    auto event = tests::g_context.get_result_as<examples::PlayerRegistered>();
    ASSERT_TRUE(event.has_value()) << "Expected PlayerRegistered event";
    ASSERT_EQ(event->display_name(), expected_name);
}

THEN("^the player event has player_type \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected_type);

    auto event = tests::g_context.get_result_as<examples::PlayerRegistered>();
    ASSERT_TRUE(event.has_value()) << "Expected PlayerRegistered event";

    examples::PlayerType expected;
    if (expected_type == "HUMAN") {
        expected = examples::PlayerType::HUMAN;
    } else if (expected_type == "AI") {
        expected = examples::PlayerType::AI;
    } else {
        expected = examples::PlayerType::PLAYER_TYPE_UNSPECIFIED;
    }
    ASSERT_EQ(event->player_type(), expected);
}

THEN("^the player event has amount (\\d+)$") {
    REGEX_PARAM(int64_t, expected_amount);

    // This could be any event with an amount field - check each type
    if (auto event = tests::g_context.get_result_as<examples::FundsDeposited>()) {
        ASSERT_EQ(event->amount().amount(), expected_amount);
    } else if (auto event = tests::g_context.get_result_as<examples::FundsWithdrawn>()) {
        ASSERT_EQ(event->amount().amount(), expected_amount);
    } else if (auto event = tests::g_context.get_result_as<examples::FundsReserved>()) {
        ASSERT_EQ(event->amount().amount(), expected_amount);
    } else if (auto event = tests::g_context.get_result_as<examples::FundsReleased>()) {
        ASSERT_EQ(event->amount().amount(), expected_amount);
    } else {
        FAIL() << "No event with 'amount' field found";
    }
}

THEN("^the player event has new_balance (\\d+)$") {
    REGEX_PARAM(int64_t, expected_balance);

    if (auto event = tests::g_context.get_result_as<examples::FundsDeposited>()) {
        ASSERT_EQ(event->new_balance().amount(), expected_balance);
    } else if (auto event = tests::g_context.get_result_as<examples::FundsWithdrawn>()) {
        ASSERT_EQ(event->new_balance().amount(), expected_balance);
    } else {
        FAIL() << "No event with 'new_balance' field found";
    }
}

THEN("^the player event has new_available_balance (\\d+)$") {
    REGEX_PARAM(int64_t, expected_balance);

    if (auto event = tests::g_context.get_result_as<examples::FundsReserved>()) {
        ASSERT_EQ(event->new_available_balance().amount(), expected_balance);
    } else if (auto event = tests::g_context.get_result_as<examples::FundsReleased>()) {
        ASSERT_EQ(event->new_available_balance().amount(), expected_balance);
    } else {
        FAIL() << "No event with 'new_available_balance' field found";
    }
}

// State assertions
THEN("^the player state has bankroll (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    ASSERT_EQ(tests::g_context.player_state.bankroll, expected);
}

THEN("^the player state has reserved_funds (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    ASSERT_EQ(tests::g_context.player_state.reserved_funds, expected);
}

THEN("^the player state has available_balance (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    ASSERT_EQ(tests::g_context.player_state.available_balance(), expected);
}
