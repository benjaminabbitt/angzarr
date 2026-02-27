#pragma once

#include "angzarr/router.hpp"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/table.pb.h"

namespace hand {
namespace saga {

/// Create the hand-table saga event router.
/// Reacts to HandComplete events from Hand domain.
/// Sends EndHand commands to Table domain.
angzarr::EventRouter create_hand_table_router();

// Note: Handlers are internal to the router - only create_hand_table_router is public

}  // namespace saga
}  // namespace hand
