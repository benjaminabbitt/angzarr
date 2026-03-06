// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

#include "angzarr/errors.hpp"
#include "examples/table.pb.h"
#include "table_state.hpp"
#include "test_context.hpp"

// Table aggregate step definitions
// TODO: Implement step definitions for table.feature scenarios

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
    event.set_small_blind(1);
    event.set_big_blind(2);
    event.set_min_buy_in(100);
    event.set_max_buy_in(500);
    event.set_max_players(9);

    tests::g_context.add_event(event);
    tests::g_context.rebuild_table_state();
}

// ==========================================================================
// When Steps - Handling commands
// ==========================================================================

WHEN("^I rebuild the table state$") { tests::g_context.rebuild_table_state(); }

// ==========================================================================
// Then Steps - Assertions on results
// ==========================================================================

THEN("^the result is a examples\\.TableCreated event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::TableCreated>();
    ASSERT_TRUE(event.has_value()) << "Expected TableCreated event";
}
