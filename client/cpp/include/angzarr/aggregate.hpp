#pragma once

#include <functional>
#include <map>
#include <string>
#include <vector>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/aggregate.pb.h"
#include "helpers.hpp"
#include "errors.hpp"
#include "router.hpp"
#include "descriptor.hpp"

namespace angzarr {

/**
 * Base class for aggregates using macro-based handler registration.
 *
 * Usage:
 *   class Player : public Aggregate<PlayerState> {
 *   public:
 *       ANGZARR_AGGREGATE("player")
 *
 *       ANGZARR_HANDLES(RegisterPlayer)
 *       PlayerRegistered handle_RegisterPlayer(const RegisterPlayer& cmd) {
 *           validation::require_not_exists(exists(), "Player already exists");
 *           return PlayerRegistered{};
 *       }
 *
 *       ANGZARR_APPLIES(PlayerRegistered)
 *       void apply_PlayerRegistered(PlayerState& state, const PlayerRegistered& event) {
 *           state.set_status("active");
 *       }
 *
 *   protected:
 *       PlayerState create_empty_state() override { return PlayerState{}; }
 *   };
 */
template<typename StateT>
class Aggregate {
public:
    using State = StateT;
    using CommandDispatcher = std::function<EventBook(Aggregate*, const google::protobuf::Any&, int)>;
    using EventApplier = std::function<void(Aggregate*, StateT&, const google::protobuf::Any&)>;
    using RejectionHandler = std::function<EventBook(Aggregate*, const Notification&, StateT&)>;

    virtual ~Aggregate() = default;

    /**
     * Get the domain name for this aggregate.
     */
    virtual std::string domain() const = 0;

    /**
     * Dispatch a ContextualCommand to the appropriate handler.
     */
    BusinessResponse dispatch(const ContextualCommand& cmd) {
        const auto& command_book = cmd.command();
        const auto* prior_events = cmd.has_events() ? &cmd.events() : nullptr;

        rebuild_state(prior_events);
        int seq = helpers::next_sequence(prior_events);

        if (command_book.pages_size() == 0) {
            throw InvalidArgumentError("No command pages");
        }

        const auto& command_any = command_book.pages(0).command();
        if (command_any.type_url().empty()) {
            throw InvalidArgumentError("No command pages");
        }

        const auto& type_url = command_any.type_url();

        // Check for Notification (rejection)
        if (helpers::type_url_matches(type_url, "Notification")) {
            Notification notification;
            command_any.UnpackTo(&notification);
            return dispatch_rejection(notification);
        }

        // Normal command dispatch
        auto suffix = helpers::type_name_from_url(type_url);
        auto it = handlers().find(suffix);
        if (it != handlers().end()) {
            auto events = it->second(this, command_any, seq);
            BusinessResponse response;
            *response.mutable_events() = std::move(events);
            return response;
        }

        throw InvalidArgumentError("Unknown command type: " + type_url);
    }

    /**
     * Build a component descriptor for this aggregate.
     */
    Descriptor descriptor() const {
        std::vector<std::string> types;
        for (const auto& [suffix, _] : handlers()) {
            types.push_back(suffix);
        }
        return {domain(), component_types::AGGREGATE, {{domain(), types}}};
    }

    /**
     * Check if the aggregate exists (has prior events).
     */
    bool exists() const { return exists_; }

    /**
     * Get the current state.
     */
    const StateT& state() const { return state_; }

    /**
     * Get mutable state (for appliers).
     */
    StateT& mutable_state() { return state_; }

protected:
    /**
     * Create an empty state instance.
     */
    virtual StateT create_empty_state() = 0;

    /**
     * Register a command handler (called by ANGZARR_HANDLES macro).
     */
    static void register_handler(const std::string& suffix, CommandDispatcher dispatcher) {
        handlers()[suffix] = std::move(dispatcher);
    }

    /**
     * Register an event applier (called by ANGZARR_APPLIES macro).
     */
    static void register_applier(const std::string& suffix, EventApplier applier) {
        appliers()[suffix] = std::move(applier);
    }

    /**
     * Register a rejection handler (called by ANGZARR_REJECTED macro).
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
            apply_event(page.event());
            exists_ = true;
        }
    }

    void apply_event(const google::protobuf::Any& event_any) {
        auto suffix = helpers::type_name_from_url(event_any.type_url());
        auto it = appliers().find(suffix);
        if (it != appliers().end()) {
            it->second(this, state_, event_any);
        }
    }

    BusinessResponse dispatch_rejection(const Notification& notification) {
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
            auto events = it->second(this, notification, state_);
            BusinessResponse response;
            *response.mutable_events() = std::move(events);
            return response;
        }

        // Default: emit system revocation
        BusinessResponse response;
        auto* revocation = response.mutable_revocation();
        revocation->set_emit_system_revocation(true);
        revocation->set_reason("Aggregate " + this->domain() +
                               " has no custom compensation for " + key);
        return response;
    }

    // Static registries per derived type
    static std::map<std::string, CommandDispatcher>& handlers() {
        static std::map<std::string, CommandDispatcher> h;
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

} // namespace angzarr
