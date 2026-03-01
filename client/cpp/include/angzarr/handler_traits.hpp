#pragma once

#include <google/protobuf/any.pb.h>

#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "angzarr/types.pb.h"
#include "errors.hpp"

namespace angzarr {

// ============================================================================
// Type Extraction for Handler Registration
// ============================================================================

namespace handler_traits {

/**
 * Extract type suffix from a protobuf message type.
 * Uses protobuf's descriptor to get the simple type name.
 *
 * Example: examples::PlayerRegistered -> "PlayerRegistered"
 */
template <typename T>
std::string type_suffix() {
    return T::descriptor()->name();
}

}  // namespace handler_traits

// ============================================================================
// Common Types
// ============================================================================

/**
 * Response from rejection handlers.
 *
 * Handlers may return:
 * - Events to compensate/fix state
 * - Notification to forward upstream
 * - Both
 */
struct RejectionHandlerResponse {
    /// Events to persist (compensation).
    std::optional<EventBook> events;
    /// Notification to forward upstream.
    std::optional<Notification> notification;

    /// Default: empty response (framework handles).
    static RejectionHandlerResponse empty() { return {}; }

    /// Create response with compensation events.
    static RejectionHandlerResponse with_events(EventBook events) {
        return {std::move(events), std::nullopt};
    }

    /// Create response with forwarded notification.
    static RejectionHandlerResponse with_notification(Notification notification) {
        return {std::nullopt, std::move(notification)};
    }
};

/**
 * Response from saga handlers.
 */
struct SagaHandlerResponse {
    /// Commands to send to other aggregates.
    std::vector<CommandBook> commands;
    /// Events/facts to inject directly into target aggregates.
    std::vector<EventBook> events;

    /// Default: empty response.
    static SagaHandlerResponse empty() { return {{}, {}}; }

    /// Create response with commands only.
    static SagaHandlerResponse with_commands(std::vector<CommandBook> commands) {
        return {std::move(commands), {}};
    }

    /// Create response with events only.
    static SagaHandlerResponse with_events(std::vector<EventBook> events) {
        return {{}, std::move(events)};
    }

    /// Create response with both commands and events.
    static SagaHandlerResponse with_both(std::vector<CommandBook> commands,
                                         std::vector<EventBook> events) {
        return {std::move(commands), std::move(events)};
    }
};

/**
 * Response from process manager handlers.
 */
struct ProcessManagerResponse {
    /// Commands to send to other aggregates.
    std::vector<CommandBook> commands;
    /// Events to persist to the PM's own domain.
    std::optional<EventBook> process_events;
    /// Facts to inject directly into other aggregates.
    std::vector<EventBook> facts;

    /// Default: empty response.
    static ProcessManagerResponse empty() { return {{}, std::nullopt, {}}; }

    /// Create response with commands only.
    static ProcessManagerResponse with_commands(std::vector<CommandBook> commands) {
        return {std::move(commands), std::nullopt, {}};
    }

    /// Create response with PM events only.
    static ProcessManagerResponse with_process_events(EventBook events) {
        return {{}, std::move(events), {}};
    }

    /// Create response with facts only.
    static ProcessManagerResponse with_facts(std::vector<EventBook> facts) {
        return {{}, std::nullopt, std::move(facts)};
    }

    /// Create response with commands and PM events.
    static ProcessManagerResponse with_both(std::vector<CommandBook> commands, EventBook events) {
        return {std::move(commands), std::move(events), {}};
    }

    /// Create response with all fields.
    static ProcessManagerResponse with_all(std::vector<CommandBook> commands, EventBook events,
                                           std::vector<EventBook> facts) {
        return {std::move(commands), std::move(events), std::move(facts)};
    }
};

// ============================================================================
// State Router
// ============================================================================

/**
 * Fluent state reconstruction router.
 *
 * Provides a builder pattern for registering event appliers with auto-unpacking.
 * Register once at startup, call `with_event_book()` per rebuild.
 *
 * @tparam S The state type (must be default-constructible)
 *
 * Example:
 * @code
 *   StateRouter<PlayerState> router([]() { return PlayerState{}; });
 *   router.on<PlayerRegistered>([](PlayerState& s, const PlayerRegistered& e) {
 *       s.set_player_id(e.player_id());
 *   });
 *   router.on<FundsDeposited>([](PlayerState& s, const FundsDeposited& e) {
 *       s.set_bankroll(e.new_balance());
 *   });
 *
 *   // Use per rebuild
 *   PlayerState state = router.with_event_book(&events);
 * @endcode
 */
template <typename S>
class StateRouter {
   public:
    using Applier = std::function<void(S&, const google::protobuf::Any&)>;
    using Factory = std::function<S()>;

    /**
     * Create a StateRouter using S's default constructor.
     */
    StateRouter() : factory_([]() { return S{}; }) {}

    /**
     * Create a StateRouter with a custom state factory.
     */
    explicit StateRouter(Factory factory) : factory_(std::move(factory)) {}

    /**
     * Register an event applier for the given protobuf event type.
     *
     * @tparam Event The protobuf event type
     * @param applier Function that takes (S&, const Event&) and mutates state
     */
    template <typename Event>
    StateRouter& on(std::function<void(S&, const Event&)> applier) {
        std::string suffix = Event::descriptor()->name();
        appliers_[suffix] = [applier](S& state, const google::protobuf::Any& any) {
            Event event;
            any.UnpackTo(&event);
            applier(state, event);
        };
        return *this;
    }

    /**
     * Rebuild state from an EventBook.
     */
    S with_event_book(const EventBook* book) const {
        S state = factory_();
        if (!book) return state;

        for (const auto& page : book->pages()) {
            if (!page.has_event()) continue;
            apply_single(state, page.event());
        }
        return state;
    }

    /**
     * Apply a single event to existing state.
     */
    void apply_single(S& state, const google::protobuf::Any& event_any) const {
        // Extract type name from URL (e.g., "type.googleapis.com/examples.CardsDealt" ->
        // "CardsDealt")
        const std::string& type_url = event_any.type_url();
        auto pos = type_url.rfind('/');
        std::string full_name = (pos != std::string::npos) ? type_url.substr(pos + 1) : type_url;

        // Extract simple name (e.g., "examples.CardsDealt" -> "CardsDealt")
        auto dot_pos = full_name.rfind('.');
        std::string simple_name =
            (dot_pos != std::string::npos) ? full_name.substr(dot_pos + 1) : full_name;

        auto it = appliers_.find(simple_name);
        if (it != appliers_.end()) {
            it->second(state, event_any);
        }
        // Unknown event type -- silently ignore (forward compatibility)
    }

   private:
    Factory factory_;
    std::map<std::string, Applier> appliers_;
};

// ============================================================================
// Command Handler Trait
// ============================================================================

/**
 * Handler for a single domain's command handler logic.
 *
 * Command handlers receive commands and emit events. They maintain state
 * that is rebuilt from events using a `StateRouter`.
 *
 * @tparam S The state type
 *
 * Example:
 * @code
 *   class PlayerHandler : public CommandHandlerDomainHandler<PlayerState> {
 *   public:
 *       std::vector<std::string> command_types() const override {
 *           return {"RegisterPlayer", "DepositFunds"};
 *       }
 *
 *       const StateRouter<PlayerState>& state_router() const override {
 *           return state_router_;
 *       }
 *
 *       EventBook handle(const CommandBook& cmd,
 *                        const google::protobuf::Any& payload,
 *                        const PlayerState& state,
 *                        int seq) override {
 *           // Dispatch based on payload.type_url()
 *       }
 *
 *   private:
 *       StateRouter<PlayerState> state_router_;
 *   };
 * @endcode
 */
template <typename S>
class CommandHandlerDomainHandler {
   public:
    using State = S;

    virtual ~CommandHandlerDomainHandler() = default;

    /**
     * Command type suffixes this handler processes.
     *
     * Used for subscription derivation and routing.
     */
    virtual std::vector<std::string> command_types() const = 0;

    /**
     * Get the state router for rebuilding state from events.
     */
    virtual const StateRouter<S>& state_router() const = 0;

    /**
     * Rebuild state from events.
     *
     * Default implementation uses `state_router().with_event_book()`.
     */
    virtual S rebuild(const EventBook* events) const {
        return state_router().with_event_book(events);
    }

    /**
     * Handle a command and return resulting events.
     *
     * The handler should dispatch internally based on `payload.type_url`.
     *
     * @param cmd The full command book
     * @param payload The unpacked command (as Any)
     * @param state The current aggregate state
     * @param seq The next sequence number for events
     * @return EventBook containing resulting events
     * @throws CommandRejectedError if the command is rejected
     */
    virtual EventBook handle(const CommandBook& cmd, const google::protobuf::Any& payload,
                             const S& state, int seq) = 0;

    /**
     * Handle a rejection notification.
     *
     * Called when a command issued by a saga/PM targeting this aggregate's
     * domain was rejected. Override to provide custom compensation logic.
     *
     * Default implementation returns an empty response (framework handles).
     *
     * @param notification The rejection notification
     * @param state The current aggregate state
     * @param target_domain The domain that rejected the command
     * @param target_command The command type that was rejected
     * @return RejectionHandlerResponse with compensation events or forwarded notification
     */
    virtual RejectionHandlerResponse on_rejected(const Notification& notification, const S& state,
                                                 const std::string& target_domain,
                                                 const std::string& target_command) {
        (void)notification;
        (void)state;
        (void)target_domain;
        (void)target_command;
        return RejectionHandlerResponse::empty();
    }
};

// ============================================================================
// Saga Handler Trait
// ============================================================================

/**
 * Handler for a single domain's events in a saga.
 *
 * Sagas translate events from one domain into commands for another.
 * They are stateless -- each event is processed independently.
 *
 * Example:
 * @code
 *   class OrderSagaHandler : public SagaDomainHandler {
 *   public:
 *       std::vector<std::string> event_types() const override {
 *           return {"OrderCompleted", "OrderCancelled"};
 *       }
 *
 *       std::vector<Cover> prepare(const EventBook& source,
 *                                  const google::protobuf::Any& event) override {
 *           // Return covers for destinations to fetch
 *       }
 *
 *       SagaHandlerResponse execute(const EventBook& source,
 *                                   const google::protobuf::Any& event,
 *                                   const std::vector<EventBook>& destinations) override {
 *           // Transform event into commands and/or events
 *           return SagaHandlerResponse::with_commands({...});
 *       }
 *   };
 * @endcode
 */
class SagaDomainHandler {
   public:
    virtual ~SagaDomainHandler() = default;

    /**
     * Event type suffixes this handler processes.
     *
     * Used for subscription derivation.
     */
    virtual std::vector<std::string> event_types() const = 0;

    /**
     * Prepare phase -- declare destination covers needed.
     *
     * Called before execute to fetch destination aggregate state.
     *
     * @param source The source event book
     * @param event The event being processed
     * @return Covers for destinations to fetch
     */
    virtual std::vector<Cover> prepare(const EventBook& source,
                                       const google::protobuf::Any& event) = 0;

    /**
     * Execute phase -- produce commands and/or events.
     *
     * Called with source event and fetched destination state.
     *
     * @param source The source event book
     * @param event The event being processed
     * @param destinations Fetched destination event books
     * @return Response containing commands and/or events to inject
     * @throws CommandRejectedError if execution fails
     */
    virtual SagaHandlerResponse execute(const EventBook& source, const google::protobuf::Any& event,
                                        const std::vector<EventBook>& destinations) = 0;

    /**
     * Handle a rejection notification.
     *
     * Called when a command issued by this saga was rejected by the target
     * aggregate. Override to provide custom compensation logic.
     *
     * Default implementation returns an empty response (framework handles).
     *
     * @param notification The rejection notification
     * @param target_domain The domain that rejected the command
     * @param target_command The command type that was rejected
     * @return RejectionHandlerResponse with compensation events or forwarded notification
     */
    virtual RejectionHandlerResponse on_rejected(const Notification& notification,
                                                 const std::string& target_domain,
                                                 const std::string& target_command) {
        (void)notification;
        (void)target_domain;
        (void)target_command;
        return RejectionHandlerResponse::empty();
    }
};

// ============================================================================
// Process Manager Handler Trait
// ============================================================================

/**
 * Handler for a single domain's events in a process manager.
 *
 * Process managers correlate events across multiple domains and maintain
 * their own state. Each domain gets its own handler, but they all share
 * the same PM state type.
 *
 * @tparam S The PM state type
 *
 * Example:
 * @code
 *   class OrderPmHandler : public ProcessManagerDomainHandler<HandFlowState> {
 *   public:
 *       std::vector<std::string> event_types() const override {
 *           return {"OrderCreated"};
 *       }
 *
 *       std::vector<Cover> prepare(const EventBook& trigger,
 *                                  const HandFlowState& state,
 *                                  const google::protobuf::Any& event) override {
 *           // Declare needed destinations
 *           return {};
 *       }
 *
 *       ProcessManagerResponse handle(const EventBook& trigger,
 *                                     const HandFlowState& state,
 *                                     const google::protobuf::Any& event,
 *                                     const std::vector<EventBook>& destinations) override {
 *           // Process event, emit commands and/or PM events
 *           return ProcessManagerResponse::empty();
 *       }
 *   };
 * @endcode
 */
template <typename S>
class ProcessManagerDomainHandler {
   public:
    virtual ~ProcessManagerDomainHandler() = default;

    /**
     * Event type suffixes this handler processes.
     */
    virtual std::vector<std::string> event_types() const = 0;

    /**
     * Prepare phase -- declare destination covers needed.
     *
     * @param trigger The triggering event book
     * @param state The current PM state
     * @param event The event being processed
     * @return Covers for destinations to fetch
     */
    virtual std::vector<Cover> prepare(const EventBook& trigger, const S& state,
                                       const google::protobuf::Any& event) = 0;

    /**
     * Handle phase -- produce commands and PM events.
     *
     * @param trigger The triggering event book
     * @param state The current PM state
     * @param event The event being processed
     * @param destinations Fetched destination event books
     * @return Response containing commands and/or PM events
     * @throws CommandRejectedError if handling fails
     */
    virtual ProcessManagerResponse handle(const EventBook& trigger, const S& state,
                                          const google::protobuf::Any& event,
                                          const std::vector<EventBook>& destinations) = 0;

    /**
     * Handle a rejection notification.
     *
     * Called when a PM-issued command was rejected. Override to provide
     * custom compensation logic.
     *
     * @param notification The rejection notification
     * @param state The current PM state
     * @param target_domain The domain that rejected the command
     * @param target_command The command type that was rejected
     * @return RejectionHandlerResponse with compensation events or forwarded notification
     */
    virtual RejectionHandlerResponse on_rejected(const Notification& notification, const S& state,
                                                 const std::string& target_domain,
                                                 const std::string& target_command) {
        (void)notification;
        (void)state;
        (void)target_domain;
        (void)target_command;
        return RejectionHandlerResponse::empty();
    }
};

// ============================================================================
// Backward Compatibility Aliases
// ============================================================================

/// @deprecated Use CommandHandlerDomainHandler instead
template <typename S>
using AggregateDomainHandler = CommandHandlerDomainHandler<S>;

// ============================================================================
// Projector Handler Trait
// ============================================================================

/**
 * Handler for a single domain's events in a projector.
 *
 * Projectors consume events and produce external output (read models,
 * caches, external systems).
 *
 * Example:
 * @code
 *   class PlayerProjectorHandler : public ProjectorDomainHandler {
 *   public:
 *       std::vector<std::string> event_types() const override {
 *           return {"PlayerRegistered", "FundsDeposited"};
 *       }
 *
 *       Projection project(const EventBook& events) override {
 *           // Update external read model
 *           return Projection{};
 *       }
 *   };
 * @endcode
 */
class ProjectorDomainHandler {
   public:
    virtual ~ProjectorDomainHandler() = default;

    /**
     * Event type suffixes this handler processes.
     */
    virtual std::vector<std::string> event_types() const = 0;

    /**
     * Project events to external output.
     *
     * @param events The event book to project
     * @return Projection result
     * @throws std::exception on projection failure
     */
    virtual Projection project(const EventBook& events) = 0;
};

}  // namespace angzarr
