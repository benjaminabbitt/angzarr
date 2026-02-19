#pragma once

#include <functional>
#include <map>
#include <string>
#include <vector>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/saga.pb.h"
#include "macros.hpp"
#include "errors.hpp"

namespace angzarr {

/// Base class for process managers using macro-based handler registration.
///
/// Usage:
///   class HandFlowPM : public ProcessManager<HandFlowState> {
///   public:
///       ANGZARR_PROCESS_MANAGER("hand-flow")
///
///       ANGZARR_PREPARES(HandStarted)
///       std::vector<Cover> prepare_HandStarted(const HandStarted& event) {
///           Cover cover;
///           cover.set_domain("hand");
///           cover.mutable_root()->set_value(event.hand_root());
///           return {cover};
///       }
///
///       ANGZARR_REACTS_TO(HandStarted)
///       std::vector<CommandBook> handle_HandStarted(const HandStarted& event) {
///           return {};  // No commands for this event
///       }
///
///       ANGZARR_APPLIES(HandStarted)
///       void apply_HandStarted(PMState& state, const HandStarted& event) {
///           state.hand_in_progress = true;
///       }
///
///   protected:
///       PMState create_empty_state() override { return PMState{}; }
///   };
template<typename StateT>
class ProcessManager {
public:
    using State = StateT;
    using EventDispatcher = std::function<std::vector<CommandBook>(
        ProcessManager*, const google::protobuf::Any&, const std::string&)>;
    using PrepareDispatcher = std::function<std::vector<Cover>(
        ProcessManager*, const google::protobuf::Any&)>;
    using EventApplier = std::function<void(
        ProcessManager*, StateT&, const google::protobuf::Any&)>;
    using RejectionHandler = std::function<EventBook(
        ProcessManager*, const Notification&, StateT&)>;

    virtual ~ProcessManager() = default;

    /// Get the process manager name.
    virtual std::string name() const = 0;

    /// Get input domains this PM subscribes to.
    virtual std::vector<std::string> input_domains() const {
        // Override in subclass to specify multiple input domains
        return {};
    }

    /// Prepare destinations for events (two-phase protocol).
    std::vector<Cover> prepare_destinations(const EventBook& book) {
        std::vector<Cover> destinations;

        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;

            auto suffix = helpers::type_name_from_url(page.event().type_url());
            auto it = prepare_handlers().find(suffix);
            if (it != prepare_handlers().end()) {
                auto covers = it->second(this, page.event());
                destinations.insert(destinations.end(), covers.begin(), covers.end());
            }
        }
        return destinations;
    }

    /// Dispatch events to handlers.
    std::vector<CommandBook> dispatch(const EventBook& book,
                                       const EventBook* prior_events = nullptr,
                                       const std::vector<EventBook>& destinations = {}) {
        rebuild_state(prior_events);

        auto correlation_id = book.has_cover() ? book.cover().correlation_id() : "";
        if (correlation_id.empty()) {
            // PMs require correlation ID
            return {};
        }

        std::vector<CommandBook> commands;
        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;

            auto suffix = helpers::type_name_from_url(page.event().type_url());

            // Apply event to state first
            auto applier_it = appliers().find(suffix);
            if (applier_it != appliers().end()) {
                applier_it->second(this, state_, page.event());
            }

            // Dispatch to handler
            auto it = handlers().find(suffix);
            if (it != handlers().end()) {
                auto cmds = it->second(this, page.event(), correlation_id);
                commands.insert(commands.end(), cmds.begin(), cmds.end());
            }
        }
        return commands;
    }

    /// Check if the PM exists (has prior events).
    bool exists() const { return exists_; }

    /// Get the current state.
    const StateT& state() const { return state_; }

    /// Get mutable state.
    StateT& mutable_state() { return state_; }

protected:
    /// Create an empty state instance.
    virtual StateT create_empty_state() = 0;

    /// Pack commands for output (single command returning CommandBooks).
    std::vector<CommandBook> pack_commands(const std::vector<CommandBook>& commands,
                                           const std::string& /* correlation_id */) {
        return commands;  // Already packed
    }

    /// Pack commands for output (returns empty for void handlers).
    std::vector<CommandBook> pack_commands(std::nullptr_t, const std::string&) {
        return {};
    }

    /// Register an event handler.
    static void register_event_handler(const std::string& suffix, EventDispatcher dispatcher) {
        handlers()[suffix] = std::move(dispatcher);
    }

    /// Register a prepare handler.
    static void register_prepare_handler(const std::string& suffix, PrepareDispatcher dispatcher) {
        prepare_handlers()[suffix] = std::move(dispatcher);
    }

    /// Register an event applier.
    static void register_applier(const std::string& suffix, EventApplier applier) {
        appliers()[suffix] = std::move(applier);
    }

    /// Register a rejection handler.
    static void register_rejection_handler(const std::string& key, RejectionHandler handler) {
        rejection_handlers()[key] = std::move(handler);
    }

private:
    void rebuild_state(const EventBook* event_book) {
        state_ = create_empty_state();
        exists_ = false;

        if (!event_book) return;

        for (const auto& page : event_book->pages()) {
            if (!page.has_event()) continue;

            auto suffix = helpers::type_name_from_url(page.event().type_url());
            auto it = appliers().find(suffix);
            if (it != appliers().end()) {
                it->second(this, state_, page.event());
                exists_ = true;
            }
        }
    }

    static std::map<std::string, EventDispatcher>& handlers() {
        static std::map<std::string, EventDispatcher> h;
        return h;
    }

    static std::map<std::string, PrepareDispatcher>& prepare_handlers() {
        static std::map<std::string, PrepareDispatcher> p;
        return p;
    }

    static std::map<std::string, EventApplier>& appliers() {
        static std::map<std::string, EventApplier> a;
        return a;
    }

    static std::map<std::string, RejectionHandler>& rejection_handlers() {
        static std::map<std::string, RejectionHandler> r;
        return r;
    }

    StateT state_;
    bool exists_ = false;
};

} // namespace angzarr
