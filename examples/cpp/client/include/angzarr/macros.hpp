#pragma once

#include <vector>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"

namespace angzarr {
namespace helpers {

/// Pack an event into an EventBook with sequence number.
template<typename T>
EventBook new_event_book(const T& event, int sequence = 1) {
    EventBook book;
    auto* page = book.add_pages();
    page->mutable_event()->PackFrom(event, "type.googleapis.com/");
    page->set_num(sequence);  // Uses oneof sequence.num field
    return book;
}

/// Extract type name suffix from a type_url.
inline std::string type_name_from_url(const std::string& type_url) {
    auto pos = type_url.rfind('/');
    return pos != std::string::npos ? type_url.substr(pos + 1) : type_url;
}

/// Check if a type_url ends with a given suffix.
inline bool type_url_matches(const std::string& type_url, const std::string& suffix) {
    if (suffix.size() > type_url.size()) return false;
    return type_url.compare(type_url.size() - suffix.size(), suffix.size(), suffix) == 0;
}

/// Get next sequence number from an EventBook.
inline int next_sequence(const EventBook* book) {
    if (!book || book->pages_size() == 0) return 1;
    const auto& last_page = book->pages(book->pages_size() - 1);
    // Uses oneof sequence.num field
    return last_page.has_num() ? last_page.num() + 1 : 1;
}

} // namespace helpers

/// Macro to extract type suffix from proto type.
#define ANGZARR_TYPE_SUFFIX(T) #T

/// Declare this class as a process manager.
#define ANGZARR_PROCESS_MANAGER(pm_name) \
    static constexpr const char* kName = pm_name; \
    std::string name() const override { return kName; }

/// Declare this class as an aggregate.
#define ANGZARR_AGGREGATE(domain_name) \
    static constexpr const char* kDomain = domain_name; \
    std::string domain() const override { return kDomain; }

/// Declare this class as a saga.
#define ANGZARR_SAGA(saga_name, in_domain, out_domain) \
    static constexpr const char* kName = saga_name; \
    static constexpr const char* kInputDomain = in_domain; \
    static constexpr const char* kOutputDomain = out_domain; \
    std::string name() const override { return kName; } \
    std::string input_domain() const override { return kInputDomain; } \
    std::string output_domain() const override { return kOutputDomain; }

/// Register a command handler method (for aggregates).
#define ANGZARR_HANDLES(T) \
    static inline const bool _handles_##T##_registered = \
        (register_handler(ANGZARR_TYPE_SUFFIX(T), [](auto* self, const ::google::protobuf::Any& any, int seq) { \
            T cmd; any.UnpackTo(&cmd); \
            auto result = self->handle_##T(cmd); \
            return ::angzarr::helpers::new_event_book(result, seq); \
        }), true); \
    auto handle_##T

/// Register an event applier method.
#define ANGZARR_APPLIES(T) \
    static inline const bool _applies_reg_##T = \
        (register_applier(ANGZARR_TYPE_SUFFIX(T), [](auto* self, auto& state, const ::google::protobuf::Any& any) { \
            T event; any.UnpackTo(&event); \
            self->apply_##T(state, event); \
        }), true); \
    void apply_##T

/// Register an event handler for sagas/process managers.
#define ANGZARR_REACTS_TO(T) \
    static inline const bool _reacts_reg_##T = \
        (register_event_handler(ANGZARR_TYPE_SUFFIX(T), [](auto* self, const ::google::protobuf::Any& any, const std::string& corr_id) { \
            T event; any.UnpackTo(&event); \
            auto result = self->handle_##T(event); \
            return self->pack_commands(result, corr_id); \
        }), true); \
    auto handle_##T

/// Register a prepare handler for two-phase protocol.
#define ANGZARR_PREPARES(T) \
    static inline const bool _prepares_reg_##T = \
        (register_prepare_handler(ANGZARR_TYPE_SUFFIX(T), [](auto* self, const ::google::protobuf::Any& any) { \
            T event; any.UnpackTo(&event); \
            return self->prepare_##T(event); \
        }), true); \
    std::vector<::angzarr::Cover> prepare_##T

/// Register a rejection handler for compensation.
#define ANGZARR_REJECTED(target_domain, command_type) \
    static inline const bool _rejected_##command_type##_registered = \
        (register_rejection_handler(target_domain "/" ANGZARR_TYPE_SUFFIX(command_type), \
            [](auto* self, const ::angzarr::Notification& n, auto& state) { \
                return self->handle_rejected_##command_type(n); \
            }), true); \
    auto handle_rejected_##command_type

} // namespace angzarr
