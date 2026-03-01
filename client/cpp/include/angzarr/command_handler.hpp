#pragma once

#include <google/protobuf/any.pb.h>

#include <functional>
#include <map>
#include <string>
#include <vector>

#include "angzarr/command_handler.pb.h"
#include "angzarr/types.pb.h"
#include "descriptor.hpp"
#include "errors.hpp"
#include "helpers.hpp"
#include "handler_traits.hpp"
#include "router.hpp"

namespace angzarr {

/**
 * OO-style command handler base class using CRTP pattern.
 *
 * Usage (similar to Go's CommandHandlerBase):
 *   class Player : public CommandHandlerBase<PlayerState, Player> {
 *   public:
 *       static constexpr const char* kDomain = "player";
 *
 *       Player(const EventBook* events = nullptr) {
 *           init(events, []() { return PlayerState{}; });
 *           set_domain(kDomain);
 *
 *           // Register handlers (type-safe, inferred from method signature)
 *           handles(&Player::register_player);
 *           handles(&Player::deposit_funds);
 *
 *           // Register event appliers
 *           applies(&Player::apply_registered);
 *           applies(&Player::apply_deposited);
 *
 *           // Register rejection handlers
 *           handles_rejection("table", "JoinTable", &Player::handle_join_rejected);
 *       }
 *
 *       // Command handlers: Event handler(const Command& cmd)
 *       PlayerRegistered register_player(const RegisterPlayer& cmd);
 *       FundsDeposited deposit_funds(const DepositFunds& cmd);
 *
 *       // Event appliers: void applier(StateT& state, const Event& event)
 *       void apply_registered(PlayerState& state, const PlayerRegistered& event);
 *       void apply_deposited(PlayerState& state, const FundsDeposited& event);
 *
 *       // Rejection handlers: EventBook handler(const Notification& notif)
 *       EventBook handle_join_rejected(const Notification& notification);
 *   };
 */
template <typename StateT, typename Derived>
class CommandHandlerBase {
   public:
    using State = StateT;
    using StateFactory = std::function<StateT()>;
    using CommandDispatcher = std::function<EventBook(Derived*, const google::protobuf::Any&, int)>;
    using EventApplier = std::function<void(Derived*, StateT&, const google::protobuf::Any&)>;
    using RejectionDispatcher = std::function<EventBook(Derived*, const Notification&)>;

    virtual ~CommandHandlerBase() = default;

    // =========================================================================
    // Initialization
    // =========================================================================

    /**
     * Initialize the command handler with prior events and state factory.
     * Call this in derived class constructor.
     */
    void init(const EventBook* events, StateFactory factory) {
        state_factory_ = std::move(factory);
        state_ = state_factory_();
        exists_ = false;

        if (events) {
            for (const auto& page : events->pages()) {
                if (page.has_event()) {
                    apply_event(page.event());
                    exists_ = true;
                }
            }
        }
    }

    /**
     * Set the domain name for this handler.
     */
    void set_domain(const std::string& domain) { domain_ = domain; }

    /**
     * Get the domain name.
     */
    std::string domain() const { return domain_; }

    // =========================================================================
    // Handler Registration (called from constructor)
    // =========================================================================

    /**
     * Register a command handler method.
     * Type-safe: extracts Command and Event types from method signature.
     *
     * Method signature: Event handler(const Command& cmd)
     */
    template <typename Command, typename Event>
    Derived& handles(Event (Derived::*method)(const Command&)) {
        auto suffix = handler_traits::type_suffix<Command>();
        handlers_[suffix] = [method](Derived* self, const google::protobuf::Any& any, int seq) {
            Command cmd;
            any.UnpackTo(&cmd);
            Event event = (self->*method)(cmd);
            return helpers::new_event_book(event, seq);
        };
        return *static_cast<Derived*>(this);
    }

    /**
     * Register an event applier method.
     * Type-safe: extracts Event type from method signature.
     *
     * Method signature: void applier(StateT& state, const Event& event)
     */
    template <typename Event>
    Derived& applies(void (Derived::*method)(StateT&, const Event&)) {
        auto suffix = handler_traits::type_suffix<Event>();
        appliers_[suffix] = [method](Derived* self, StateT& state, const google::protobuf::Any& any) {
            Event event;
            any.UnpackTo(&event);
            (self->*method)(state, event);
        };
        return *static_cast<Derived*>(this);
    }

    /**
     * Register a rejection handler for compensation.
     *
     * Method signature: EventBook handler(const Notification& notification)
     */
    Derived& handles_rejection(const std::string& source_domain, const std::string& command_type,
                               EventBook (Derived::*method)(const Notification&)) {
        auto key = source_domain + "/" + command_type;
        rejection_handlers_[key] = [method](Derived* self, const Notification& notification) {
            return (self->*method)(notification);
        };
        return *static_cast<Derived*>(this);
    }

    // =========================================================================
    // Dispatch
    // =========================================================================

    /**
     * Dispatch a ContextualCommand to the appropriate handler.
     */
    BusinessResponse dispatch(const ContextualCommand& cmd) {
        const auto& command_book = cmd.command();
        const auto* prior_events = cmd.has_events() ? &cmd.events() : nullptr;

        // Rebuild state from prior events
        state_ = state_factory_();
        exists_ = false;
        if (prior_events) {
            for (const auto& page : prior_events->pages()) {
                if (page.has_event()) {
                    apply_event(page.event());
                    exists_ = true;
                }
            }
        }

        int seq = helpers::next_sequence(prior_events);

        if (command_book.pages_size() == 0) {
            throw InvalidArgumentError("No command pages");
        }

        const auto& command_any = command_book.pages(0).command();
        if (command_any.type_url().empty()) {
            throw InvalidArgumentError("Empty command type_url");
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
        auto it = handlers_.find(suffix);
        if (it != handlers_.end()) {
            auto events = it->second(static_cast<Derived*>(this), command_any, seq);
            BusinessResponse response;
            *response.mutable_events() = std::move(events);
            return response;
        }

        throw InvalidArgumentError("Unknown command type: " + type_url);
    }

    /**
     * Build a component descriptor for this command handler.
     */
    Descriptor descriptor() const {
        std::vector<std::string> types;
        for (const auto& [suffix, _] : handlers_) {
            types.push_back(suffix);
        }
        return {domain_, component_types::AGGREGATE, {{domain_, types}}};
    }

    // =========================================================================
    // State Access
    // =========================================================================

    /**
     * Check if the aggregate exists (has prior events).
     */
    bool exists() const { return exists_; }

    /**
     * Get the current state (const).
     */
    const StateT& state() const { return state_; }

    /**
     * Get a pointer to current state (for appliers that need access).
     */
    StateT* mutable_state() { return &state_; }

   private:
    void apply_event(const google::protobuf::Any& event_any) {
        auto suffix = helpers::type_name_from_url(event_any.type_url());
        auto it = appliers_.find(suffix);
        if (it != appliers_.end()) {
            it->second(static_cast<Derived*>(this), state_, event_any);
        }
    }

    BusinessResponse dispatch_rejection(const Notification& notification) {
        std::string source_domain;
        std::string command_suffix;

        if (notification.has_payload()) {
            RejectionNotification rejection;
            if (notification.payload().UnpackTo(&rejection)) {
                if (rejection.has_rejected_command() &&
                    rejection.rejected_command().pages_size() > 0) {
                    const auto& rejected_cmd = rejection.rejected_command();
                    source_domain = rejected_cmd.cover().domain();
                    command_suffix =
                        helpers::type_name_from_url(rejected_cmd.pages(0).command().type_url());
                }
            }
        }

        auto key = source_domain + "/" + command_suffix;
        auto it = rejection_handlers_.find(key);
        if (it != rejection_handlers_.end()) {
            auto events = it->second(static_cast<Derived*>(this), notification);
            BusinessResponse response;
            *response.mutable_events() = std::move(events);
            return response;
        }

        // Default: emit system revocation
        BusinessResponse response;
        auto* revocation = response.mutable_revocation();
        revocation->set_emit_system_revocation(true);
        revocation->set_reason("CommandHandler " + domain_ +
                               " has no custom compensation for " + key);
        return response;
    }

    std::string domain_;
    StateT state_;
    StateFactory state_factory_;
    bool exists_ = false;

    // Per-instance registries (populated in constructor)
    std::map<std::string, CommandDispatcher> handlers_;
    std::map<std::string, EventApplier> appliers_;
    std::map<std::string, RejectionDispatcher> rejection_handlers_;
};

// =============================================================================
// Legacy CommandHandler (for backwards compatibility with functional pattern)
// =============================================================================

/**
 * Legacy base class for command handlers using macro-based handler registration.
 * @deprecated Use CommandHandlerBase with CRTP pattern instead.
 */
template <typename StateT>
class CommandHandler {
   public:
    using State = StateT;
    using CommandDispatcher =
        std::function<EventBook(CommandHandler*, const google::protobuf::Any&, int)>;
    using EventApplier =
        std::function<void(CommandHandler*, StateT&, const google::protobuf::Any&)>;
    using RejectionHandler =
        std::function<EventBook(CommandHandler*, const Notification&, StateT&)>;

    virtual ~CommandHandler() = default;

    virtual std::string domain() const = 0;

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

        if (helpers::type_url_matches(type_url, "Notification")) {
            Notification notification;
            command_any.UnpackTo(&notification);
            return dispatch_rejection(notification);
        }

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

    Descriptor descriptor() const {
        std::vector<std::string> types;
        for (const auto& [suffix, _] : handlers()) {
            types.push_back(suffix);
        }
        return {domain(), component_types::AGGREGATE, {{domain(), types}}};
    }

    bool exists() const { return exists_; }
    const StateT& state() const { return state_; }
    StateT& mutable_state() { return state_; }

   protected:
    virtual StateT create_empty_state() = 0;

    static void register_handler(const std::string& suffix, CommandDispatcher dispatcher) {
        handlers()[suffix] = std::move(dispatcher);
    }

    static void register_applier(const std::string& suffix, EventApplier applier) {
        appliers()[suffix] = std::move(applier);
    }

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
        std::string domain_name;
        std::string command_suffix;

        if (notification.has_payload()) {
            RejectionNotification rejection;
            if (notification.payload().UnpackTo(&rejection)) {
                if (rejection.has_rejected_command() &&
                    rejection.rejected_command().pages_size() > 0) {
                    const auto& rejected_cmd = rejection.rejected_command();
                    domain_name = rejected_cmd.cover().domain();
                    command_suffix =
                        helpers::type_name_from_url(rejected_cmd.pages(0).command().type_url());
                }
            }
        }

        auto key = domain_name + "/" + command_suffix;
        auto it = rejection_handlers().find(key);
        if (it != rejection_handlers().end()) {
            auto events = it->second(this, notification, state_);
            BusinessResponse response;
            *response.mutable_events() = std::move(events);
            return response;
        }

        BusinessResponse response;
        auto* revocation = response.mutable_revocation();
        revocation->set_emit_system_revocation(true);
        revocation->set_reason("CommandHandler " + this->domain() +
                               " has no custom compensation for " + key);
        return response;
    }

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

}  // namespace angzarr
