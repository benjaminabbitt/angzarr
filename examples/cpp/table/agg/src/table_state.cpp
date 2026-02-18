#include "table_state.hpp"
#include <algorithm>
#include <vector>

namespace table {

int TableState::active_player_count() const {
    return static_cast<int>(std::count_if(seats.begin(), seats.end(),
        [](const auto& p) { return !p.second.is_sitting_out; }));
}

TableState TableState::from_event_book(const angzarr::EventBook& event_book) {
    TableState state;
    for (const auto& page : event_book.pages()) {
        apply_event(state, page.event());
    }
    return state;
}

void TableState::apply_event(TableState& state, const google::protobuf::Any& event_any) {
    const std::string& type_url = event_any.type_url();

    if (type_url.find("TableCreated") != std::string::npos) {
        examples::TableCreated event;
        if (event_any.UnpackTo(&event)) {
            state.table_id = "table_" + event.table_name();
            state.table_name = event.table_name();
            state.game_variant = event.game_variant();
            state.small_blind = event.small_blind();
            state.big_blind = event.big_blind();
            state.min_buy_in = event.min_buy_in();
            state.max_buy_in = event.max_buy_in();
            state.max_players = event.max_players();
            state.action_timeout_seconds = event.action_timeout_seconds();
            state.status = "waiting";
        }
    } else if (type_url.find("PlayerJoined") != std::string::npos) {
        examples::PlayerJoined event;
        if (event_any.UnpackTo(&event)) {
            state.seats[event.seat_position()] = SeatState{
                event.seat_position(),
                event.player_root(),
                event.stack(),
                true,
                false
            };
        }
    } else if (type_url.find("PlayerLeft") != std::string::npos) {
        examples::PlayerLeft event;
        if (event_any.UnpackTo(&event)) {
            state.seats.erase(event.seat_position());
        }
    } else if (type_url.find("PlayerSatOut") != std::string::npos) {
        examples::PlayerSatOut event;
        if (event_any.UnpackTo(&event)) {
            for (auto& [pos, seat] : state.seats) {
                if (seat.player_root == event.player_root()) {
                    seat.is_sitting_out = true;
                    break;
                }
            }
        }
    } else if (type_url.find("PlayerSatIn") != std::string::npos) {
        examples::PlayerSatIn event;
        if (event_any.UnpackTo(&event)) {
            for (auto& [pos, seat] : state.seats) {
                if (seat.player_root == event.player_root()) {
                    seat.is_sitting_out = false;
                    break;
                }
            }
        }
    } else if (type_url.find("HandStarted") != std::string::npos) {
        examples::HandStarted event;
        if (event_any.UnpackTo(&event)) {
            state.hand_count = event.hand_number();
            state.current_hand_root = event.hand_root();
            state.dealer_position = event.dealer_position();
            state.status = "in_hand";
        }
    } else if (type_url.find("HandEnded") != std::string::npos) {
        examples::HandEnded event;
        if (event_any.UnpackTo(&event)) {
            state.current_hand_root.clear();
            state.status = "waiting";
            // Apply stack changes
            for (const auto& [player_hex, delta] : event.stack_changes()) {
                for (auto& [pos, seat] : state.seats) {
                    std::string seat_hex;
                    for (unsigned char c : seat.player_root) {
                        char buf[3];
                        snprintf(buf, sizeof(buf), "%02x", c);
                        seat_hex += buf;
                    }
                    if (seat_hex == player_hex) {
                        seat.stack += delta;
                        break;
                    }
                }
            }
        }
    } else if (type_url.find("ChipsAdded") != std::string::npos) {
        examples::ChipsAdded event;
        if (event_any.UnpackTo(&event)) {
            for (auto& [pos, seat] : state.seats) {
                if (seat.player_root == event.player_root()) {
                    seat.stack = event.new_stack();
                    break;
                }
            }
        }
    }
}

const SeatState* TableState::get_seat(int position) const {
    auto it = seats.find(position);
    return it != seats.end() ? &it->second : nullptr;
}

const SeatState* TableState::find_player_seat(const std::string& player_root) const {
    for (const auto& [pos, seat] : seats) {
        if (seat.player_root == player_root) {
            return &seat;
        }
    }
    return nullptr;
}

int TableState::find_available_seat(int preferred) const {
    // preferred > 0 means explicit seat preference
    if (preferred > 0 && preferred < max_players) {
        if (seats.find(preferred) == seats.end()) {
            return preferred;
        }
    }
    for (int pos = 0; pos < max_players; ++pos) {
        if (seats.find(pos) == seats.end()) {
            return pos;
        }
    }
    return -1;
}

int TableState::next_dealer_position() const {
    if (seats.empty()) {
        return 0;
    }
    std::vector<int> positions;
    for (const auto& [pos, seat] : seats) {
        positions.push_back(pos);
    }
    std::sort(positions.begin(), positions.end());

    int current_idx = 0;
    for (size_t i = 0; i < positions.size(); ++i) {
        if (positions[i] == dealer_position) {
            current_idx = static_cast<int>(i);
            break;
        }
    }
    int next_idx = (current_idx + 1) % static_cast<int>(positions.size());
    return positions[next_idx];
}

} // namespace table
