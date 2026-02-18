#pragma once

#include <functional>
#include <string>
#include "angzarr/types.pb.h"
#include "text_renderer.hpp"

namespace projector {

/// Projector that subscribes to events from all domains and outputs text.
class OutputProjector {
public:
    using OutputFn = std::function<void(const std::string&)>;

    explicit OutputProjector(OutputFn output_fn, bool show_timestamps = false);

    /// Set display name for a player.
    void set_player_name(const std::string& player_root, const std::string& name);

    /// Handle a single event page from any domain.
    void handle_event(const angzarr::EventPage& event_page);

    /// Handle all events in an event book.
    void handle_event_book(const angzarr::EventBook& event_book);

private:
    TextRenderer renderer_;
    OutputFn output_fn_;
    bool show_timestamps_;
};

} // namespace projector
