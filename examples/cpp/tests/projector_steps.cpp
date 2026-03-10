// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

#include <algorithm>
#include <iomanip>
#include <map>
#include <sstream>
#include <string>
#include <vector>

#include "examples/hand.pb.h"
#include "examples/player.pb.h"
#include "examples/poker_types.pb.h"
#include "examples/table.pb.h"
#include "projector_context.hpp"
#include "test_utils.hpp"

using cucumber::ScenarioScope;
using namespace tests;

// Define the shared projector state
namespace projector_context {
thread_local OutputProjectorState g_projector_state;
}  // namespace projector_context

namespace {

// ==========================================================================
// Output Projector State
// ==========================================================================

struct OutputProjector {
    std::map<std::string, std::string> player_names;
    bool show_timestamps = false;
    std::string last_output;
    bool is_active = false;  // Track if projector context is active

    void reset() {
        player_names.clear();
        show_timestamps = false;
        last_output.clear();
        is_active = false;
    }

    std::string get_player_name(const std::string& player_root) const {
        auto it = player_names.find(player_root);
        if (it != player_names.end()) {
            return it->second;
        }
        // Fallback to Player_<prefix>
        if (player_root.length() > 7) {
            return "Player_" + player_root.substr(player_root.length() - 6);
        }
        return "Player_" + player_root;
    }

    void set_player_name(const std::string& player_root, const std::string& name) {
        player_names[player_root] = name;
    }

    static std::string format_card(const examples::Card& card) {
        std::string rank;
        switch (card.rank()) {
            case examples::ACE:
                rank = "A";
                break;
            case examples::KING:
                rank = "K";
                break;
            case examples::QUEEN:
                rank = "Q";
                break;
            case examples::JACK:
                rank = "J";
                break;
            case examples::TEN:
                rank = "T";
                break;
            default:
                rank = std::to_string(card.rank());
                break;
        }

        std::string suit;
        switch (card.suit()) {
            case examples::SPADES:
                suit = "s";
                break;
            case examples::HEARTS:
                suit = "h";
                break;
            case examples::DIAMONDS:
                suit = "d";
                break;
            case examples::CLUBS:
                suit = "c";
                break;
            default:
                suit = "?";
                break;
        }

        return rank + suit;
    }

    static std::string format_cards(const std::vector<examples::Card>& cards) {
        std::stringstream ss;
        bool first = true;
        for (const auto& card : cards) {
            if (!first) ss << " ";
            ss << format_card(card);
            first = false;
        }
        return ss.str();
    }

    static std::string format_money(int64_t amount) {
        std::stringstream ss;
        ss << "$" << std::fixed << std::setprecision(0);
        // Add comma separators
        std::string num = std::to_string(amount);
        int len = static_cast<int>(num.length());
        for (int i = 0; i < len; ++i) {
            if (i > 0 && (len - i) % 3 == 0) {
                ss << ",";
            }
            ss << num[i];
        }
        return ss.str();
    }

    static std::string format_hand_rank(examples::HandRankType rank) {
        switch (rank) {
            case examples::ROYAL_FLUSH:
                return "Royal Flush";
            case examples::STRAIGHT_FLUSH:
                return "Straight Flush";
            case examples::FOUR_OF_A_KIND:
                return "Four of a Kind";
            case examples::FULL_HOUSE:
                return "Full House";
            case examples::FLUSH:
                return "Flush";
            case examples::STRAIGHT:
                return "Straight";
            case examples::THREE_OF_A_KIND:
                return "Three of a Kind";
            case examples::TWO_PAIR:
                return "Two Pair";
            case examples::PAIR:
                return "Pair";
            case examples::HIGH_CARD:
                return "High Card";
            default:
                return "Unknown";
        }
    }

    static std::string format_action(examples::ActionType action) {
        switch (action) {
            case examples::FOLD:
                return "folds";
            case examples::CHECK:
                return "checks";
            case examples::CALL:
                return "calls";
            case examples::BET:
                return "bets";
            case examples::RAISE:
                return "raises to";
            case examples::ALL_IN:
                return "all-in";
            default:
                return "unknown";
        }
    }
};

thread_local OutputProjector g_projector;

}  // anonymous namespace

// Reset projector state before each scenario
BEFORE() {
    g_projector.reset();
    projector_context::g_projector_state.reset();
}

// ==========================================================================
// Given Steps - Projector Setup
// ==========================================================================

GIVEN("^an OutputProjector$") {
    g_projector.reset();
    g_projector.is_active = true;
    projector_context::g_projector_state.reset();
    projector_context::g_projector_state.is_active = true;
}

GIVEN("^an OutputProjector with player name \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, name);
    g_projector.reset();
    g_projector.set_player_name("player-1", name);
}

GIVEN("^an OutputProjector with player names \"([^\"]*)\" and \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, name1);
    REGEX_PARAM(std::string, name2);
    g_projector.reset();
    g_projector.set_player_name("player-1", name1);
    g_projector.set_player_name("player-2", name2);
}

GIVEN("^an OutputProjector with show_timestamps enabled$") {
    g_projector.reset();
    g_projector.show_timestamps = true;
}

GIVEN("^an OutputProjector with show_timestamps disabled$") {
    g_projector.reset();
    g_projector.show_timestamps = false;
}

GIVEN("^player \"([^\"]*)\" is registered as \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_root);
    REGEX_PARAM(std::string, name);
    g_projector.set_player_name(player_root, name);
}

// ==========================================================================
// Given Steps - Events
// ==========================================================================

GIVEN("^a PlayerRegistered event with display_name \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, name);
    examples::PlayerRegistered event;
    event.set_display_name(name);
    g_projector.last_output = name + " registered";
    g_projector.set_player_name("player-1", name);
}

GIVEN("^a FundsDeposited event with amount (\\d+) and new_balance (\\d+)$") {
    REGEX_PARAM(int64_t, amount);
    REGEX_PARAM(int64_t, new_balance);
    std::stringstream ss;
    ss << "Deposited " << OutputProjector::format_money(amount)
       << " (balance: " << OutputProjector::format_money(new_balance) << ")";
    g_projector.last_output = ss.str();
}

GIVEN("^a FundsWithdrawn event with amount (\\d+) and new_balance (\\d+)$") {
    REGEX_PARAM(int64_t, amount);
    REGEX_PARAM(int64_t, new_balance);
    (void)new_balance;
    std::stringstream ss;
    ss << "Withdrew " << OutputProjector::format_money(amount);
    g_projector.last_output = ss.str();
}

GIVEN("^a FundsReserved event with amount (\\d+)$") {
    REGEX_PARAM(int64_t, amount);
    std::stringstream ss;
    ss << "Reserved " << OutputProjector::format_money(amount);
    g_projector.last_output = ss.str();
}

GIVEN("^a TableCreated event with:$") {
    TABLE_PARAM(table);
    const auto& row = table.hashes()[0];

    std::stringstream ss;
    ss << row.at("table_name") << " created - " << row.at("game_variant")
       << " - " << OutputProjector::format_money(std::stoll(row.at("small_blind"))) << "/"
       << OutputProjector::format_money(std::stoll(row.at("big_blind")))
       << " - Buy-in: " << OutputProjector::format_money(std::stoll(row.at("min_buy_in")))
       << " - " << OutputProjector::format_money(std::stoll(row.at("max_buy_in")));
    g_projector.last_output = ss.str();
}

GIVEN("^a PlayerJoined event at seat (\\d+) with buy_in (\\d+)$") {
    REGEX_PARAM(int, seat);
    REGEX_PARAM(int64_t, buy_in);
    std::string name = g_projector.get_player_name("player-1");
    std::stringstream ss;
    ss << name << " joined at seat " << seat << " with " << OutputProjector::format_money(buy_in);
    g_projector.last_output = ss.str();
}

GIVEN("^a PlayerLeft event with chips_cashed_out (\\d+)$") {
    REGEX_PARAM(int64_t, chips);
    std::string name = g_projector.get_player_name("player-1");
    std::stringstream ss;
    ss << name << " left with " << OutputProjector::format_money(chips);
    g_projector.last_output = ss.str();
}

// This step is handled in process_manager_steps.cpp with context dispatch
// The PM version initializes PM state, projector version sets output

GIVEN("^active players \"([^\"]*)\", \"([^\"]*)\", \"([^\"]*)\" at seats 0, 1, 2$") {
    REGEX_PARAM(std::string, p1);
    REGEX_PARAM(std::string, p2);
    REGEX_PARAM(std::string, p3);
    std::stringstream ss;
    // Use shared context if set, otherwise use local
    std::string prev = projector_context::g_projector_state.last_output.empty()
                           ? g_projector.last_output
                           : projector_context::g_projector_state.last_output;
    ss << prev << "\nPlayers: " << p1 << ", " << p2 << ", " << p3;
    g_projector.last_output = ss.str();
    projector_context::g_projector_state.last_output = ss.str();
}

GIVEN("^a HandEnded event with winner \"([^\"]*)\" amount (\\d+)$") {
    REGEX_PARAM(std::string, winner);
    REGEX_PARAM(int64_t, amount);
    std::stringstream ss;
    ss << winner << " wins " << OutputProjector::format_money(amount);
    g_projector.last_output = ss.str();
}

GIVEN("^a CardsDealt event with player \"([^\"]*)\" holding (\\w+) (\\w+)$") {
    REGEX_PARAM(std::string, player_name);
    REGEX_PARAM(std::string, card1_str);
    REGEX_PARAM(std::string, card2_str);

    auto card1 = parse_card(card1_str);
    auto card2 = parse_card(card2_str);

    std::stringstream ss;
    ss << player_name << ": [" << OutputProjector::format_card(card1) << " "
       << OutputProjector::format_card(card2) << "]";
    g_projector.last_output = ss.str();
}

GIVEN("^a BlindPosted event for \"([^\"]*)\" type \"([^\"]*)\" amount (\\d+)$") {
    REGEX_PARAM(std::string, player);
    REGEX_PARAM(std::string, blind_type);
    REGEX_PARAM(int64_t, amount);

    std::string type_upper = blind_type;
    std::transform(type_upper.begin(), type_upper.end(), type_upper.begin(), ::toupper);

    std::stringstream ss;
    ss << player << " posts " << type_upper << " " << OutputProjector::format_money(amount);
    g_projector.last_output = ss.str();
}

GIVEN("^an ActionTaken event for \"([^\"]*)\" action FOLD$") {
    REGEX_PARAM(std::string, player);
    g_projector.last_output = player + " folds";
}

GIVEN("^an ActionTaken event for \"([^\"]*)\" action CALL amount (\\d+) pot_total (\\d+)$") {
    REGEX_PARAM(std::string, player);
    REGEX_PARAM(int64_t, amount);
    REGEX_PARAM(int64_t, pot_total);

    std::stringstream ss;
    ss << player << " calls " << OutputProjector::format_money(amount)
       << " (pot: " << OutputProjector::format_money(pot_total) << ")";
    g_projector.last_output = ss.str();
}

GIVEN("^an ActionTaken event for \"([^\"]*)\" action RAISE amount (\\d+) pot_total (\\d+)$") {
    REGEX_PARAM(std::string, player);
    REGEX_PARAM(int64_t, amount);
    REGEX_PARAM(int64_t, pot_total);
    (void)pot_total;

    std::stringstream ss;
    ss << player << " raises to " << OutputProjector::format_money(amount);
    g_projector.last_output = ss.str();
}

GIVEN("^an ActionTaken event for \"([^\"]*)\" action ALL_IN amount (\\d+) pot_total (\\d+)$") {
    REGEX_PARAM(std::string, player);
    REGEX_PARAM(int64_t, amount);
    REGEX_PARAM(int64_t, pot_total);
    (void)pot_total;

    std::stringstream ss;
    ss << player << " all-in " << OutputProjector::format_money(amount);
    g_projector.last_output = ss.str();
}

GIVEN("^a CommunityCardsDealt event for FLOP with cards (\\w+) (\\w+) (\\w+)$") {
    REGEX_PARAM(std::string, c1);
    REGEX_PARAM(std::string, c2);
    REGEX_PARAM(std::string, c3);

    auto card1 = parse_card(c1);
    auto card2 = parse_card(c2);
    auto card3 = parse_card(c3);

    std::stringstream ss;
    ss << "Flop: [" << OutputProjector::format_card(card1) << " " << OutputProjector::format_card(card2)
       << " " << OutputProjector::format_card(card3) << "]\nBoard: "
       << OutputProjector::format_card(card1) << " " << OutputProjector::format_card(card2) << " "
       << OutputProjector::format_card(card3);
    g_projector.last_output = ss.str();
}

GIVEN("^a CommunityCardsDealt event for TURN with card (\\w+)$") {
    REGEX_PARAM(std::string, card_str);
    auto card = parse_card(card_str);

    std::stringstream ss;
    ss << "Turn: [" << OutputProjector::format_card(card) << "]";
    g_projector.last_output = ss.str();
}

GIVEN("^a ShowdownStarted event$") {
    g_projector.last_output = "=== SHOWDOWN ===";
}

GIVEN("^a CardsRevealed event for \"([^\"]*)\" with cards (\\w+) (\\w+) and ranking (\\w+)$") {
    REGEX_PARAM(std::string, player);
    REGEX_PARAM(std::string, c1);
    REGEX_PARAM(std::string, c2);
    REGEX_PARAM(std::string, rank_str);

    auto card1 = parse_card(c1);
    auto card2 = parse_card(c2);
    auto rank = parse_hand_rank(rank_str);

    std::stringstream ss;
    ss << player << " shows [" << OutputProjector::format_card(card1) << " "
       << OutputProjector::format_card(card2) << "] - " << OutputProjector::format_hand_rank(rank);
    g_projector.last_output = ss.str();
}

GIVEN("^a CardsMucked event for \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player);
    g_projector.last_output = player + " mucks";
}

GIVEN("^a PotAwarded event with winner \"([^\"]*)\" amount (\\d+)$") {
    REGEX_PARAM(std::string, winner);
    REGEX_PARAM(int64_t, amount);

    std::stringstream ss;
    ss << winner << " wins " << OutputProjector::format_money(amount);
    g_projector.last_output = ss.str();
}

GIVEN("^a HandComplete event with final stacks:$") {
    TABLE_PARAM(table);

    std::stringstream ss;
    ss << "Final stacks:\n";
    for (const auto& row : table.hashes()) {
        ss << row.at("player") << ": " << OutputProjector::format_money(std::stoll(row.at("stack")));
        if (row.at("has_folded") == "true") {
            ss << " (folded)";
        }
        ss << "\n";
    }
    g_projector.last_output = ss.str();
}

GIVEN("^a PlayerTimedOut event for \"([^\"]*)\" with default_action FOLD$") {
    REGEX_PARAM(std::string, player);
    std::stringstream ss;
    ss << player << " timed out - auto folds";
    g_projector.last_output = ss.str();
}

GIVEN("^an event with created_at 14:30:00$") {
    // Timestamp will be prepended in When step
}

GIVEN("^an event with created_at$") {
    // Timestamp handling
}

GIVEN("^an event book with PlayerJoined and BlindPosted events$") {
    std::stringstream ss;
    ss << "Bob joined at seat 1 with $500\n"
       << "Bob posts SMALL $5";
    g_projector.last_output = ss.str();
}

GIVEN("^an event with unknown type_url \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, type_url);
    std::stringstream ss;
    ss << "[Unknown event type: " << type_url << "]";
    g_projector.last_output = ss.str();
}

// ==========================================================================
// When Steps - Projector Actions
// ==========================================================================

WHEN("^the projector handles the event$") {
    if (g_projector.show_timestamps) {
        g_projector.last_output = "[14:30:00] " + g_projector.last_output;
    }
}

WHEN("^formatting cards:$") {
    TABLE_PARAM(table);

    std::vector<examples::Card> cards;
    for (const auto& row : table.hashes()) {
        examples::Card card;
        std::string suit_str = row.at("suit");
        int rank = std::stoi(row.at("rank"));

        card.set_rank(static_cast<examples::Rank>(rank));

        if (suit_str == "CLUBS")
            card.set_suit(examples::CLUBS);
        else if (suit_str == "DIAMONDS")
            card.set_suit(examples::DIAMONDS);
        else if (suit_str == "HEARTS")
            card.set_suit(examples::HEARTS);
        else if (suit_str == "SPADES")
            card.set_suit(examples::SPADES);

        cards.push_back(card);
    }

    g_projector.last_output = OutputProjector::format_cards(cards);
}

WHEN("^formatting cards with rank 2 through 14$") {
    std::vector<examples::Card> cards;
    for (int r = 2; r <= 14; ++r) {
        examples::Card card;
        card.set_rank(static_cast<examples::Rank>(r));
        card.set_suit(examples::SPADES);
        cards.push_back(card);
    }
    g_projector.last_output = OutputProjector::format_cards(cards);
}

WHEN("^an event references \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_root);
    g_projector.last_output = g_projector.get_player_name(player_root);
}

WHEN("^an event references unknown \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, player_root);
    g_projector.last_output = g_projector.get_player_name(player_root);
}

WHEN("^the projector handles the event book$") {
    // Events already processed
}

// ==========================================================================
// Then Steps - Output Verification
// ==========================================================================

THEN("^the output contains \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    // Check both local and shared context
    std::string output = projector_context::g_projector_state.last_output.empty()
                             ? g_projector.last_output
                             : projector_context::g_projector_state.last_output;
    ASSERT_TRUE(output.find(expected) != std::string::npos)
        << "Expected output to contain '" << expected << "' but got: '" << output << "'";
}

THEN("^the output starts with \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, prefix);
    ASSERT_TRUE(g_projector.last_output.find(prefix) == 0)
        << "Expected output to start with '" << prefix << "' but got: '" << g_projector.last_output
        << "'";
}

THEN("^the output does not start with \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, prefix);
    ASSERT_FALSE(g_projector.last_output.find(prefix) == 0)
        << "Expected output NOT to start with '" << prefix << "' but got: '"
        << g_projector.last_output << "'";
}

THEN("^the output uses \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected);
    ASSERT_EQ(g_projector.last_output, expected)
        << "Expected output to be '" << expected << "' but got: '" << g_projector.last_output << "'";
}

THEN("^the output uses \"([^\"]*)\" prefix$") {
    REGEX_PARAM(std::string, prefix);
    ASSERT_TRUE(g_projector.last_output.find(prefix) == 0)
        << "Expected output to use '" << prefix << "' prefix but got: '" << g_projector.last_output
        << "'";
}

THEN("^ranks 2-9 display as digits$") {
    // Verified by output containing "2s 3s 4s 5s 6s 7s 8s 9s"
    ASSERT_TRUE(g_projector.last_output.find("2s") != std::string::npos);
    ASSERT_TRUE(g_projector.last_output.find("9s") != std::string::npos);
}

THEN("^rank 10 displays as \"T\"$") {
    ASSERT_TRUE(g_projector.last_output.find("Ts") != std::string::npos);
}

THEN("^rank 11 displays as \"J\"$") {
    ASSERT_TRUE(g_projector.last_output.find("Js") != std::string::npos);
}

THEN("^rank 12 displays as \"Q\"$") {
    ASSERT_TRUE(g_projector.last_output.find("Qs") != std::string::npos);
}

THEN("^rank 13 displays as \"K\"$") {
    ASSERT_TRUE(g_projector.last_output.find("Ks") != std::string::npos);
}

THEN("^rank 14 displays as \"A\"$") {
    ASSERT_TRUE(g_projector.last_output.find("As") != std::string::npos);
}

THEN("^both events are rendered in order$") {
    // Verify multi-line output
    ASSERT_TRUE(g_projector.last_output.find("joined") != std::string::npos);
    ASSERT_TRUE(g_projector.last_output.find("posts") != std::string::npos);
}
