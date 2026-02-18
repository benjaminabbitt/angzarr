#pragma once

#include <functional>
#include <map>
#include <string>
#include <vector>
#include <google/protobuf/any.pb.h>
#include "types.pb.h"
#include "aggregate.pb.h"
#include "helpers.hpp"
#include "errors.hpp"

namespace angzarr {

/**
 * Component type constants for descriptors.
 */
namespace component_types {
    constexpr const char* AGGREGATE = "aggregate";
    constexpr const char* SAGA = "saga";
    constexpr const char* PROCESS_MANAGER = "process_manager";
    constexpr const char* PROJECTOR = "projector";
    constexpr const char* UPCASTER = "upcaster";
}

/**
 * Describes what a component subscribes to or sends to.
 */
struct TargetDesc {
    std::string domain;
    std::vector<std::string> types;
};

/**
 * Describes a component for topology discovery.
 */
struct Descriptor {
    std::string name;
    std::string component_type;
    std::vector<TargetDesc> inputs;
};

/**
 * DRY command dispatcher for aggregates (functional pattern).
 */
template<typename State>
class CommandRouter {
public:
    using CommandHandler = std::function<EventBook(
        const CommandBook&, const google::protobuf::Any&, State&, int)>;
    using RejectionHandler = std::function<EventBook(const Notification&, State&)>;
    using StateRebuilder = std::function<State(const EventBook*)>;

    explicit CommandRouter(const std::string& domain, StateRebuilder rebuild = nullptr)
        : domain_(domain), rebuild_(std::move(rebuild)) {}

    /**
     * Register a handler for a command type_url suffix.
     */
    CommandRouter& on(const std::string& suffix, CommandHandler handler) {
        handlers_.emplace_back(suffix, std::move(handler));
        return *this;
    }

    /**
     * Register a rejection handler.
     */
    CommandRouter& on_rejected(const std::string& domain, const std::string& command,
                               RejectionHandler handler) {
        rejection_handlers_[domain + "/" + command] = std::move(handler);
        return *this;
    }

    /**
     * Dispatch a ContextualCommand to the matching handler.
     */
    BusinessResponse dispatch(const ContextualCommand& cmd) {
        const auto& command_book = cmd.command();
        const auto* prior_events = cmd.has_events() ? &cmd.events() : nullptr;

        auto state = get_state(prior_events);
        int seq = helpers::next_sequence(prior_events);

        if (command_book.pages_size() == 0) {
            throw InvalidArgumentError("No command pages");
        }

        const auto& command_any = command_book.pages(0).command();
        if (command_any.type_url().empty()) {
            throw InvalidArgumentError("No command pages");
        }

        const auto& type_url = command_any.type_url();

        // Check for Notification
        if (helpers::type_url_matches(type_url, "Notification")) {
            Notification notification;
            command_any.UnpackTo(&notification);
            return dispatch_rejection(notification, state);
        }

        // Normal command dispatch
        for (const auto& [suffix, handler] : handlers_) {
            if (helpers::type_url_matches(type_url, suffix)) {
                auto events = handler(command_book, command_any, state, seq);
                BusinessResponse response;
                *response.mutable_events() = std::move(events);
                return response;
            }
        }

        throw InvalidArgumentError("Unknown command type: " + type_url);
    }

    /**
     * Build a component descriptor.
     */
    Descriptor descriptor() const {
        return {domain_, component_types::AGGREGATE, {{domain_, types()}}};
    }

    /**
     * Return registered command type suffixes.
     */
    std::vector<std::string> types() const {
        std::vector<std::string> result;
        for (const auto& [suffix, _] : handlers_) {
            result.push_back(suffix);
        }
        return result;
    }

private:
    State get_state(const EventBook* event_book) {
        if (rebuild_) {
            return rebuild_(event_book);
        }
        throw std::runtime_error("CommandRouter requires rebuild function");
    }

    BusinessResponse dispatch_rejection(const Notification& notification, State& state) {
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

        for (const auto& [key, handler] : rejection_handlers_) {
            auto pos = key.find('/');
            auto expected_domain = key.substr(0, pos);
            auto expected_command = key.substr(pos + 1);
            if (domain == expected_domain &&
                helpers::type_url_matches(command_suffix, expected_command)) {
                auto events = handler(notification, state);
                BusinessResponse response;
                *response.mutable_events() = std::move(events);
                return response;
            }
        }

        BusinessResponse response;
        auto* revocation = response.mutable_revocation();
        revocation->set_emit_system_revocation(true);
        revocation->set_reason("Aggregate " + domain_ + " has no custom compensation for " +
                               domain + "/" + command_suffix);
        return response;
    }

    std::string domain_;
    StateRebuilder rebuild_;
    std::vector<std::pair<std::string, CommandHandler>> handlers_;
    std::map<std::string, RejectionHandler> rejection_handlers_;
};

/**
 * DRY event dispatcher for sagas (functional pattern).
 */
class EventRouter {
public:
    using EventHandler = std::function<std::vector<CommandBook>(
        const google::protobuf::Any&, const std::string&, const std::string&,
        const std::vector<EventBook>&)>;
    using PrepareHandler = std::function<std::vector<Cover>(
        const google::protobuf::Any&, const UUID*)>;

    EventRouter(const std::string& name, const std::string& input_domain)
        : name_(name), input_domain_(input_domain) {}

    /**
     * Declare an output domain and command type.
     */
    EventRouter& sends(const std::string& domain, const std::string& command_type) {
        output_targets_[domain].push_back(command_type);
        return *this;
    }

    /**
     * Register a prepare handler.
     */
    EventRouter& prepare(const std::string& suffix, PrepareHandler handler) {
        prepare_handlers_[suffix] = std::move(handler);
        return *this;
    }

    /**
     * Register an event handler.
     */
    EventRouter& on(const std::string& suffix, EventHandler handler) {
        handlers_.emplace_back(suffix, std::move(handler));
        return *this;
    }

    /**
     * Get destinations needed for source events.
     */
    std::vector<Cover> prepare_destinations(const EventBook& book) {
        const UUID* root = book.has_cover() && book.cover().has_root()
            ? &book.cover().root() : nullptr;
        std::vector<Cover> destinations;

        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;
            for (const auto& [suffix, handler] : prepare_handlers_) {
                if (helpers::type_url_matches(page.event().type_url(), suffix)) {
                    auto covers = handler(page.event(), root);
                    destinations.insert(destinations.end(), covers.begin(), covers.end());
                    break;
                }
            }
        }
        return destinations;
    }

    /**
     * Dispatch all events to handlers.
     */
    std::vector<CommandBook> dispatch(const EventBook& book,
                                       const std::vector<EventBook>& destinations = {}) {
        std::string root_str;
        if (book.has_cover() && book.cover().has_root()) {
            root_str = book.cover().root().value();
        }
        auto correlation_id = book.has_cover() ? book.cover().correlation_id() : "";

        std::vector<CommandBook> commands;
        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;
            for (const auto& [suffix, handler] : handlers_) {
                if (helpers::type_url_matches(page.event().type_url(), suffix)) {
                    auto cmds = handler(page.event(), root_str, correlation_id, destinations);
                    commands.insert(commands.end(), cmds.begin(), cmds.end());
                    break;
                }
            }
        }
        return commands;
    }

    /**
     * Build a component descriptor.
     */
    Descriptor descriptor() const {
        return {name_, component_types::SAGA, {{input_domain_, types()}}};
    }

    /**
     * Return registered event type suffixes.
     */
    std::vector<std::string> types() const {
        std::vector<std::string> result;
        for (const auto& [suffix, _] : handlers_) {
            result.push_back(suffix);
        }
        return result;
    }

private:
    std::string name_;
    std::string input_domain_;
    std::map<std::string, std::vector<std::string>> output_targets_;
    std::vector<std::pair<std::string, EventHandler>> handlers_;
    std::map<std::string, PrepareHandler> prepare_handlers_;
};

/**
 * Fluent state reconstruction from events (functional pattern).
 */
template<typename State>
class StateRouter {
public:
    using Applier = std::function<void(State&, const google::protobuf::Any&)>;

    explicit StateRouter(std::function<State()> factory)
        : factory_(std::move(factory)) {}

    /**
     * Register an event applier.
     */
    template<typename Event>
    StateRouter& on(std::function<void(State&, const Event&)> applier) {
        std::string suffix = Event::descriptor()->name();
        appliers_[suffix] = [applier](State& state, const google::protobuf::Any& any) {
            Event event;
            any.UnpackTo(&event);
            applier(state, event);
        };
        return *this;
    }

    /**
     * Rebuild state from an EventBook.
     */
    State with_event_book(const EventBook* book) {
        auto state = factory_();
        if (!book) return state;

        for (const auto& page : book->pages()) {
            if (!page.has_event()) continue;
            apply_event(state, page.event());
        }
        return state;
    }

private:
    void apply_event(State& state, const google::protobuf::Any& event_any) {
        for (const auto& [suffix, applier] : appliers_) {
            if (helpers::type_url_matches(event_any.type_url(), suffix)) {
                applier(state, event_any);
                return;
            }
        }
    }

    std::function<State()> factory_;
    std::map<std::string, Applier> appliers_;
};

} // namespace angzarr
