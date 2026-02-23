#pragma once

#include "examples/table.pb.h"
#include "table_state.hpp"

namespace table {
namespace handlers {

/// Handle LeaveTable command.
examples::PlayerLeft handle_leave(const examples::LeaveTable& cmd, const TableState& state);

}  // namespace handlers
}  // namespace table
