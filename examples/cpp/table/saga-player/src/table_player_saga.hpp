#pragma once

#include "angzarr/router.hpp"
#include "angzarr/types.pb.h"
#include "examples/player.pb.h"
#include "examples/table.pb.h"

namespace table {
namespace saga {

/// Create the table-player saga event router.
/// Reacts to HandEnded events from Table domain.
/// Sends ReleaseFunds commands to Player domain.
angzarr::EventRouter create_table_player_router();

// Note: Handlers are internal to the router - only create_table_player_router is public

}  // namespace saga
}  // namespace table
