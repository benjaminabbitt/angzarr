#pragma once

/**
 * @file macros.hpp
 * @brief Convenience macros for handler registration.
 *
 * RECOMMENDED: Use the CRTP-based CommandHandlerBase with constructor registration
 * instead of these macros. The CRTP pattern is type-safe and similar to Go:
 *
 * @code
 *   class Player : public CommandHandlerBase<PlayerState, Player> {
 *   public:
 *       static constexpr const char* kDomain = "player";
 *
 *       Player(const EventBook* events = nullptr) {
 *           init(events, []() { return PlayerState{}; });
 *           set_domain(kDomain);
 *
 *           // Type-safe registration (types inferred from method signature)
 *           handles(&Player::register_player);
 *           handles(&Player::deposit_funds);
 *           applies(&Player::apply_registered);
 *           applies(&Player::apply_deposited);
 *           handles_rejection("table", "JoinTable", &Player::handle_join_rejected);
 *       }
 *
 *       // Command handlers: Event handler(const Command& cmd)
 *       PlayerRegistered register_player(const RegisterPlayer& cmd);
 *       FundsDeposited deposit_funds(const DepositFunds& cmd);
 *
 *       // Event appliers: void applier(State& state, const Event& event)
 *       void apply_registered(PlayerState& state, const PlayerRegistered& event);
 *       void apply_deposited(PlayerState& state, const FundsDeposited& event);
 *
 *       // Rejection handlers: EventBook handler(const Notification&)
 *       EventBook handle_join_rejected(const Notification& notification);
 *   };
 * @endcode
 *
 * The macros below are provided for backward compatibility and simple cases.
 * They require passing the class name explicitly to avoid `decltype(this)` issues.
 */

// Helper to extract type suffix from proto type
#define ANGZARR_TYPE_SUFFIX(T) #T

/**
 * Declare this class as an aggregate with the given domain name.
 * Must be in public section of class.
 */
#define ANGZARR_AGGREGATE(domain_name)                  \
    static constexpr const char* kDomain = domain_name; \
    std::string domain() const override { return kDomain; }

/**
 * Declare this class as a saga with the given name and domains.
 * Must be in public section of class.
 */
#define ANGZARR_SAGA(saga_name, in_domain, out_domain)                 \
    static constexpr const char* kName = saga_name;                    \
    static constexpr const char* kInputDomain = in_domain;             \
    static constexpr const char* kOutputDomain = out_domain;           \
    std::string name() const override { return kName; }                \
    std::string input_domain() const override { return kInputDomain; } \
    std::string output_domain() const override { return kOutputDomain; }

/**
 * Register handler registration calls for the constructor.
 * Use with CommandHandlerBase CRTP pattern.
 *
 * Usage in constructor:
 *   ANGZARR_REGISTER_HANDLERS(Player,
 *       handles(&Player::register_player),
 *       handles(&Player::deposit_funds),
 *       applies(&Player::apply_registered),
 *       handles_rejection("table", "JoinTable", &Player::handle_join_rejected)
 *   )
 */
#define ANGZARR_REGISTER_HANDLERS(Class, ...) __VA_ARGS__

/**
 * Initialize CommandHandlerBase in constructor.
 * Combines init(), set_domain(), and handler registration.
 *
 * Usage:
 *   Player(const EventBook* events = nullptr) {
 *       ANGZARR_INIT_HANDLER(Player, "player", PlayerState, events);
 *       handles(&Player::register_player);
 *       applies(&Player::apply_registered);
 *   }
 */
#define ANGZARR_INIT_HANDLER(Class, domain_name, StateType, events) \
    init(events, []() { return StateType{}; });                     \
    set_domain(domain_name)

// =============================================================================
// Legacy macros (deprecated - require explicit class name)
// =============================================================================

/**
 * @deprecated Use CommandHandlerBase with constructor registration instead.
 *
 * Register a command handler method with explicit class name.
 * Works around `decltype(this)` issue at class scope.
 */
#define ANGZARR_HANDLES_FOR(Class, T)                                                         \
    static inline const bool _handles_##T##_registered =                                      \
        (register_handler(ANGZARR_TYPE_SUFFIX(T), &Class::_dispatch_##T), true);              \
    template <typename Self>                                                                  \
    static ::angzarr::EventBook _dispatch_##T(Self* self, const ::google::protobuf::Any& any, \
                                              int seq) {                                      \
        T cmd;                                                                                \
        any.UnpackTo(&cmd);                                                                   \
        auto result = self->handle_##T(cmd);                                                  \
        return ::angzarr::helpers::new_event_book(result);                                    \
    }                                                                                         \
    /* User implements: */ auto handle_##T

/**
 * @deprecated Use CommandHandlerBase with constructor registration instead.
 *
 * Register an event applier method with explicit class name.
 */
#define ANGZARR_APPLIES_FOR(Class, T)                                                      \
    static inline const bool _applies_##T##_registered =                                   \
        (register_applier(ANGZARR_TYPE_SUFFIX(T), &Class::_apply_##T), true);              \
    template <typename Self, typename State>                                               \
    static void _apply_##T(Self* self, State& state, const ::google::protobuf::Any& any) { \
        T event;                                                                           \
        any.UnpackTo(&event);                                                              \
        self->apply_##T(state, event);                                                     \
    }                                                                                      \
    /* User implements: */ void apply_##T

/**
 * @deprecated Use CommandHandlerBase with constructor registration instead.
 *
 * Register a rejection handler with explicit class name.
 */
#define ANGZARR_REJECTED_FOR(Class, target_domain, command_type)      \
    static inline const bool _rejected_##command_type##_registered =  \
        (register_rejection_handler(                                  \
             target_domain "/" ANGZARR_TYPE_SUFFIX(command_type),     \
             &Class::handle_rejected_##command_type),                 \
         true);                                                       \
    /* User implements: */ auto handle_rejected_##command_type

// =============================================================================
// Broken legacy macros (kept for documentation, DO NOT USE)
// =============================================================================

// NOTE: The following macros use `decltype(this)` which is invalid at class
// scope in C++. They are kept here as documentation of what NOT to do.
// Use ANGZARR_HANDLES_FOR(ClassName, Type) instead, or better yet, use
// the CommandHandlerBase CRTP pattern with constructor registration.

#if 0  // DISABLED - these macros do not compile

#define ANGZARR_HANDLES(T)  // BROKEN: uses decltype(this) at class scope
#define ANGZARR_APPLIES(T)  // BROKEN: uses decltype(this) at class scope
#define ANGZARR_REACTS_TO(T)  // BROKEN: uses decltype(this) at class scope
#define ANGZARR_PREPARES(T)  // BROKEN: uses decltype(this) at class scope
#define ANGZARR_PROJECTS(T)  // BROKEN: uses decltype(this) at class scope
#define ANGZARR_REJECTED(domain, cmd)  // BROKEN: uses decltype(this) at class scope

#endif
