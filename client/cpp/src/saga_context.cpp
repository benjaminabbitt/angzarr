#include "angzarr/saga_context.hpp"

#include <iomanip>
#include <sstream>

namespace angzarr {

SagaContext::SagaContext(const std::vector<EventBook>& destination_books) {
    for (const auto& book : destination_books) {
        add_destination(book);
    }
}

void SagaContext::add_destination(const EventBook& book) {
    if (book.has_cover() && !book.cover().domain().empty()) {
        std::string root_bytes;
        if (book.cover().has_root()) {
            root_bytes = book.cover().root().value();
        }
        std::string key = make_key(book.cover().domain(), root_bytes);
        destinations_[key] = book;
    }
}

uint32_t SagaContext::get_sequence(const std::string& domain,
                                   const std::string& aggregate_root) const {
    std::string key = make_key(domain, aggregate_root);
    auto it = destinations_.find(key);
    if (it == destinations_.end() || it->second.pages_size() == 0) {
        return 1;
    }
    const auto& last_page = it->second.pages(it->second.pages_size() - 1);
    return last_page.sequence() + 1;
}

const EventBook* SagaContext::get_destination(const std::string& domain,
                                              const std::string& aggregate_root) const {
    std::string key = make_key(domain, aggregate_root);
    auto it = destinations_.find(key);
    return it != destinations_.end() ? &it->second : nullptr;
}

bool SagaContext::has_destination(const std::string& domain,
                                  const std::string& aggregate_root) const {
    std::string key = make_key(domain, aggregate_root);
    return destinations_.find(key) != destinations_.end();
}

std::string SagaContext::make_key(const std::string& domain, const std::string& root) {
    std::ostringstream ss;
    ss << domain << ":";
    for (unsigned char c : root) {
        ss << std::hex << std::setfill('0') << std::setw(2) << static_cast<int>(c);
    }
    return ss.str();
}

}  // namespace angzarr
