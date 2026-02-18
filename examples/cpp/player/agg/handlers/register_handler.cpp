#include "register_handler.hpp"
#include "angzarr/errors.hpp"
#include <chrono>

namespace player {
namespace handlers {

examples::PlayerRegistered handle_register(const examples::RegisterPlayer& cmd, const PlayerState& state) {
    // Guard: preconditions on state
    if (state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Player already exists");
    }

    // Validate: command inputs
    if (cmd.display_name().empty()) {
        throw angzarr::CommandRejectedError::invalid_argument("display_name is required");
    }
    if (cmd.email().empty()) {
        throw angzarr::CommandRejectedError::invalid_argument("email is required");
    }

    // Compute: build event
    examples::PlayerRegistered event;
    event.set_display_name(cmd.display_name());
    event.set_email(cmd.email());
    event.set_player_type(cmd.player_type());
    event.set_ai_model_id(cmd.ai_model_id());

    // Set timestamp
    auto now = std::chrono::system_clock::now();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
    event.mutable_registered_at()->set_seconds(seconds);

    return event;
}

} // namespace handlers
} // namespace player
