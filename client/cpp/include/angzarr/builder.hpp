#pragma once

#include <string>
#include <optional>
#include <vector>
#include <google/protobuf/any.pb.h>
#include <google/protobuf/timestamp.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/aggregate.pb.h"
#include "angzarr/query.pb.h"
#include "errors.hpp"
#include "helpers.hpp"
#include "client.hpp"

namespace angzarr {

/**
 * Fluent builder for constructing and executing commands.
 *
 * CommandBuilder reduces boilerplate when creating commands:
 *
 * - Chain method calls instead of nested object construction
 * - Type-safe methods prevent invalid field combinations
 * - Auto-generates correlation IDs when not provided
 * - Build incrementally, execute when ready
 *
 * Example:
 *   auto response = CommandBuilder(client.aggregate(), "orders")
 *       .with_root(order_id)
 *       .with_correlation_id("corr-123")
 *       .with_sequence(5)
 *       .with_command("type.googleapis.com/orders.CreateOrder", create_cmd)
 *       .execute();
 */
class CommandBuilder {
public:
    /**
     * Create a command builder for a domain.
     *
     * @param client The aggregate client to use for execution
     * @param domain The aggregate domain
     */
    CommandBuilder(AggregateClient* client, const std::string& domain)
        : client_(client), domain_(domain), sequence_(0) {}

    /**
     * Set the aggregate root UUID.
     *
     * For existing aggregates, this identifies which instance to target.
     * For new aggregates, omit this to let the coordinator generate one.
     *
     * @param root_bytes UUID as 16-byte array
     * @return Reference to this builder for chaining
     */
    CommandBuilder& with_root(const std::string& root_bytes) {
        root_ = root_bytes;
        return *this;
    }

    /**
     * Set the correlation ID for request tracing.
     *
     * Correlation IDs link related operations across services.
     * If not set, a UUID will be auto-generated on build.
     *
     * @param id The correlation ID
     * @return Reference to this builder for chaining
     */
    CommandBuilder& with_correlation_id(const std::string& id) {
        correlation_id_ = id;
        return *this;
    }

    /**
     * Set the expected sequence number for optimistic locking.
     *
     * The aggregate will reject commands with mismatched sequences,
     * preventing concurrent modification conflicts.
     *
     * @param seq The sequence number (0 for new aggregates)
     * @return Reference to this builder for chaining
     */
    CommandBuilder& with_sequence(uint32_t seq) {
        sequence_ = seq;
        return *this;
    }

    /**
     * Set the command type URL and message.
     *
     * The message is serialized to bytes and wrapped in protobuf Any.
     *
     * @param type_url Fully-qualified type URL (e.g., "type.googleapis.com/orders.CreateOrder")
     * @param message The protobuf command message
     * @return Reference to this builder for chaining
     */
    template<typename T>
    CommandBuilder& with_command(const std::string& type_url, const T& message) {
        type_url_ = type_url;
        payload_ = message.SerializeAsString();
        return *this;
    }

    /**
     * Build the CommandBook without executing.
     *
     * @return The constructed CommandBook
     * @throws InvalidArgumentError if required fields are missing
     */
    CommandBook build() const {
        if (!type_url_.has_value()) {
            throw InvalidArgumentError("command type_url not set");
        }
        if (!payload_.has_value()) {
            throw InvalidArgumentError("command payload not set");
        }

        std::string corr_id = correlation_id_.value_or(generate_uuid());

        CommandBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain(domain_);
        cover->set_correlation_id(corr_id);

        if (root_.has_value()) {
            cover->mutable_root()->set_value(root_.value());
        }

        auto* page = book.add_pages();
        page->set_sequence(sequence_);
        auto* cmd = page->mutable_command();
        cmd->set_type_url(type_url_.value());
        cmd->set_value(payload_.value());

        return book;
    }

    /**
     * Build and execute the command.
     *
     * @return The command response
     * @throws InvalidArgumentError if required fields are missing
     * @throws GrpcError if the gRPC call fails
     */
    CommandResponse execute() {
        auto command = build();
        return client_->handle(command);
    }

private:
    AggregateClient* client_;
    std::string domain_;
    std::optional<std::string> root_;
    std::optional<std::string> correlation_id_;
    uint32_t sequence_;
    std::optional<std::string> type_url_;
    std::optional<std::string> payload_;

    static std::string generate_uuid() {
        // Simple UUID v4 generation (for correlation IDs)
        static const char hex[] = "0123456789abcdef";
        std::string uuid(36, '-');
        uuid[8] = uuid[13] = uuid[18] = uuid[23] = '-';

        for (int i = 0; i < 36; ++i) {
            if (uuid[i] == '-') continue;
            uuid[i] = hex[rand() % 16];
        }
        // Set version (4) and variant (8, 9, a, or b)
        uuid[14] = '4';
        uuid[19] = hex[(rand() % 4) + 8];
        return uuid;
    }
};

/**
 * Fluent builder for constructing and executing event queries.
 *
 * QueryBuilder supports multiple access patterns:
 *
 * - By root: Fetch all events for a specific aggregate instance
 * - By correlation ID: Fetch events across aggregates in a workflow
 * - By sequence range: Fetch specific event windows for pagination
 * - By temporal point: Reconstruct historical state (as-of queries)
 * - By edition: Query from specific schema versions after upcasting
 *
 * Example:
 *   auto events = QueryBuilder(client.query(), "orders")
 *       .with_root(order_id)
 *       .range(10)
 *       .get_event_book();
 */
class QueryBuilder {
public:
    /**
     * Create a query builder for a domain.
     *
     * @param client The query client to use for execution
     * @param domain The aggregate domain
     */
    QueryBuilder(QueryClient* client, const std::string& domain)
        : client_(client), domain_(domain), has_range_(false), has_temporal_(false) {}

    /**
     * Set the aggregate root UUID.
     *
     * @param root_bytes UUID as 16-byte array
     * @return Reference to this builder for chaining
     */
    QueryBuilder& with_root(const std::string& root_bytes) {
        root_ = root_bytes;
        correlation_id_.reset(); // Clear correlation ID when root is set
        return *this;
    }

    /**
     * Query by correlation ID instead of root.
     *
     * Correlation IDs link events across aggregates in a distributed workflow.
     * This enables queries like "show me all events for order workflow corr-456".
     *
     * @param id The correlation ID
     * @return Reference to this builder for chaining
     */
    QueryBuilder& by_correlation_id(const std::string& id) {
        correlation_id_ = id;
        root_.reset(); // Clear root when correlation ID is set
        return *this;
    }

    /**
     * Query events from a specific edition.
     *
     * After upcasting (event schema migration), events exist in multiple editions.
     *
     * @param edition The edition name
     * @return Reference to this builder for chaining
     */
    QueryBuilder& with_edition(const std::string& edition) {
        edition_ = edition;
        return *this;
    }

    /**
     * Query a range of sequences from lower (inclusive).
     *
     * Use for incremental sync: "give me events since sequence N"
     *
     * @param lower The lower bound (inclusive)
     * @return Reference to this builder for chaining
     */
    QueryBuilder& range(uint32_t lower) {
        range_lower_ = lower;
        range_upper_.reset();
        has_range_ = true;
        has_temporal_ = false;
        return *this;
    }

    /**
     * Query a range of sequences with upper bound (inclusive).
     *
     * Use for pagination: fetch events 100-200, then 200-300.
     *
     * @param lower The lower bound (inclusive)
     * @param upper The upper bound (inclusive)
     * @return Reference to this builder for chaining
     */
    QueryBuilder& range_to(uint32_t lower, uint32_t upper) {
        range_lower_ = lower;
        range_upper_ = upper;
        has_range_ = true;
        has_temporal_ = false;
        return *this;
    }

    /**
     * Query state as of a specific sequence number.
     *
     * Essential for debugging: "What was the state when this bug occurred?"
     *
     * @param seq The sequence number
     * @return Reference to this builder for chaining
     */
    QueryBuilder& as_of_sequence(uint32_t seq) {
        temporal_sequence_ = seq;
        has_temporal_ = true;
        has_range_ = false;
        return *this;
    }

    /**
     * Query state as of a specific timestamp (RFC3339 format).
     *
     * Example: "2024-01-15T10:30:00Z"
     *
     * @param rfc3339 The timestamp in RFC3339 format
     * @return Reference to this builder for chaining
     * @throws InvalidTimestampError if timestamp parsing fails
     */
    QueryBuilder& as_of_time(const std::string& rfc3339) {
        // Simple RFC3339 parsing (YYYY-MM-DDTHH:MM:SSZ)
        // Production code should use a proper date library
        temporal_time_ = parse_rfc3339(rfc3339);
        has_temporal_ = true;
        has_range_ = false;
        return *this;
    }

    /**
     * Build the Query without executing.
     *
     * @return The constructed Query
     */
    Query build() const {
        Query query;
        auto* cover = query.mutable_cover();
        cover->set_domain(domain_);

        if (root_.has_value()) {
            cover->mutable_root()->set_value(root_.value());
        }
        if (correlation_id_.has_value()) {
            cover->set_correlation_id(correlation_id_.value());
        }
        if (edition_.has_value()) {
            cover->mutable_edition()->set_name(edition_.value());
        }

        if (has_range_) {
            auto* range = query.mutable_range();
            range->set_lower(range_lower_);
            if (range_upper_.has_value()) {
                range->set_upper(range_upper_.value());
            }
        } else if (has_temporal_) {
            auto* temporal = query.mutable_temporal();
            if (temporal_sequence_.has_value()) {
                temporal->set_as_of_sequence(temporal_sequence_.value());
            } else if (temporal_time_.has_value()) {
                *temporal->mutable_as_of_time() = temporal_time_.value();
            }
        }

        return query;
    }

    /**
     * Execute the query and return a single EventBook.
     *
     * @return The EventBook containing matching events
     * @throws GrpcError if the gRPC call fails
     */
    EventBook get_event_book() {
        auto query = build();
        return client_->get_event_book(query);
    }

    /**
     * Execute the query and return all matching EventBooks.
     *
     * @return Vector of EventBooks
     * @throws GrpcError if the gRPC call fails
     */
    std::vector<EventBook> get_events() {
        auto query = build();
        return client_->get_events(query);
    }

    /**
     * Execute the query and return just the event pages.
     *
     * Convenience method when you only need events, not metadata.
     *
     * @return Vector of EventPages
     * @throws GrpcError if the gRPC call fails
     */
    std::vector<EventPage> get_pages() {
        auto book = get_event_book();
        std::vector<EventPage> pages;
        pages.reserve(book.pages_size());
        for (const auto& page : book.pages()) {
            pages.push_back(page);
        }
        return pages;
    }

private:
    QueryClient* client_;
    std::string domain_;
    std::optional<std::string> root_;
    std::optional<std::string> correlation_id_;
    std::optional<std::string> edition_;

    bool has_range_;
    uint32_t range_lower_ = 0;
    std::optional<uint32_t> range_upper_;

    bool has_temporal_;
    std::optional<uint32_t> temporal_sequence_;
    std::optional<google::protobuf::Timestamp> temporal_time_;

    static google::protobuf::Timestamp parse_rfc3339(const std::string& rfc3339) {
        // Simple RFC3339 parsing for YYYY-MM-DDTHH:MM:SSZ format
        // For production use, consider using absl::Time or std::chrono::parse
        google::protobuf::Timestamp ts;

        if (rfc3339.length() < 20 || rfc3339[10] != 'T') {
            throw InvalidTimestampError("Invalid RFC3339 timestamp: " + rfc3339);
        }

        int year, month, day, hour, minute, second;
        if (sscanf(rfc3339.c_str(), "%d-%d-%dT%d:%d:%d",
                   &year, &month, &day, &hour, &minute, &second) != 6) {
            throw InvalidTimestampError("Invalid RFC3339 timestamp: " + rfc3339);
        }

        // Simplified calculation (assumes UTC, no leap seconds)
        // For accurate conversion, use a proper date library
        int64_t days_since_1970 = 0;

        // Days from years
        for (int y = 1970; y < year; ++y) {
            days_since_1970 += 365;
            if ((y % 4 == 0 && y % 100 != 0) || y % 400 == 0) {
                days_since_1970 += 1;
            }
        }

        // Days from months
        static const int days_in_month[] = {0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
        for (int m = 1; m < month; ++m) {
            days_since_1970 += days_in_month[m];
        }
        // Leap year adjustment for current year
        if (month > 2 && ((year % 4 == 0 && year % 100 != 0) || year % 400 == 0)) {
            days_since_1970 += 1;
        }

        // Add days
        days_since_1970 += day - 1;

        int64_t seconds_since_1970 = days_since_1970 * 86400 + hour * 3600 + minute * 60 + second;
        ts.set_seconds(seconds_since_1970);
        ts.set_nanos(0);

        return ts;
    }
};

} // namespace angzarr
