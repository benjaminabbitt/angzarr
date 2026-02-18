#pragma once

#include "table_state.hpp"
#include "examples/table.pb.h"

namespace table {
namespace handlers {

/// Handle LeaveTable command.
examples::PlayerLeft handle_leave(
    const examples::LeaveTable& cmd,
    const TableState& state);

} // namespace handlers
} // namespace table
