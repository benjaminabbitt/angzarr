#pragma once

#include <chrono>
#include <iomanip>
#include <iostream>
#include <nlohmann/json.hpp>
#include <sstream>
#include <string>

namespace angzarr {

inline std::string now_iso8601() {
    auto now = std::chrono::system_clock::now();
    auto time_t = std::chrono::system_clock::to_time_t(now);
    std::stringstream ss;
    ss << std::put_time(std::gmtime(&time_t), "%FT%TZ");
    return ss.str();
}

inline void log_info(const std::string& domain, const std::string& message,
                     const nlohmann::json& fields = {}) {
    nlohmann::json log_entry = {
        {"level", "info"},
        {"message", message},
        {"domain", domain},
        {"timestamp", now_iso8601()}
    };
    for (auto& [key, value] : fields.items()) {
        log_entry[key] = value;
    }
    std::cout << log_entry.dump() << std::endl;
}

}  // namespace angzarr
