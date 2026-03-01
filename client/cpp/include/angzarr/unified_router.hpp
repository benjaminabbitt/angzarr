#pragma once

#include <google/protobuf/any.pb.h>

#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "angzarr/command_handler.pb.h"
#include "angzarr/process_manager.pb.h"
#include "angzarr/saga.pb.h"
#include "angzarr/types.pb.h"
#include "descriptor.hpp"
#include "errors.hpp"
#include "handler_traits.hpp"
#include "helpers.hpp"

namespace angzarr {

// ============================================================================
// CommandHandler Router
// ============================================================================

/**
 * Router for command handler components (commands -> events, single domain).
 *
 * Domain is set at construction time. No `.domain()` method exists,
 * enforcing single-domain constraint at compile time.
 *
 * @tparam S The state type
 * @tparam H The handler type (must satisfy CommandHandlerDomainHandler<S>)
 *
 * Example:
 * @code
 *   CommandHandlerRouter<PlayerState, PlayerHandler> router("player", "player", PlayerHandler());
 *
 *   // Get subscriptions for registration
 *   auto subs = router.subscriptions();
 *
 *   // Dispatch a command
 *   auto response = router.dispatch(contextual_command);
 * @endcode
 */
template <typename S, typename H>
class CommandHandlerRouter {
    static_assert(std::is_base_of_v<CommandHandlerDomainHandler<S>, H>,
                  "H must derive from CommandHandlerDomainHandler<S>");

   public:
    using State = S;
    using Handler = H;

    /**
     * Create a new command handler router.
     *
     * @param name The router name (e.g., "player")
     * @param domain The domain name (e.g., "player")
     * @param handler The handler instance
     */
    CommandHandlerRouter(std::string name, std::string domain, H handler)
        : name_(std::move(name)), domain_(std::move(domain)), handler_(std::move(handler)) {}

    /**
     * Get the router name.
     */
    const std::string& name() const { return name_; }

    /**
     * Get the domain.
     */
    const std::string& domain() const { return domain_; }

    /**
     * Get command types from the handler.
     */
    std::vector<std::string> command_types() const { return handler_.command_types(); }

    /**
     * Get subscriptions for this command handler.
     *
     * @return Vector of (domain, types) pairs
     */
    std::vector<std::pair<std::string, std::vector<std::string>>> subscriptions() const {
        return {{domain_, handler_.command_types()}};
    }

    /**
     * Build a component descriptor for topology registration.
     */
    Descriptor descriptor() const {
        return {name_, component_types::AGGREGATE, {{domain_, handler_.command_types()}}};
    }

    /**
     * Rebuild state from events using the handler's state router.
     */
    S rebuild_state(const EventBook* events) const { return handler_.rebuild(events); }

    /**
     * Dispatch a contextual command to the handler.
     *
     * @param cmd The contextual command (command + prior events)
     * @return BusinessResponse with events, notification, or revocation
     * @throws InvalidArgumentError if command is malformed
     * @throws CommandRejectedError if command is rejected by handler
     */
    BusinessResponse dispatch(const ContextualCommand& cmd) {
        const auto& command_book = cmd.command();
        const auto* prior_events = cmd.has_events() ? &cmd.events() : nullptr;

        // Rebuild state from prior events
        S state = handler_.rebuild(prior_events);
        int seq = helpers::next_sequence(prior_events);

        if (command_book.pages_size() == 0) {
            throw InvalidArgumentError("No command pages");
        }

        const auto& command_any = command_book.pages(0).command();
        if (command_any.type_url().empty()) {
            throw InvalidArgumentError("Empty command type_url");
        }

        const auto& type_url = command_any.type_url();

        // Check for Notification (rejection/compensation)
        if (helpers::type_url_matches(type_url, "angzarr.Notification")) {
            return dispatch_notification(command_any, state);
        }

        // Execute handler
        auto result_book = handler_.handle(command_book, command_any, state, seq);

        BusinessResponse response;
        *response.mutable_events() = std::move(result_book);
        return response;
    }

   private:
    /**
     * Dispatch a Notification to the handler's rejection handler.
     */
    BusinessResponse dispatch_notification(const google::protobuf::Any& command_any,
                                           const S& state) {
        Notification notification;
        if (!command_any.UnpackTo(&notification)) {
            throw InvalidArgumentError("Failed to unpack Notification");
        }

        auto [target_domain, target_command] = extract_rejection_key(notification);
        auto response = handler_.on_rejected(notification, state, target_domain, target_command);

        return build_rejection_response(response, target_domain, target_command);
    }

    /**
     * Extract domain and command suffix from a rejection notification.
     */
    static std::pair<std::string, std::string> extract_rejection_key(
        const Notification& notification) {
        std::string domain;
        std::string command_suffix;

        if (notification.has_payload()) {
            RejectionNotification rejection;
            if (notification.payload().UnpackTo(&rejection)) {
                if (rejection.has_rejected_command() &&
                    rejection.rejected_command().pages_size() > 0) {
                    const auto& rejected_cmd = rejection.rejected_command();
                    domain = rejected_cmd.cover().domain();
                    command_suffix =
                        helpers::type_name_from_url(rejected_cmd.pages(0).command().type_url());

                    // Extract simple type name
                    auto dot_pos = command_suffix.rfind('.');
                    if (dot_pos != std::string::npos) {
                        command_suffix = command_suffix.substr(dot_pos + 1);
                    }
                }
            }
        }
        return {domain, command_suffix};
    }

    /**
     * Build BusinessResponse from RejectionHandlerResponse.
     */
    BusinessResponse build_rejection_response(const RejectionHandlerResponse& response,
                                              const std::string& target_domain,
                                              const std::string& target_command) {
        BusinessResponse biz_response;

        if (response.events.has_value()) {
            *biz_response.mutable_events() = *response.events;
            return biz_response;
        }

        if (response.notification.has_value()) {
            *biz_response.mutable_notification() = *response.notification;
            return biz_response;
        }

        // Default: emit system revocation
        auto* revocation = biz_response.mutable_revocation();
        revocation->set_emit_system_revocation(true);
        revocation->set_reason("Handler returned empty response for " + target_domain + "/" +
                               target_command);
        return biz_response;
    }

    std::string name_;
    std::string domain_;
    H handler_;
};

// ============================================================================
// Saga Router
// ============================================================================

/**
 * Router for saga components (events -> commands, single domain, stateless).
 *
 * Domain is set at construction time. No `.domain()` method exists,
 * enforcing single-domain constraint at compile time.
 *
 * @tparam H The handler type (must satisfy SagaDomainHandler)
 *
 * Example:
 * @code
 *   SagaRouter<OrderSagaHandler> router("saga-order-fulfillment", "order", OrderSagaHandler());
 *
 *   // Get subscriptions
 *   auto subs = router.subscriptions();
 *
 *   // Prepare destinations
 *   auto covers = router.prepare_destinations(source);
 *
 *   // Dispatch event
 *   auto response = router.dispatch(source, destinations);
 * @endcode
 */
template <typename H>
class SagaRouter {
    static_assert(std::is_base_of_v<SagaDomainHandler, H>, "H must derive from SagaDomainHandler");

   public:
    using Handler = H;

    /**
     * Create a new saga router.
     *
     * @param name The router name (e.g., "saga-order-fulfillment")
     * @param domain The input domain name (e.g., "order")
     * @param handler The handler instance
     */
    SagaRouter(std::string name, std::string domain, H handler)
        : name_(std::move(name)), domain_(std::move(domain)), handler_(std::move(handler)) {}

    /**
     * Get the router name.
     */
    const std::string& name() const { return name_; }

    /**
     * Get the input domain.
     */
    const std::string& input_domain() const { return domain_; }

    /**
     * Get event types from the handler.
     */
    std::vector<std::string> event_types() const { return handler_.event_types(); }

    /**
     * Get subscriptions for this saga.
     *
     * @return Vector of (domain, types) pairs
     */
    std::vector<std::pair<std::string, std::vector<std::string>>> subscriptions() const {
        return {{domain_, handler_.event_types()}};
    }

    /**
     * Build a component descriptor for topology registration.
     */
    Descriptor descriptor() const {
        return {name_, component_types::SAGA, {{domain_, handler_.event_types()}}};
    }

    /**
     * Get destinations needed for the given source events.
     *
     * @param source The source event book (optional)
     * @return Covers for destinations to fetch
     */
    std::vector<Cover> prepare_destinations(const EventBook* source) {
        if (!source || source->pages_size() == 0) {
            return {};
        }

        const auto& last_page = source->pages(source->pages_size() - 1);
        if (!last_page.has_event()) {
            return {};
        }

        return handler_.prepare(*source, last_page.event());
    }

    /**
     * Dispatch an event to the saga handler.
     *
     * @param source The source event book
     * @param destinations Fetched destination event books
     * @return SagaResponse with commands
     * @throws InvalidArgumentError if source is malformed
     * @throws CommandRejectedError if execution fails
     */
    SagaResponse dispatch(const EventBook& source, const std::vector<EventBook>& destinations) {
        if (source.pages_size() == 0) {
            throw InvalidArgumentError("Source event book has no events");
        }

        const auto& last_page = source.pages(source.pages_size() - 1);
        if (!last_page.has_event()) {
            throw InvalidArgumentError("Missing event payload");
        }

        auto result = handler_.execute(source, last_page.event(), destinations);

        SagaResponse response;
        for (auto& cmd : result.commands) {
            *response.add_commands() = std::move(cmd);
        }
        return response;
    }

   private:
    std::string name_;
    std::string domain_;
    H handler_;
};

// ============================================================================
// Process Manager Router
// ============================================================================

/**
 * Router for process manager components (events -> commands + PM events, multi-domain).
 *
 * Domains are registered via fluent `.domain()` calls.
 *
 * @tparam S The PM state type
 *
 * Example:
 * @code
 *   auto router = ProcessManagerRouter<HandFlowState>("pmg-hand-flow", "hand-flow",
 *       [](const EventBook* events) { return rebuild_hand_flow_state(events); })
 *       .domain("order", std::make_shared<OrderPmHandler>())
 *       .domain("inventory", std::make_shared<InventoryPmHandler>());
 *
 *   // Get subscriptions
 *   auto subs = router.subscriptions();
 *
 *   // Prepare destinations
 *   auto covers = router.prepare_destinations(trigger, process_state);
 *
 *   // Dispatch event
 *   auto response = router.dispatch(trigger, process_state, destinations);
 * @endcode
 */
template <typename S>
class ProcessManagerRouter {
   public:
    using State = S;
    using Rebuilder = std::function<S(const EventBook*)>;
    using HandlerPtr = std::shared_ptr<ProcessManagerDomainHandler<S>>;

    /**
     * Create a new process manager router.
     *
     * @param name The router name (e.g., "pmg-hand-flow")
     * @param pm_domain The PM's own domain for state storage (e.g., "hand-flow")
     * @param rebuild Function to rebuild PM state from events
     */
    ProcessManagerRouter(std::string name, std::string pm_domain, Rebuilder rebuild)
        : name_(std::move(name)), pm_domain_(std::move(pm_domain)), rebuild_(std::move(rebuild)) {}

    /**
     * Register a domain handler.
     *
     * Process managers can have multiple input domains.
     *
     * @param domain_name The domain name (e.g., "order")
     * @param handler The handler for this domain
     * @return Reference to this router for chaining
     */
    ProcessManagerRouter& domain(std::string domain_name, HandlerPtr handler) {
        domains_[std::move(domain_name)] = std::move(handler);
        return *this;
    }

    /**
     * Get the router name.
     */
    const std::string& name() const { return name_; }

    /**
     * Get the PM's own domain (for state storage).
     */
    const std::string& pm_domain() const { return pm_domain_; }

    /**
     * Get subscriptions (domain + event types) for this PM.
     *
     * @return Vector of (domain, types) pairs
     */
    std::vector<std::pair<std::string, std::vector<std::string>>> subscriptions() const {
        std::vector<std::pair<std::string, std::vector<std::string>>> result;
        for (const auto& [domain, handler] : domains_) {
            result.emplace_back(domain, handler->event_types());
        }
        return result;
    }

    /**
     * Build a component descriptor for topology registration.
     */
    Descriptor descriptor() const {
        std::map<std::string, std::vector<std::string>> inputs;
        for (const auto& [domain, handler] : domains_) {
            inputs[domain] = handler->event_types();
        }
        return {name_, component_types::PROCESS_MANAGER, inputs};
    }

    /**
     * Rebuild PM state from events.
     */
    S rebuild_state(const EventBook* events) const { return rebuild_(events); }

    /**
     * Get destinations needed for the given trigger and process state.
     *
     * @param trigger The triggering event book (optional)
     * @param process_state The PM's current state events (optional)
     * @return Covers for destinations to fetch
     */
    std::vector<Cover> prepare_destinations(const EventBook* trigger,
                                            const EventBook* process_state) {
        if (!trigger || trigger->pages_size() == 0) {
            return {};
        }

        std::string trigger_domain = trigger->has_cover() ? trigger->cover().domain() : "";

        auto it = domains_.find(trigger_domain);
        if (it == domains_.end()) {
            return {};
        }

        const auto& last_page = trigger->pages(trigger->pages_size() - 1);
        if (!last_page.has_event()) {
            return {};
        }

        S state = rebuild_(process_state);
        return it->second->prepare(*trigger, state, last_page.event());
    }

    /**
     * Dispatch a trigger event to the appropriate handler.
     *
     * @param trigger The triggering event book
     * @param process_state The PM's current state events
     * @param destinations Fetched destination event books
     * @return ProcessManagerHandleResponse with commands and PM events
     * @throws InvalidArgumentError if trigger is malformed
     * @throws CommandRejectedError if handling fails
     */
    ProcessManagerHandleResponse dispatch(const EventBook& trigger, const EventBook* process_state,
                                          const std::vector<EventBook>& destinations) {
        std::string trigger_domain = trigger.has_cover() ? trigger.cover().domain() : "";

        auto it = domains_.find(trigger_domain);
        if (it == domains_.end()) {
            throw InvalidArgumentError("No handler for domain: " + trigger_domain);
        }

        if (trigger.pages_size() == 0) {
            throw InvalidArgumentError("Trigger event book has no events");
        }

        const auto& last_page = trigger.pages(trigger.pages_size() - 1);
        if (!last_page.has_event()) {
            throw InvalidArgumentError("Missing event payload");
        }

        S state = rebuild_(process_state);
        const auto& event_any = last_page.event();

        // Check for Notification
        if (helpers::type_url_matches(event_any.type_url(), "angzarr.Notification")) {
            return dispatch_notification(it->second.get(), event_any, state);
        }

        auto response = it->second->handle(trigger, state, event_any, destinations);

        ProcessManagerHandleResponse pm_response;
        for (auto& cmd : response.commands) {
            *pm_response.add_commands() = std::move(cmd);
        }
        if (response.process_events.has_value()) {
            *pm_response.mutable_process_events() = std::move(*response.process_events);
        }
        return pm_response;
    }

   private:
    /**
     * Dispatch a Notification to the handler's rejection handler.
     */
    ProcessManagerHandleResponse dispatch_notification(ProcessManagerDomainHandler<S>* handler,
                                                       const google::protobuf::Any& event_any,
                                                       const S& state) {
        Notification notification;
        if (!event_any.UnpackTo(&notification)) {
            throw InvalidArgumentError("Failed to unpack Notification");
        }

        auto [target_domain, target_command] = extract_rejection_key(notification);
        auto response = handler->on_rejected(notification, state, target_domain, target_command);

        ProcessManagerHandleResponse pm_response;
        if (response.events.has_value()) {
            *pm_response.mutable_process_events() = std::move(*response.events);
        }
        return pm_response;
    }

    /**
     * Extract domain and command suffix from a rejection notification.
     */
    static std::pair<std::string, std::string> extract_rejection_key(
        const Notification& notification) {
        std::string domain;
        std::string command_suffix;

        if (notification.has_payload()) {
            RejectionNotification rejection;
            if (notification.payload().UnpackTo(&rejection)) {
                if (rejection.has_rejected_command() &&
                    rejection.rejected_command().pages_size() > 0) {
                    const auto& rejected_cmd = rejection.rejected_command();
                    domain = rejected_cmd.cover().domain();
                    command_suffix =
                        helpers::type_name_from_url(rejected_cmd.pages(0).command().type_url());

                    // Extract simple type name
                    auto dot_pos = command_suffix.rfind('.');
                    if (dot_pos != std::string::npos) {
                        command_suffix = command_suffix.substr(dot_pos + 1);
                    }
                }
            }
        }
        return {domain, command_suffix};
    }

    std::string name_;
    std::string pm_domain_;
    Rebuilder rebuild_;
    std::map<std::string, HandlerPtr> domains_;
};

// ============================================================================
// Projector Router
// ============================================================================

/**
 * Router for projector components (events -> external output, multi-domain).
 *
 * Domains are registered via fluent `.domain()` calls.
 *
 * Example:
 * @code
 *   auto router = ProjectorRouter("prj-output")
 *       .domain("player", std::make_shared<PlayerProjectorHandler>())
 *       .domain("hand", std::make_shared<HandProjectorHandler>());
 *
 *   // Get subscriptions
 *   auto subs = router.subscriptions();
 *
 *   // Dispatch events
 *   auto projection = router.dispatch(events);
 * @endcode
 */
class ProjectorRouter {
   public:
    using HandlerPtr = std::shared_ptr<ProjectorDomainHandler>;

    /**
     * Create a new projector router.
     *
     * @param name The router name (e.g., "prj-output")
     */
    explicit ProjectorRouter(std::string name) : name_(std::move(name)) {}

    /**
     * Register a domain handler.
     *
     * Projectors can have multiple input domains.
     *
     * @param domain_name The domain name (e.g., "player")
     * @param handler The handler for this domain
     * @return Reference to this router for chaining
     */
    ProjectorRouter& domain(std::string domain_name, HandlerPtr handler) {
        domains_[std::move(domain_name)] = std::move(handler);
        return *this;
    }

    /**
     * Get the router name.
     */
    const std::string& name() const { return name_; }

    /**
     * Get subscriptions (domain + event types) for this projector.
     *
     * @return Vector of (domain, types) pairs
     */
    std::vector<std::pair<std::string, std::vector<std::string>>> subscriptions() const {
        std::vector<std::pair<std::string, std::vector<std::string>>> result;
        for (const auto& [domain, handler] : domains_) {
            result.emplace_back(domain, handler->event_types());
        }
        return result;
    }

    /**
     * Build a component descriptor for topology registration.
     */
    Descriptor descriptor() const {
        std::map<std::string, std::vector<std::string>> inputs;
        for (const auto& [domain, handler] : domains_) {
            inputs[domain] = handler->event_types();
        }
        return {name_, component_types::PROJECTOR, inputs};
    }

    /**
     * Dispatch events to the appropriate handler.
     *
     * @param events The event book to project
     * @return Projection result
     * @throws InvalidArgumentError if no handler for domain
     */
    Projection dispatch(const EventBook& events) {
        std::string domain = events.has_cover() ? events.cover().domain() : "";

        auto it = domains_.find(domain);
        if (it == domains_.end()) {
            throw InvalidArgumentError("No handler for domain: " + domain);
        }

        return it->second->project(events);
    }

   private:
    std::string name_;
    std::map<std::string, HandlerPtr> domains_;
};

// ============================================================================
// Factory Functions
// ============================================================================

/**
 * Create a command handler router.
 *
 * @tparam S The state type
 * @tparam H The handler type
 * @param name The router name
 * @param domain The domain name
 * @param handler The handler instance
 * @return CommandHandlerRouter instance
 */
template <typename S, typename H>
CommandHandlerRouter<S, H> make_command_handler_router(std::string name, std::string domain,
                                                       H handler) {
    return CommandHandlerRouter<S, H>(std::move(name), std::move(domain), std::move(handler));
}

/**
 * Create a saga router.
 *
 * @tparam H The handler type
 * @param name The router name
 * @param domain The input domain name
 * @param handler The handler instance
 * @return SagaRouter instance
 */
template <typename H>
SagaRouter<H> make_saga_router(std::string name, std::string domain, H handler) {
    return SagaRouter<H>(std::move(name), std::move(domain), std::move(handler));
}

/**
 * Create a process manager router.
 *
 * @tparam S The state type
 * @param name The router name
 * @param pm_domain The PM's own domain
 * @param rebuild Function to rebuild PM state
 * @return ProcessManagerRouter instance
 */
template <typename S>
ProcessManagerRouter<S> make_pm_router(std::string name, std::string pm_domain,
                                       typename ProcessManagerRouter<S>::Rebuilder rebuild) {
    return ProcessManagerRouter<S>(std::move(name), std::move(pm_domain), std::move(rebuild));
}

/**
 * Create a projector router.
 *
 * @param name The router name
 * @return ProjectorRouter instance
 */
inline ProjectorRouter make_projector_router(std::string name) {
    return ProjectorRouter(std::move(name));
}

// ============================================================================
// Backward Compatibility Aliases
// ============================================================================

/// @deprecated Use CommandHandlerRouter instead
template <typename S, typename H>
using AggregateRouter = CommandHandlerRouter<S, H>;

/// @deprecated Use make_command_handler_router instead
template <typename S, typename H>
AggregateRouter<S, H> make_aggregate_router(std::string name, std::string domain, H handler) {
    return CommandHandlerRouter<S, H>(std::move(name), std::move(domain), std::move(handler));
}

}  // namespace angzarr
