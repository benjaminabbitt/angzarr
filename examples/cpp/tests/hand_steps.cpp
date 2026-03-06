// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

#include "angzarr/errors.hpp"
#include "examples/hand.pb.h"
#include "hand_state.hpp"
#include "test_context.hpp"

// Hand aggregate step definitions
// TODO: Implement step definitions for hand.feature scenarios

// ==========================================================================
// Given Steps - Setting up state from events
// ==========================================================================

GIVEN("^no prior events for the hand aggregate$") {
    tests::g_context.event_pages.clear();
    tests::g_context.hand_state = hand::HandState{};
}

// ==========================================================================
// When Steps - Handling commands
// ==========================================================================

WHEN("^I rebuild the hand state$") { tests::g_context.rebuild_hand_state(); }

// ==========================================================================
// Then Steps - Assertions on results
// ==========================================================================

THEN("^the result is a examples\\.CardsDealt event$") {
    ASSERT_TRUE(tests::g_context.result_event.has_value()) << "Expected a result event";
    auto event = tests::g_context.get_result_as<examples::CardsDealt>();
    ASSERT_TRUE(event.has_value()) << "Expected CardsDealt event";
}
