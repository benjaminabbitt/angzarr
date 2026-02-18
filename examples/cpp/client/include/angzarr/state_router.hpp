#pragma once

#include <functional>
#include <string>
#include <vector>
#include <google/protobuf/message.h>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"

namespace angzarr {

/// Reusable state reconstruction from EventBook.
/// Registers event appliers and applies them to reconstruct state.
template<typename State>
class StateRouter {
public:
    using StateFactory = std::function<State()>;
    using EventApplier = std::function<void(State&, const google::protobuf::Any&)>;

private:
    struct ApplierEntry {
        std::string suffix;
        EventApplier applier;
    };

    StateFactory state_factory_;
    std::vector<ApplierEntry> appliers_;

public:
    explicit StateRouter(StateFactory factory = []() { return State{}; })
        : state_factory_(std::move(factory)) {}

    /// Register an event applier using suffix matching (forward compatible).
    template<typename EventType>
    StateRouter& on(std::function<void(State&, const EventType&)> applier) {
        const std::string full_name = EventType::descriptor()->full_name();
        // Extract suffix (e.g., "examples.PlayerRegistered" -> "PlayerRegistered")
        size_t pos = full_name.rfind('.');
        std::string suffix = (pos != std::string::npos) ? full_name.substr(pos + 1) : full_name;

        appliers_.push_back({
            suffix,
            [applier, full_name](State& state, const google::protobuf::Any& event_any) {
                EventType event;
                if (event_any.UnpackTo(&event)) {
                    applier(state, event);
                }
            }
        });
        return *this;
    }

    /// Build state from an EventBook by applying all events.
    State from_event_book(const EventBook& event_book) {
        State state = state_factory_();
        for (const auto& page : event_book.pages()) {
            apply_single(state, page.event());
        }
        return state;
    }

    /// Apply a single event to state.
    void apply_single(State& state, const google::protobuf::Any& event_any) {
        const std::string& type_url = event_any.type_url();
        for (const auto& entry : appliers_) {
            // Suffix matching for forward compatibility
            if (type_url.size() >= entry.suffix.size() &&
                type_url.compare(type_url.size() - entry.suffix.size(),
                                 entry.suffix.size(), entry.suffix) == 0) {
                entry.applier(state, event_any);
                return;
            }
        }
        // Unknown event type - silently ignore for forward compatibility
    }

    /// Get a state builder function suitable for CommandRouter.
    std::function<State(const EventBook&)> state_builder() {
        return [this](const EventBook& eb) { return from_event_book(eb); };
    }
};

} // namespace angzarr
