#pragma once

#include "angzarr/router.hpp"
#include "angzarr/types.pb.h"
#include "examples/player.pb.h"
#include "examples/table.pb.h"

namespace player {
namespace saga {

/// Create the player-table saga event router.
/// Reacts to PlayerSittingOut and PlayerReturningToPlay events from Player domain.
/// Emits PlayerSatOut and PlayerSatIn facts to Table domain.
angzarr::EventRouter create_player_table_router();

/// Set source root for handler access (call before processing).
void set_source_root(const angzarr::EventBook* source);

/// Get accumulated facts after dispatch.
std::vector<angzarr::EventBook> get_emitted_facts();

/// Clear accumulated facts.
void clear_emitted_facts();

}  // namespace saga
}  // namespace player
