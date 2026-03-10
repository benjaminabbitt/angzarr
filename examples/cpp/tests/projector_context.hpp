#pragma once

#include <map>
#include <string>

namespace projector_context {

struct OutputProjectorState {
    std::map<std::string, std::string> player_names;
    bool show_timestamps = false;
    std::string last_output;
    bool is_active = false;

    void reset() {
        player_names.clear();
        show_timestamps = false;
        last_output.clear();
        is_active = false;
    }
};

// Global projector state - accessible from multiple step files
extern thread_local OutputProjectorState g_projector_state;

}  // namespace projector_context
