#include "text_renderer.hpp"
#include <sstream>
#include <iomanip>

namespace projector {

void TextRenderer::set_player_name(const std::string& player_root, const std::string& name) {
    // Convert bytes to hex for key
    std::stringstream ss;
    for (unsigned char c : player_root) {
        ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
    }
    player_names_[ss.str()] = name;
}

std::string TextRenderer::get_player_name(const std::string& player_root) const {
    std::stringstream ss;
    for (unsigned char c : player_root) {
        ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
    }
    std::string key = ss.str();

    auto it = player_names_.find(key);
    if (it != player_names_.end()) {
        return it->second;
    }
    // Return shortened hex if no name set
    return key.length() >= 8 ? key.substr(0, 8) : key;
}

std::string TextRenderer::render_card(const examples::Card& card) {
    std::string rank;
    if (card.rank() == 14) rank = "A";
    else if (card.rank() == 13) rank = "K";
    else if (card.rank() == 12) rank = "Q";
    else if (card.rank() == 11) rank = "J";
    else if (card.rank() == 10) rank = "T";
    else rank = std::to_string(card.rank());

    std::string suit;
    switch (card.suit()) {
        case examples::SPADES: suit = "♠"; break;
        case examples::HEARTS: suit = "♥"; break;
        case examples::DIAMONDS: suit = "♦"; break;
        case examples::CLUBS: suit = "♣"; break;
        default: suit = "?"; break;
    }

    return rank + suit;
}

std::string TextRenderer::render_action(examples::ActionType action) {
    switch (action) {
        case examples::FOLD: return "folds";
        case examples::CHECK: return "checks";
        case examples::CALL: return "calls";
        case examples::BET: return "bets";
        case examples::RAISE: return "raises";
        case examples::ALL_IN: return "all-in";
        default: return "unknown";
    }
}

std::string TextRenderer::render_player_registered(const examples::PlayerRegistered& event) {
    std::stringstream ss;
    ss << "Player '" << event.display_name() << "' registered";
    if (event.player_type() == examples::AI) {
        ss << " (AI)";
    }
    return ss.str();
}

std::string TextRenderer::render_funds_deposited(const examples::FundsDeposited& event) {
    std::stringstream ss;
    ss << "Deposited " << event.amount().amount()
       << " (new balance: " << event.new_balance().amount() << ")";
    return ss.str();
}

std::string TextRenderer::render_funds_withdrawn(const examples::FundsWithdrawn& event) {
    std::stringstream ss;
    ss << "Withdrew " << event.amount().amount()
       << " (new balance: " << event.new_balance().amount() << ")";
    return ss.str();
}

std::string TextRenderer::render_funds_reserved(const examples::FundsReserved& event) {
    std::stringstream ss;
    ss << "Reserved " << event.amount().amount() << " for table";
    return ss.str();
}

std::string TextRenderer::render_funds_released(const examples::FundsReleased& event) {
    std::stringstream ss;
    ss << "Released reserved funds (new balance: " << event.new_available_balance().amount() << ")";
    return ss.str();
}

std::string TextRenderer::render_table_created(const examples::TableCreated& event) {
    std::stringstream ss;
    ss << "Table '" << event.table_name() << "' created - "
       << event.small_blind() << "/" << event.big_blind() << " blinds, "
       << "max " << event.max_players() << " players";
    return ss.str();
}

std::string TextRenderer::render_player_joined(const examples::PlayerJoined& event) {
    std::stringstream ss;
    ss << get_player_name(event.player_root()) << " joined at seat "
       << event.seat_position() << " with " << event.stack();
    return ss.str();
}

std::string TextRenderer::render_player_left(const examples::PlayerLeft& event) {
    std::stringstream ss;
    ss << get_player_name(event.player_root()) << " left with "
       << event.chips_cashed_out();
    return ss.str();
}

std::string TextRenderer::render_hand_started(const examples::HandStarted& event) {
    std::stringstream ss;
    ss << "=== Hand #" << event.hand_number() << " ===\n"
       << "Dealer: seat " << event.dealer_position() << ", "
       << "Blinds: " << event.small_blind() << "/" << event.big_blind();
    return ss.str();
}

std::string TextRenderer::render_hand_ended(const examples::HandEnded& event) {
    return "Hand ended";
}

std::string TextRenderer::render_cards_dealt(const examples::CardsDealt& event) {
    std::stringstream ss;
    ss << "Cards dealt to " << event.player_cards_size() << " players";
    return ss.str();
}

std::string TextRenderer::render_blind_posted(const examples::BlindPosted& event) {
    std::stringstream ss;
    ss << get_player_name(event.player_root()) << " posts "
       << event.blind_type() << " blind: " << event.amount();
    return ss.str();
}

std::string TextRenderer::render_action_taken(const examples::ActionTaken& event) {
    std::stringstream ss;
    ss << get_player_name(event.player_root()) << " "
       << render_action(event.action());
    if (event.amount() > 0) {
        ss << " " << event.amount();
    }
    return ss.str();
}

std::string TextRenderer::render_community_cards_dealt(const examples::CommunityCardsDealt& event) {
    std::stringstream ss;
    ss << "*** ";
    switch (event.phase()) {
        case examples::FLOP: ss << "FLOP"; break;
        case examples::TURN: ss << "TURN"; break;
        case examples::RIVER: ss << "RIVER"; break;
        default: ss << "COMMUNITY"; break;
    }
    ss << " *** [";
    bool first = true;
    for (const auto& card : event.all_community_cards()) {
        if (!first) ss << " ";
        ss << render_card(card);
        first = false;
    }
    ss << "]";
    return ss.str();
}

std::string TextRenderer::render_pot_awarded(const examples::PotAwarded& event) {
    std::stringstream ss;
    ss << "*** POT AWARDED ***\n";
    for (const auto& winner : event.winners()) {
        ss << get_player_name(winner.player_root()) << " wins "
           << winner.amount() << "\n";
    }
    return ss.str();
}

std::string TextRenderer::render_hand_complete(const examples::HandComplete& event) {
    std::stringstream ss;
    ss << "=== Hand Complete ===\n";
    ss << "Final stacks:\n";
    for (const auto& stack : event.final_stacks()) {
        ss << "  " << get_player_name(stack.player_root()) << ": "
           << stack.stack() << "\n";
    }
    return ss.str();
}

} // namespace projector
