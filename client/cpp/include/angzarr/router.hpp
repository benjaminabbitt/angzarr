#pragma once

#include <functional>
#include <map>
#include <optional>
#include <string>
#include <vector>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/aggregate.pb.h"
#include "helpers.hpp"
#include "errors.hpp"

namespace angzarr {

/**
 * Response from rejection handlers - can emit events AND/OR notification.
 */
struct RejectionHandlerResponse {
    /// Events to persist to own state (compensation).
    std::optional<EventBook> events;
    /// Notification to forward upstream (rejection propagation).
    std::optional<Notification> notification;
};

/**
 * DRY command dispatcher for aggregates (functional pattern).
 */
template<typename State>
class CommandRouter {
public:
    using CommandHandler = std::function<EventBook(
        const CommandBook&, const google::protobuf::Any&, State&, int)>;
    using RejectionHandler = std::function<RejectionHandlerResponse(const Notification&, State&)>;
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
                auto response = handler(notification, state);
                // Handle notification forwarding
                if (response.notification.has_value()) {
                    BusinessResponse biz_response;
                    *biz_response.mutable_notification() = std::move(*response.notification);
                    return biz_response;
                }
                // Handle compensation events
                if (response.events.has_value()) {
                    BusinessResponse biz_response;
                    *biz_response.mutable_events() = std::move(*response.events);
                    return biz_response;
                }
                // Handler returned empty response
                BusinessResponse biz_response;
                auto* revocation = biz_response.mutable_revocation();
                revocation->set_emit_system_revocation(false);
                revocation->set_reason("Aggregate " + domain_ + " handled rejection for " + key);
                return biz_response;
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
 * Unified event dispatcher for sagas, process managers, and projectors.
 * Uses fluent .domain().on() pattern to register handlers with domain context.
 *
 * Example (Saga - single domain):
 *   EventRouter router("saga-table-hand");
 *   router.domain("table").on("HandStarted", handle_started);
 *
 * Example (Process Manager - multi-domain):
 *   EventRouter router("pmg-order-flow");
 *   router.domain("order").on("OrderCreated", handle_created)
 *         .domain("inventory").on("StockReserved", handle_reserved);
 *
 * Example (Projector - multi-domain):
 *   EventRouter router("prj-output");
 *   router.domain("player").on("PlayerRegistered", handle_registered)
 *         .domain("hand").on("CardsDealt", handle_dealt);
 */
class EventRouter {
public:
    using EventHandler = std::function<std::vector<CommandBook>(
        const google::protobuf::Any&, const std::string&, const std::string&,
        const std::vector<EventBook>&)>;
    using PrepareHandler = std::function<std::vector<Cover>(
        const google::protobuf::Any&, const UUID*)>;

    explicit EventRouter(const std::string& name)
        : name_(name) {}

    /**
     * Create a new EventRouter with a single input domain (backwards compatibility).
     * @deprecated Use EventRouter(name).domain(input_domain) instead.
     */
    [[deprecated("Use EventRouter(name).domain(input_domain) instead")]]
    EventRouter(const std::string& name, const std::string& input_domain)
        : name_(name) {
        if (!input_domain.empty()) {
            domain(input_domain);
        }
    }

    /**
     * Set the current domain context for subsequent on() calls.
     */
    EventRouter& domain(const std::string& name) {
        current_domain_ = name;
        if (handlers_.find(name) == handlers_.end()) {
            handlers_[name] = {};
        }
        if (prepare_handlers_.find(name) == prepare_handlers_.end()) {
            prepare_handlers_[name] = {};
        }
        return *this;
    }

    /**
     * Register a prepare handler.
     * Must be called after domain() to set context.
     */
    EventRouter& prepare(const std::string& suffix, PrepareHandler handler) {
        if (current_domain_.empty()) {
            throw std::runtime_error("Must call domain() before prepare()");
        }
        prepare_handlers_[current_domain_][suffix] = std::move(handler);
        return *this;
    }

    /**
     * Register an event handler in current domain.
     * Must be called after domain() to set context.
     */
    EventRouter& on(const std::string& suffix, EventHandler handler) {
        if (current_domain_.empty()) {
            throw std::runtime_error("Must call domain() before on()");
        }
        handlers_[current_domain_].emplace_back(suffix, std::move(handler));
        return *this;
    }

    /**
     * Auto-derive subscriptions from registered handlers.
     * Returns map of domain to event types.
     */
    std::map<std::string, std::vector<std::string>> subscriptions() const {
        std::map<std::string, std::vector<std::string>> result;
        for (const auto& [domain, handlers] : handlers_) {
            if (!handlers.empty()) {
                std::vector<std::string> types;
                for (const auto& [suffix, _] : handlers) {
                    types.push_back(suffix);
                }
                result[domain] = types;
            }
        }
        return result;
    }

    /**
     * Get destinations needed for source events.
     * Routes based on source domain.
     */
    std::vector<Cover> prepare_destinations(const EventBook& book) {
        std::string source_domain;
        if (book.has_cover()) {
            source_domain = book.cover().domain();
        }

        auto it = prepare_handlers_.find(source_domain);
        if (it == prepare_handlers_.end()) {
            return {};
        }

        const UUID* root = book.has_cover() && book.cover().has_root()
            ? &book.cover().root() : nullptr;
        std::vector<Cover> destinations;

        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;
            for (const auto& [suffix, handler] : it->second) {
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
     * Routes based on source domain and event type suffix.
     */
    std::vector<CommandBook> dispatch(const EventBook& book,
                                       const std::vector<EventBook>& destinations = {}) {
        std::string source_domain;
        if (book.has_cover()) {
            source_domain = book.cover().domain();
        }

        auto it = handlers_.find(source_domain);
        if (it == handlers_.end()) {
            return {};
        }

        std::string root_str;
        if (book.has_cover() && book.cover().has_root()) {
            root_str = book.cover().root().value();
        }
        auto correlation_id = book.has_cover() ? book.cover().correlation_id() : "";

        std::vector<CommandBook> commands;
        for (const auto& page : book.pages()) {
            if (!page.has_event()) continue;
            for (const auto& [suffix, handler] : it->second) {
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
     * Return the first registered domain (for backwards compatibility).
     * @deprecated Use subscriptions() instead.
     */
    [[deprecated("Use subscriptions() instead")]]
    std::string input_domain() const {
        if (!handlers_.empty()) {
            return handlers_.begin()->first;
        }
        return "";
    }

    /**
     * Declare an output domain and command type (deprecated, no-op).
     * This method was used for topology discovery but is now deprecated.
     * @deprecated This method has no effect and will be removed.
     */
    [[deprecated("This method has no effect and will be removed")]]
    EventRouter& sends(const std::string& /*domain*/, const std::string& /*command_type*/) {
        // No-op for backwards compatibility
        return *this;
    }

    /**
     * Return output domain names (deprecated, returns empty vector).
     * @deprecated Output domains are no longer tracked.
     */
    [[deprecated("Output domains are no longer tracked")]]
    std::vector<std::string> output_domains() const {
        return {};
    }

    /**
     * Return command types for a given output domain (deprecated, returns empty vector).
     * @deprecated Output types are no longer tracked.
     */
    [[deprecated("Output types are no longer tracked")]]
    std::vector<std::string> output_types(const std::string& /*domain*/) const {
        return {};
    }

private:
    std::string name_;
    std::string current_domain_;
    std::map<std::string, std::vector<std::pair<std::string, EventHandler>>> handlers_;
    std::map<std::string, std::map<std::string, PrepareHandler>> prepare_handlers_;
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
