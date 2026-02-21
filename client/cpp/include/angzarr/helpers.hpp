#pragma once

#include <string>
#include <vector>
#include <chrono>
#include <google/protobuf/any.pb.h>
#include <google/protobuf/timestamp.pb.h>
#include "angzarr/types.pb.h"

namespace angzarr {

/**
 * Helper functions for working with Angzarr types.
 */
namespace helpers {

/**
 * Get the domain from an EventBook.
 */
inline std::string domain(const EventBook& book) {
    return book.has_cover() ? book.cover().domain() : "";
}

/**
 * Get the correlation ID from an EventBook.
 */
inline std::string correlation_id(const EventBook& book) {
    return book.has_cover() ? book.cover().correlation_id() : "";
}

/**
 * Check if an EventBook has a correlation ID.
 */
inline bool has_correlation_id(const EventBook& book) {
    return book.has_cover() && !book.cover().correlation_id().empty();
}

/**
 * Get the root UUID from an EventBook.
 */
inline const UUID* root_uuid(const EventBook& book) {
    return book.has_cover() && book.cover().has_root() ? &book.cover().root() : nullptr;
}

/**
 * Get the root UUID as hex string from an EventBook.
 */
std::string root_id_hex(const EventBook& book);

/**
 * Calculate the next sequence number from an EventBook.
 */
inline int next_sequence(const EventBook* book) {
    if (!book || book->pages_size() == 0) return 0;
    return book->pages_size();
}

/**
 * Extract the type name from a type URL.
 */
inline std::string type_name_from_url(const std::string& type_url) {
    auto pos = type_url.rfind('/');
    return pos != std::string::npos ? type_url.substr(pos + 1) : type_url;
}

constexpr const char* TYPE_URL_PREFIX = "type.googleapis.com/";

/**
 * Check if a type URL matches the given fully qualified type name.
 * @param type_url Full type URL (e.g., "type.googleapis.com/examples.CardsDealt")
 * @param type_name Fully qualified type name (e.g., "examples.CardsDealt")
 * @return true if type_url equals TYPE_URL_PREFIX + type_name
 */
inline bool type_url_matches(const std::string& type_url, const std::string& type_name) {
    return type_url == std::string(TYPE_URL_PREFIX) + type_name;
}

/**
 * Get the current timestamp as a protobuf Timestamp.
 */
google::protobuf::Timestamp now();

/**
 * Pack a protobuf message into an Any.
 */
template<typename T>
google::protobuf::Any pack_any(const T& message) {
    google::protobuf::Any any;
    any.PackFrom(message, "type.googleapis.com/");
    return any;
}

/**
 * Pack an event into an EventPage.
 */
template<typename T>
EventPage pack_event(const T& event_message) {
    EventPage page;
    page.mutable_event()->PackFrom(event_message, "type.googleapis.com/");
    return page;
}

/**
 * Create a new EventBook with the given events.
 */
template<typename... Events>
EventBook new_event_book(const Events&... events) {
    EventBook book;
    (book.add_pages()->CopyFrom(pack_event(events)), ...);
    return book;
}

} // namespace helpers
} // namespace angzarr
