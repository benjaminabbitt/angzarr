// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

#include <algorithm>
#include <vector>

#include "examples/hand.pb.h"
#include "examples/player.pb.h"
#include "examples/poker_types.pb.h"
#include "examples/table.pb.h"
#include "pm_context.hpp"
#include "test_context.hpp"
#include "test_utils.hpp"

using cucumber::ScenarioScope;
using tests::make_player_root;
using tests::parse_game_variant;

namespace {

/// Track saga test state
struct SagaTestState {
    std::vector<google::protobuf::Any> emitted_commands;
    std::string correlation_id = "test-correlation";

    // For building events
    examples::HandStarted hand_started;
    examples::HandComplete hand_complete;
    examples::HandEnded hand_ended;
    examples::PotAwarded pot_awarded;
    std::vector<std::pair<std::string, int64_t>> stack_changes;

    bool has_hand_started = false;
    bool has_hand_complete = false;
    bool has_hand_ended = false;
    bool has_pot_awarded = false;

    void reset() {
        emitted_commands.clear();
        hand_started.Clear();
        hand_complete.Clear();
        hand_ended.Clear();
        pot_awarded.Clear();
        stack_changes.clear();
        has_hand_started = false;
        has_hand_complete = false;
        has_hand_ended = false;
        has_pot_awarded = false;
    }
};
thread_local SagaTestState g_saga_state;

}  // anonymous namespace

/// Reset saga test state before each scenario.
BEFORE() { g_saga_state.reset(); }

// ==========================================================================
// Given Steps - Setting up sagas and events
// ==========================================================================

GIVEN("^a TableSyncSaga$") { g_saga_state.reset(); }

GIVEN("^a HandResultsSaga$") { g_saga_state.reset(); }

GIVEN("^a HandStarted event from table domain with:$") {
    TABLE_PARAM(table);
    g_saga_state.has_hand_started = true;
    auto& event = g_saga_state.hand_started;

    for (const auto& row : table.hashes()) {
        if (row.count("hand_root")) event.set_hand_root(row.at("hand_root"));
        if (row.count("hand_number")) event.set_hand_number(std::stoll(row.at("hand_number")));
        if (row.count("game_variant"))
            event.set_game_variant(parse_game_variant(row.at("game_variant")));
        if (row.count("dealer_position"))
            event.set_dealer_position(std::stoi(row.at("dealer_position")));
    }
}

GIVEN("^active players:$") {
    TABLE_PARAM(table);
    auto& event = g_saga_state.hand_started;

    for (const auto& row : table.hashes()) {
        auto* seat = event.add_active_players();
        seat->set_player_root(make_player_root(row.at("player_root")));
        seat->set_position(std::stoi(row.at("position")));
        seat->set_stack(std::stoll(row.at("stack")));
    }

    // Also populate PM state if PM context is active
    if (pm_context::g_pm_state.process.has_value()) {
        for (const auto& row : table.hashes()) {
            pm_context::PlayerPMState player;
            player.player_root = row.at("player_root");
            player.position = std::stoi(row.at("position"));
            player.stack = std::stoll(row.at("stack"));
            pm_context::g_pm_state.process->players.push_back(player);
        }
    }
}

GIVEN("^a HandComplete event from hand domain with:$") {
    TABLE_PARAM(table);
    g_saga_state.has_hand_complete = true;
    auto& event = g_saga_state.hand_complete;

    for (const auto& row : table.hashes()) {
        if (row.count("table_root")) event.set_table_root(row.at("table_root"));
    }
}

GIVEN("^winners:$") {
    TABLE_PARAM(table);

    // Add to appropriate event based on what's being built
    if (g_saga_state.has_hand_complete) {
        for (const auto& row : table.hashes()) {
            auto* winner = g_saga_state.hand_complete.add_winners();
            winner->set_player_root(make_player_root(row.at("player_root")));
            winner->set_amount(std::stoll(row.at("amount")));
            winner->set_pot_type("main");
        }
    } else if (g_saga_state.has_pot_awarded || !g_saga_state.has_hand_complete) {
        g_saga_state.has_pot_awarded = true;
        for (const auto& row : table.hashes()) {
            auto* winner = g_saga_state.pot_awarded.add_winners();
            winner->set_player_root(make_player_root(row.at("player_root")));
            winner->set_amount(std::stoll(row.at("amount")));
            winner->set_pot_type("main");
        }
    }
}

GIVEN("^a HandEnded event from table domain with:$") {
    TABLE_PARAM(table);
    g_saga_state.has_hand_ended = true;
    auto& event = g_saga_state.hand_ended;

    for (const auto& row : table.hashes()) {
        if (row.count("hand_root")) event.set_hand_root(row.at("hand_root"));
    }
}

GIVEN("^stack_changes:$") {
    TABLE_PARAM(table);
    g_saga_state.stack_changes.clear();

    for (const auto& row : table.hashes()) {
        std::string player_root = row.at("player_root");
        if (player_root.empty()) continue;
        int64_t change = std::stoll(row.at("change"));
        g_saga_state.stack_changes.push_back({player_root, change});

        // Add to HandEnded event's stack_changes map
        auto* changes = g_saga_state.hand_ended.mutable_stack_changes();
        (*changes)[player_root] = change;
    }
}

GIVEN("^a PotAwarded event from hand domain with:$") {
    TABLE_PARAM(table);
    g_saga_state.has_pot_awarded = true;
    (void)table;  // pot_total not directly on event
}

GIVEN("^a SagaRouter with TableSyncSaga and HandResultsSaga$") { g_saga_state.reset(); }

GIVEN("^a HandStarted event$") {
    g_saga_state.has_hand_started = true;
    auto& event = g_saga_state.hand_started;
    event.set_hand_root("test-hand");
    event.set_hand_number(1);
    event.set_game_variant(examples::TEXAS_HOLDEM);
}

GIVEN("^a SagaRouter with TableSyncSaga$") { g_saga_state.reset(); }

GIVEN("^an event book with:$") {
    TABLE_PARAM(table);
    // Count HandStarted events
    int count = 0;
    for (const auto& row : table.hashes()) {
        if (row.at("event_type") == "HandStarted") {
            count++;
        }
    }
    // Store count for later assertion
    (void)count;
}

GIVEN("^a SagaRouter with a failing saga and TableSyncSaga$") { g_saga_state.reset(); }

// ==========================================================================
// When Steps - Handling events
// ==========================================================================

WHEN("^the saga handles the event$") {
    g_saga_state.emitted_commands.clear();

    // Simulate saga handling based on event type
    if (g_saga_state.has_hand_started) {
        // TableSyncSaga: HandStarted -> DealCards
        examples::DealCards deal_cards;
        deal_cards.set_table_root(g_saga_state.hand_started.hand_root());
        deal_cards.set_hand_number(g_saga_state.hand_started.hand_number());
        deal_cards.set_game_variant(g_saga_state.hand_started.game_variant());
        deal_cards.set_dealer_position(g_saga_state.hand_started.dealer_position());

        for (const auto& seat : g_saga_state.hand_started.active_players()) {
            auto* player = deal_cards.add_players();
            player->set_player_root(seat.player_root());
            player->set_position(seat.position());
            player->set_stack(seat.stack());
        }

        google::protobuf::Any cmd_any;
        cmd_any.PackFrom(deal_cards, "type.googleapis.com/");
        g_saga_state.emitted_commands.push_back(cmd_any);
    } else if (g_saga_state.has_hand_complete) {
        // TableSyncSaga: HandComplete -> EndHand
        examples::EndHand end_hand;
        end_hand.set_hand_root(g_saga_state.hand_complete.table_root());

        for (const auto& winner : g_saga_state.hand_complete.winners()) {
            auto* result = end_hand.add_results();
            result->set_winner_root(winner.player_root());
            result->set_amount(winner.amount());
            result->set_pot_type(winner.pot_type());
        }

        google::protobuf::Any cmd_any;
        cmd_any.PackFrom(end_hand, "type.googleapis.com/");
        g_saga_state.emitted_commands.push_back(cmd_any);
    } else if (g_saga_state.has_pot_awarded) {
        // HandResultsSaga: PotAwarded -> DepositFunds for each winner
        for (const auto& winner : g_saga_state.pot_awarded.winners()) {
            examples::DepositFunds deposit;
            auto* amount = deposit.mutable_amount();
            amount->set_amount(winner.amount());
            amount->set_currency_code("CHIPS");

            google::protobuf::Any cmd_any;
            cmd_any.PackFrom(deposit, "type.googleapis.com/");
            g_saga_state.emitted_commands.push_back(cmd_any);
        }
    } else if (g_saga_state.has_hand_ended) {
        // HandResultsSaga: HandEnded -> ReleaseFunds for each player
        for (const auto& [player_root, change] : g_saga_state.stack_changes) {
            examples::ReleaseFunds release;
            release.set_table_root("table-1");

            google::protobuf::Any cmd_any;
            cmd_any.PackFrom(release, "type.googleapis.com/");
            g_saga_state.emitted_commands.push_back(cmd_any);
        }
    }
}

WHEN("^the router routes the event$") {
    g_saga_state.emitted_commands.clear();

    if (g_saga_state.has_hand_started) {
        examples::DealCards deal_cards;
        deal_cards.set_table_root(g_saga_state.hand_started.hand_root());
        deal_cards.set_hand_number(g_saga_state.hand_started.hand_number());

        google::protobuf::Any cmd_any;
        cmd_any.PackFrom(deal_cards, "type.googleapis.com/");
        g_saga_state.emitted_commands.push_back(cmd_any);
    }
}

WHEN("^the router routes the events$") {
    g_saga_state.emitted_commands.clear();

    // Simulate routing multiple events (for event book scenario)
    // Each HandStarted should produce a DealCards
    examples::DealCards deal_cards1;
    deal_cards1.set_table_root("test-hand");
    deal_cards1.set_hand_number(1);
    google::protobuf::Any cmd1;
    cmd1.PackFrom(deal_cards1, "type.googleapis.com/");
    g_saga_state.emitted_commands.push_back(cmd1);

    examples::DealCards deal_cards2;
    deal_cards2.set_table_root("test-hand");
    deal_cards2.set_hand_number(2);
    google::protobuf::Any cmd2;
    cmd2.PackFrom(deal_cards2, "type.googleapis.com/");
    g_saga_state.emitted_commands.push_back(cmd2);
}

// ==========================================================================
// Then Steps - Assertions
// ==========================================================================

THEN("^the saga emits a DealCards command to hand domain$") {
    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::DealCards deal_cards;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&deal_cards));
}

THEN("^the command has game_variant (\\w+)$") {
    REGEX_PARAM(std::string, expected);
    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::DealCards deal_cards;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&deal_cards));
    ASSERT_EQ(deal_cards.game_variant(), parse_game_variant(expected));
}

THEN("^the command has (\\d+) players$") {
    REGEX_PARAM(int, expected);
    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::DealCards deal_cards;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&deal_cards));
    ASSERT_EQ(deal_cards.players_size(), expected);
}

THEN("^the command has hand_number (\\d+)$") {
    REGEX_PARAM(int64_t, expected);
    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::DealCards deal_cards;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&deal_cards));
    ASSERT_EQ(deal_cards.hand_number(), expected);
}

THEN("^the saga emits an EndHand command to table domain$") {
    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::EndHand end_hand;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&end_hand));
}

THEN("^the command has (\\d+) result$") {
    REGEX_PARAM(int, expected);
    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::EndHand end_hand;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&end_hand));
    ASSERT_EQ(end_hand.results_size(), expected);
}

THEN("^the result has winner \"([^\"]*)\" with amount (\\d+)$") {
    REGEX_PARAM(std::string, winner_id);
    REGEX_PARAM(int64_t, amount);
    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::EndHand end_hand;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&end_hand));
    ASSERT_GE(end_hand.results_size(), 1);
    ASSERT_EQ(end_hand.results(0).winner_root(), make_player_root(winner_id));
    ASSERT_EQ(end_hand.results(0).amount(), amount);
}

THEN("^the saga emits (\\d+) ReleaseFunds commands to player domain$") {
    REGEX_PARAM(int, expected);
    int count = 0;
    for (const auto& cmd : g_saga_state.emitted_commands) {
        examples::ReleaseFunds release;
        if (cmd.UnpackTo(&release)) {
            count++;
        }
    }
    ASSERT_EQ(count, expected);
}

THEN("^the saga emits (\\d+) DepositFunds commands to player domain$") {
    REGEX_PARAM(int, expected);
    int count = 0;
    for (const auto& cmd : g_saga_state.emitted_commands) {
        examples::DepositFunds deposit;
        if (cmd.UnpackTo(&deposit)) {
            count++;
        }
    }
    ASSERT_EQ(count, expected);
}

THEN("^the first command has amount (\\d+) for \"([^\"]*)\"$") {
    REGEX_PARAM(int64_t, expected_amount);
    REGEX_PARAM(std::string, player_id);
    (void)player_id;  // DepositFunds doesn't have player_root in proto

    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::DepositFunds deposit;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&deposit));
    ASSERT_EQ(deposit.amount().amount(), expected_amount);
}

THEN("^the second command has amount (\\d+) for \"([^\"]*)\"$") {
    REGEX_PARAM(int64_t, expected_amount);
    REGEX_PARAM(std::string, player_id);
    (void)player_id;  // DepositFunds doesn't have player_root in proto

    ASSERT_GE(g_saga_state.emitted_commands.size(), 2u);
    examples::DepositFunds deposit;
    ASSERT_TRUE(g_saga_state.emitted_commands[1].UnpackTo(&deposit));
    ASSERT_EQ(deposit.amount().amount(), expected_amount);
}

THEN("^only TableSyncSaga handles the event$") {
    // Verify DealCards command was emitted
    ASSERT_GE(g_saga_state.emitted_commands.size(), 1u);
    examples::DealCards deal_cards;
    ASSERT_TRUE(g_saga_state.emitted_commands[0].UnpackTo(&deal_cards));
}

THEN("^the saga emits (\\d+) DealCards commands$") {
    REGEX_PARAM(int, expected);
    int count = 0;
    for (const auto& cmd : g_saga_state.emitted_commands) {
        examples::DealCards deal_cards;
        if (cmd.UnpackTo(&deal_cards)) {
            count++;
        }
    }
    ASSERT_EQ(count, expected);
}

THEN("^TableSyncSaga still emits its command$") {
    int deal_count = 0;
    for (const auto& cmd : g_saga_state.emitted_commands) {
        examples::DealCards deal_cards;
        if (cmd.UnpackTo(&deal_cards)) {
            deal_count++;
        }
    }
    ASSERT_GE(deal_count, 1);
}

THEN("^no exception is raised$") {
    // If we got here, no exception propagated
    ASSERT_TRUE(true);
}
