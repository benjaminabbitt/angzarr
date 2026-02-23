#pragma once

#include "examples/table.pb.h"
#include "table_state.hpp"

namespace table {
namespace handlers {

/// Handle JoinTable command.
examples::PlayerJoined handle_join(const examples::JoinTable& cmd, const TableState& state);

}  // namespace handlers
}  // namespace table
