#include "player_state.hpp"
#include <algorithm>
#include <iomanip>
#include <sstream>

namespace player {

namespace {

std::string bytes_to_hex(const std::string& bytes) {
    std::ostringstream ss;
    for (unsigned char c : bytes) {
        ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
    }
    return ss.str();
}

bool ends_with(const std::string& str, const std::string& suffix) {
    if (suffix.size() > str.size()) return false;
    return str.compare(str.size() - suffix.size(), suffix.size(), suffix) == 0;
}

} // anonymous namespace

PlayerState PlayerState::from_event_book(const angzarr::EventBook& event_book) {
    PlayerState state;
    for (const auto& page : event_book.pages()) {
        apply_event(state, page.event());
    }
    return state;
}

void PlayerState::apply_event(PlayerState& state, const google::protobuf::Any& event_any) {
    const std::string& type_url = event_any.type_url();

    if (ends_with(type_url, "PlayerRegistered")) {
        examples::PlayerRegistered event;
        if (event_any.UnpackTo(&event)) {
            state.player_id = "player_" + event.email();
            state.display_name = event.display_name();
            state.email = event.email();
            state.player_type = event.player_type();
            state.ai_model_id = event.ai_model_id();
            state.status = "active";
            state.bankroll = 0;
            state.reserved_funds = 0;
        }
    } else if (ends_with(type_url, "FundsDeposited")) {
        examples::FundsDeposited event;
        if (event_any.UnpackTo(&event)) {
            if (event.has_new_balance()) {
                state.bankroll = event.new_balance().amount();
            }
        }
    } else if (ends_with(type_url, "FundsWithdrawn")) {
        examples::FundsWithdrawn event;
        if (event_any.UnpackTo(&event)) {
            if (event.has_new_balance()) {
                state.bankroll = event.new_balance().amount();
            }
        }
    } else if (ends_with(type_url, "FundsReserved")) {
        examples::FundsReserved event;
        if (event_any.UnpackTo(&event)) {
            if (event.has_new_reserved_balance()) {
                state.reserved_funds = event.new_reserved_balance().amount();
            }
            std::string table_key = bytes_to_hex(event.table_root());
            if (event.has_amount()) {
                state.table_reservations[table_key] = event.amount().amount();
            }
        }
    } else if (ends_with(type_url, "FundsReleased")) {
        examples::FundsReleased event;
        if (event_any.UnpackTo(&event)) {
            if (event.has_new_reserved_balance()) {
                state.reserved_funds = event.new_reserved_balance().amount();
            }
            std::string table_key = bytes_to_hex(event.table_root());
            state.table_reservations.erase(table_key);
        }
    } else if (ends_with(type_url, "FundsTransferred")) {
        examples::FundsTransferred event;
        if (event_any.UnpackTo(&event)) {
            if (event.has_new_balance()) {
                state.bankroll = event.new_balance().amount();
            }
        }
    }
}

} // namespace player
