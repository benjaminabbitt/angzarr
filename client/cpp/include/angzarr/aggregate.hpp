#pragma once

/**
 * @file aggregate.hpp
 * @brief Aggregate (command handler) support for C++ client.
 *
 * This header re-exports the CommandHandler infrastructure.
 * Aggregates in angzarr are command handlers that:
 * - Process commands and emit events
 * - Rebuild state from event history
 * - Handle rejection notifications for compensation
 *
 * Two patterns are available:
 *
 * 1. CRTP OO Pattern (Recommended):
 *    @code
 *    class Player : public CommandHandlerBase<PlayerState, Player> {
 *        Player(const EventBook* events = nullptr) {
 *            init(events, []() { return PlayerState{}; });
 *            set_domain("player");
 *            handles(&Player::register_player);
 *            applies(&Player::apply_registered);
 *        }
 *        PlayerRegistered register_player(const RegisterPlayer& cmd);
 *        void apply_registered(PlayerState& state, const PlayerRegistered& event);
 *    };
 *    @endcode
 *
 * 2. Functional Router Pattern:
 *    @code
 *    auto rebuild = [](const EventBook* events) { return rebuild_state(events); };
 *    CommandRouter<PlayerState> router("player", rebuild);
 *    router.on("RegisterPlayer", handle_register);
 *    @endcode
 */

#include "angzarr/command_handler.hpp"
#include "angzarr/handler_traits.hpp"
#include "angzarr/router.hpp"

namespace angzarr {

// Aggregate is an alias for command handler in CQRS/ES terminology.
// The CommandHandlerBase and CommandRouter classes provide aggregate functionality.

/**
 * @deprecated Use CommandHandlerBase<State, Derived> instead.
 */
template <typename StateT>
using Aggregate = CommandHandler<StateT>;

}  // namespace angzarr
