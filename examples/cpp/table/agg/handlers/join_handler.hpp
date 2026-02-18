#pragma once

#include "table_state.hpp"
#include "examples/table.pb.h"

namespace table {
namespace handlers {

/// Handle JoinTable command.
examples::PlayerJoined handle_join(
    const examples::JoinTable& cmd,
    const TableState& state);

} // namespace handlers
} // namespace table
