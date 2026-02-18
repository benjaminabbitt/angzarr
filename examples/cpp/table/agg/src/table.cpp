#include "table.hpp"
#include "create_handler.hpp"
#include "join_handler.hpp"
#include "leave_handler.hpp"
#include "start_hand_handler.hpp"
#include "end_hand_handler.hpp"

namespace table {

examples::TableCreated Table::create(const examples::CreateTable& cmd) {
    return handlers::handle_create(cmd, state_);
}

examples::PlayerJoined Table::join(const examples::JoinTable& cmd) {
    return handlers::handle_join(cmd, state_);
}

examples::PlayerLeft Table::leave(const examples::LeaveTable& cmd) {
    return handlers::handle_leave(cmd, state_);
}

examples::HandStarted Table::start_hand(const examples::StartHand& cmd) {
    return handlers::handle_start_hand(cmd, state_);
}

examples::HandEnded Table::end_hand(const examples::EndHand& cmd) {
    return handlers::handle_end_hand(cmd, state_);
}

} // namespace table
