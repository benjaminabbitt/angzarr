#pragma once

#include <functional>
#include <map>
#include <string>
#include <vector>
#include <google/protobuf/any.pb.h>
#include "types.pb.h"
#include "saga.pb.h"
#include "helpers.hpp"
#include "errors.hpp"
#include "router.hpp"

namespace angzarr {

/**
 * Base class for process managers using macro-based handler registration.
 *
 * Process managers are stateful coordinators that accept events from multiple
 * domains and emit commands. They use correlation IDs as aggregate roots.
 *
 * Usage:
 *   class HandFlowPM : public ProcessManager<HandFlowState> {
 *   public:
 *       ANGZARR_PROCESS_MANAGER("hand-flow")
 *
 *       ANGZARR_REACTS_TO(HandStarted)
 *       std::vector<CommandBook> handle_HandStarted(const HandStarted& event) {
 *           // Use state() to access current PM state
 *           // Return commands to emit
 *       }
 *
 *       ANGZARR_APPLIES(HandStarted)
 *       void apply_HandStarted(HandFlowState& state, const HandStarted& event) {
 *           state.set_phase("started");
 *       }
 *
 *       ANGZARR_REJECTED("player", DeductBuyIn)
 *       EventBook handle_rejected_DeductBuyIn(const Notification& notification) {
 *           // Handle compensation
 *       }
 *
 *   protected:
 *       HandFlowState create_empty_state() override { return HandFlowState{}; }
 *   };
 */
template<typename StateT>
class ProcessManager {
public:
    using State = StateT;
    using EventDispatcher = std::function<std::vector<CommandBook>(
        ProcessManager*, const google::protobuf::Any&, const std::string&)>;
    using EventApplier = std::function<void(
        ProcessManager*, StateT&, const google::protobuf::Any&)>;
    using RejectionHandler = std::function<EventBook(
        ProcessManager*, const Notification&, StateT&)>;

    virtual ~ProcessManager() = default;

    /**
     * Get the process manager name.
     */
    virtual std::string name() const = 0;

    /**
     * Dispatch an event to the appropriate handler.
     */
    std::vector<CommandBook> dispatch(const EventBook& book,
                                       const EventBook* prior_events = nullptr) {
        rebuild_state(prior_events);

        auto correlation_id = book.has_cover() ? book.cover().correlation_id() : "";
        if (correlation_id.empty()) {
            // PMs require correlation ID
            return {};
        }

        std::vector<CommandBook> commands;
        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;

            // Check for rejection notification
            if (helpers::type_url_matches(page.event().type_url(), "Notification")) {
                Notification notification;
                page.event().UnpackTo(&notification);
                auto events = dispatch_rejection(notification);
                // Note: PM rejection returns events, not commands
                // This would need additional handling in the framework
                continue;
            }

            auto suffix = helpers::type_name_from_url(page.event().type_url());

            // Apply event to state
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

    /**
     * Build a component descriptor.
     */
    Descriptor descriptor() const {
        std::vector<std::string> types;
        for (const auto& [suffix, _] : handlers()) {
            types.push_back(suffix);
        }
        // PM subscribes to multiple domains - would need additional config
        return {name(), component_types::PROCESS_MANAGER, {}};
    }

    /**
     * Check if the PM exists (has prior events).
     */
    bool exists() const { return exists_; }

    /**
     * Get the current state.
     */
    const StateT& state() const { return state_; }

    /**
     * Get mutable state.
     */
    StateT& mutable_state() { return state_; }

protected:
    /**
     * Create an empty state instance.
     */
    virtual StateT create_empty_state() = 0;

    /**
     * Pack commands for output.
     */
    template<typename T>
    std::vector<CommandBook> pack_commands(const T& command, const std::string& domain,
                                           const std::string& correlation_id) {
        CommandBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain(domain);
        cover->set_correlation_id(correlation_id);

        auto* page = book.add_pages();
        page->mutable_command()->PackFrom(command, "type.googleapis.com/");

        return {book};
    }

    /**
     * Register an event handler.
     */
    static void register_event_handler(const std::string& suffix, EventDispatcher dispatcher) {
        handlers()[suffix] = std::move(dispatcher);
    }

    /**
     * Register an event applier.
     */
    static void register_applier(const std::string& suffix, EventApplier applier) {
        appliers()[suffix] = std::move(applier);
    }

    /**
     * Register a rejection handler.
     */
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

    EventBook dispatch_rejection(const Notification& notification) {
        std::string domain;
        std::string command_suffix;

        if (notification.has_payload()) {
            RejectionNotification rejection;
            if (notification.payload().UnpackTo(&rejection)) {
                if (rejection.has_rejected_command() &&
                    rejection.rejected_command().pages_size() > 0) {
                    const auto& rejected_cmd = rejection.rejected_command();
                    domain = rejected_cmd.cover().domain();
                    command_suffix = helpers::type_name_from_url(
                        rejected_cmd.pages(0).command().type_url());
                }
            }
        }

        auto key = domain + "/" + command_suffix;
        auto it = rejection_handlers().find(key);
        if (it != rejection_handlers().end()) {
            return it->second(this, notification, state_);
        }

        return EventBook{};
    }

    static std::map<std::string, EventDispatcher>& handlers() {
        static std::map<std::string, EventDispatcher> h;
        return h;
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

/**
 * Macro to declare a process manager.
 */
#define ANGZARR_PROCESS_MANAGER(pm_name) \
    static constexpr const char* kName = pm_name; \
    std::string name() const override { return kName; }

} // namespace angzarr
