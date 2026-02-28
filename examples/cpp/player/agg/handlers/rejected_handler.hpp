#pragma once

#include "../src/player_state.hpp"
#include "angzarr/types.pb.h"
#include "examples/player.pb.h"

namespace player {
namespace handlers {

/**
 * Handle JoinTable rejection by releasing reserved funds.
 *
 * Called when the JoinTable command (issued by saga-player-table after
 * FundsReserved) is rejected by the Table aggregate.
 */
examples::FundsReleased handle_join_rejected(const angzarr::Notification& notification,
                                             const PlayerState& state);

}  // namespace handlers
}  // namespace player
