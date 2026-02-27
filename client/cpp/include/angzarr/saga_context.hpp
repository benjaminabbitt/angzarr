#pragma once

#include <string>
#include <unordered_map>
#include <vector>

#include "angzarr/types.pb.h"

namespace angzarr {

/**
 * Context for saga handlers, providing access to destination aggregate state.
 *
 * Used in the splitter pattern where one event triggers commands to multiple aggregates.
 * Provides sequence number lookup for optimistic concurrency control.
 *
 * Example usage:
 * @code
 * std::vector<CommandBook> handle_table_settled(
 *     const TableSettled& evt,
 *     const SagaContext& ctx
 * ) {
 *     std::vector<CommandBook> commands;
 *     for (const auto& payout : evt.payouts()) {
 *         uint32_t seq = ctx.get_sequence("player", payout.player_root());
 *         // Build command with sequence...
 *     }
 *     return commands;
 * }
 * @endcode
 */
class SagaContext {
   public:
    /**
     * Create a context from a list of destination EventBooks.
     * @param destination_books EventBooks fetched during prepare phase.
     */
    explicit SagaContext(const std::vector<EventBook>& destination_books);

    /**
     * Create a context from a repeated field of EventBooks.
     */
    template <typename Container>
    explicit SagaContext(const Container& destination_books) {
        for (const auto& book : destination_books) {
            add_destination(book);
        }
    }

    /**
     * Get the next sequence number for a destination aggregate.
     * Returns 1 if the aggregate doesn't exist yet.
     * @param domain The domain of the target aggregate.
     * @param aggregate_root The root identifier as bytes.
     * @return The next sequence number for the aggregate.
     */
    uint32_t get_sequence(const std::string& domain, const std::string& aggregate_root) const;

    /**
     * Get the EventBook for a destination aggregate, if available.
     * @param domain The domain of the target aggregate.
     * @param aggregate_root The root identifier as bytes.
     * @return Pointer to the EventBook if found, nullptr otherwise.
     */
    const EventBook* get_destination(const std::string& domain,
                                     const std::string& aggregate_root) const;

    /**
     * Check if a destination exists.
     * @param domain The domain of the target aggregate.
     * @param aggregate_root The root identifier as bytes.
     * @return True if the destination exists.
     */
    bool has_destination(const std::string& domain, const std::string& aggregate_root) const;

   private:
    void add_destination(const EventBook& book);
    static std::string make_key(const std::string& domain, const std::string& root);

    std::unordered_map<std::string, EventBook> destinations_;
};

}  // namespace angzarr
