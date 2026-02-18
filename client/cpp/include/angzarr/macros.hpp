#pragma once

/**
 * Macros for handler registration in C++ aggregates, sagas, etc.
 *
 * These macros provide a similar feel to Python's decorators by generating
 * static registration code at compile time.
 *
 * Usage:
 *   class Player : public Aggregate<PlayerState> {
 *   public:
 *       ANGZARR_AGGREGATE("player")
 *
 *       ANGZARR_HANDLES(RegisterPlayer)
 *       PlayerRegistered handle_register(const RegisterPlayer& cmd) {
 *           if (exists()) throw CommandRejectedError("Already exists");
 *           return PlayerRegistered{};
 *       }
 *
 *       ANGZARR_APPLIES(PlayerRegistered)
 *       void apply_registered(PlayerState& state, const PlayerRegistered& event) {
 *           state.set_status("active");
 *       }
 *
 *   protected:
 *       PlayerState create_empty_state() override { return PlayerState{}; }
 *   };
 */

// Helper to extract type suffix from proto type
#define ANGZARR_TYPE_SUFFIX(T) #T

/**
 * Declare this class as an aggregate with the given domain name.
 * Must be in public section of class.
 */
#define ANGZARR_AGGREGATE(domain_name) \
    static constexpr const char* kDomain = domain_name; \
    std::string domain() const override { return kDomain; }

/**
 * Declare this class as a saga with the given name and domains.
 * Must be in public section of class.
 */
#define ANGZARR_SAGA(saga_name, in_domain, out_domain) \
    static constexpr const char* kName = saga_name; \
    static constexpr const char* kInputDomain = in_domain; \
    static constexpr const char* kOutputDomain = out_domain; \
    std::string name() const override { return kName; } \
    std::string input_domain() const override { return kInputDomain; } \
    std::string output_domain() const override { return kOutputDomain; }

/**
 * Register a command handler method.
 * The next method definition handles commands of type T.
 */
#define ANGZARR_HANDLES(T) \
    static inline const bool _handles_##T##_registered = \
        (register_handler(ANGZARR_TYPE_SUFFIX(T), &std::remove_pointer_t<decltype(this)>::_dispatch_##T), true); \
    template<typename Self> \
    static ::angzarr::EventBook _dispatch_##T(Self* self, const ::google::protobuf::Any& any, int seq) { \
        T cmd; any.UnpackTo(&cmd); \
        auto result = self->handle_##T(cmd); \
        return ::angzarr::helpers::new_event_book(result); \
    } \
    /* User implements: */ auto handle_##T

/**
 * Register an event applier method.
 * The next method definition applies events of type T.
 */
#define ANGZARR_APPLIES(T) \
    static inline const bool _applies_##T##_registered = \
        (register_applier(ANGZARR_TYPE_SUFFIX(T), &std::remove_pointer_t<decltype(this)>::_apply_##T), true); \
    template<typename Self, typename State> \
    static void _apply_##T(Self* self, State& state, const ::google::protobuf::Any& any) { \
        T event; any.UnpackTo(&event); \
        self->apply_##T(state, event); \
    } \
    /* User implements: */ void apply_##T

/**
 * Register an event handler for sagas.
 * The next method definition handles events of type T.
 */
#define ANGZARR_REACTS_TO(T) \
    static inline const bool _reacts_##T##_registered = \
        (register_event_handler(ANGZARR_TYPE_SUFFIX(T), &std::remove_pointer_t<decltype(this)>::_react_##T), true); \
    template<typename Self> \
    static std::vector<::angzarr::CommandBook> _react_##T( \
        Self* self, const ::google::protobuf::Any& any, const std::string& corr_id) { \
        T event; any.UnpackTo(&event); \
        auto result = self->handle_##T(event); \
        return self->pack_commands(result, corr_id); \
    } \
    /* User implements: */ auto handle_##T

/**
 * Register a prepare handler for two-phase protocol.
 */
#define ANGZARR_PREPARES(T) \
    static inline const bool _prepares_##T##_registered = \
        (register_prepare_handler(ANGZARR_TYPE_SUFFIX(T), &std::remove_pointer_t<decltype(this)>::_prepare_##T), true); \
    template<typename Self> \
    static std::vector<::angzarr::Cover> _prepare_##T(Self* self, const ::google::protobuf::Any& any) { \
        T event; any.UnpackTo(&event); \
        return self->prepare_##T(event); \
    } \
    /* User implements: */ std::vector<::angzarr::Cover> prepare_##T

/**
 * Register a projector handler.
 */
#define ANGZARR_PROJECTS(T) \
    static inline const bool _projects_##T##_registered = \
        (register_projector_handler(ANGZARR_TYPE_SUFFIX(T), &std::remove_pointer_t<decltype(this)>::_project_##T), true); \
    template<typename Self> \
    static ::angzarr::Projection _project_##T(Self* self, const ::google::protobuf::Any& any) { \
        T event; any.UnpackTo(&event); \
        return self->project_##T(event); \
    } \
    /* User implements: */ ::angzarr::Projection project_##T

/**
 * Register a rejection handler for compensation.
 */
#define ANGZARR_REJECTED(target_domain, command_type) \
    static inline const bool _rejected_##command_type##_registered = \
        (register_rejection_handler(target_domain "/" ANGZARR_TYPE_SUFFIX(command_type), \
            &std::remove_pointer_t<decltype(this)>::handle_rejected_##command_type), true); \
    /* User implements: */ auto handle_rejected_##command_type
