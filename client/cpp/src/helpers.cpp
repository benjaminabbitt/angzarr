#include "angzarr/helpers.hpp"

namespace angzarr {
namespace helpers {

std::string root_id_hex(const EventBook& book) {
    const UUID* root = root_uuid(book);
    if (!root) return "";

    // Convert binary UUID to hex string
    const std::string& value = root->value();
    std::string hex;
    hex.reserve(value.size() * 2);

    static const char hex_chars[] = "0123456789abcdef";
    for (unsigned char c : value) {
        hex.push_back(hex_chars[c >> 4]);
        hex.push_back(hex_chars[c & 0x0f]);
    }
    return hex;
}

google::protobuf::Timestamp now() {
    auto time_point = std::chrono::system_clock::now();
    auto duration = time_point.time_since_epoch();
    auto seconds = std::chrono::duration_cast<std::chrono::seconds>(duration);
    auto nanos = std::chrono::duration_cast<std::chrono::nanoseconds>(duration - seconds);

    google::protobuf::Timestamp ts;
    ts.set_seconds(seconds.count());
    ts.set_nanos(static_cast<int32_t>(nanos.count()));
    return ts;
}

} // namespace helpers
} // namespace angzarr
