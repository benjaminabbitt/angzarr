#pragma once

#include <memory>
#include <string>
#include <cstdlib>
#include <grpcpp/grpcpp.h>
#include "angzarr/types.pb.h"
#include "angzarr/aggregate.pb.h"
#include "angzarr/aggregate.grpc.pb.h"
#include "angzarr/query.pb.h"
#include "angzarr/query.grpc.pb.h"
#include "errors.hpp"

namespace angzarr {

/**
 * Client for querying aggregate event streams.
 *
 * QueryClient provides read access to aggregate event streams. In event-sourced
 * systems, all state is derived from events. QueryClient enables:
 *
 * - State reconstruction: Fetch events to rebuild aggregate state locally
 * - Audit trails: Read complete history for debugging and compliance
 * - Projections: Feed events to read-model projectors
 * - Testing: Verify events were persisted correctly after commands
 *
 * Example:
 *   auto client = QueryClient::connect("localhost:1310");
 *   Query query;
 *   query.mutable_cover()->set_domain("orders");
 *   auto events = client->get_event_book(query);
 */
class QueryClient {
public:
    /**
     * Connect to an event query service at the given endpoint.
     *
     * @param endpoint Server endpoint (e.g., "localhost:1310")
     * @return Unique pointer to QueryClient
     * @throws ConnectionError if connection fails
     */
    static std::unique_ptr<QueryClient> connect(const std::string& endpoint) {
        auto channel = grpc::CreateChannel(format_endpoint(endpoint),
                                           grpc::InsecureChannelCredentials());
        return std::make_unique<QueryClient>(channel);
    }

    /**
     * Connect using an endpoint from environment variable with fallback.
     *
     * Production deployments use environment variables for configuration.
     * This enables the same binary to run in different environments
     * without code changes.
     *
     * @param env_var Environment variable name
     * @param default_endpoint Fallback endpoint if env var is not set
     * @return Unique pointer to QueryClient
     */
    static std::unique_ptr<QueryClient> from_env(const std::string& env_var,
                                                  const std::string& default_endpoint) {
        const char* endpoint = std::getenv(env_var.c_str());
        return connect(endpoint ? endpoint : default_endpoint);
    }

    /**
     * Create a client from an existing channel.
     *
     * @param channel Shared gRPC channel
     */
    explicit QueryClient(std::shared_ptr<grpc::Channel> channel)
        : stub_(EventQueryService::NewStub(channel)) {}

    /**
     * Query events for an aggregate and return a single EventBook.
     *
     * @param query The query specifying domain, root, and optional filters
     * @return EventBook containing matching events
     * @throws GrpcError if the gRPC call fails
     */
    EventBook get_event_book(const Query& query) {
        EventBook response;
        grpc::ClientContext context;
        auto status = stub_->GetEventBook(&context, query, &response);
        if (!status.ok()) {
            throw GrpcError(status.error_message(), status.error_code());
        }
        return response;
    }

    /**
     * Query events and return all matching EventBooks.
     *
     * Uses streaming RPC for bulk retrieval.
     *
     * @param query The query specifying domain, root, and optional filters
     * @return Vector of EventBooks
     * @throws GrpcError if the gRPC call fails
     */
    std::vector<EventBook> get_events(const Query& query) {
        std::vector<EventBook> results;
        grpc::ClientContext context;
        auto reader = stub_->GetEvents(&context, query);
        EventBook book;
        while (reader->Read(&book)) {
            results.push_back(std::move(book));
        }
        auto status = reader->Finish();
        if (!status.ok()) {
            throw GrpcError(status.error_message(), status.error_code());
        }
        return results;
    }

private:
    std::unique_ptr<EventQueryService::Stub> stub_;

    static std::string format_endpoint(const std::string& endpoint) {
        if (endpoint.find("://") == std::string::npos) {
            return endpoint; // Already plain host:port
        }
        // Strip http:// or https:// prefix for gRPC
        auto pos = endpoint.find("://");
        return endpoint.substr(pos + 3);
    }
};

/**
 * Client for sending commands to aggregates through the coordinator.
 *
 * AggregateClient handles command routing, response parsing, and provides
 * multiple execution modes:
 *
 * - Async (fire-and-forget): For high-throughput scenarios
 * - Sync: Wait for persistence, receive resulting events
 * - Speculative: What-if execution without persistence
 *
 * Example:
 *   auto client = AggregateClient::connect("localhost:1310");
 *   CommandBook cmd;
 *   // ... build command ...
 *   auto response = client->handle(cmd);
 */
class AggregateClient {
public:
    /**
     * Connect to an aggregate coordinator at the given endpoint.
     *
     * @param endpoint Server endpoint (e.g., "localhost:1310")
     * @return Unique pointer to AggregateClient
     * @throws ConnectionError if connection fails
     */
    static std::unique_ptr<AggregateClient> connect(const std::string& endpoint) {
        auto channel = grpc::CreateChannel(format_endpoint(endpoint),
                                           grpc::InsecureChannelCredentials());
        return std::make_unique<AggregateClient>(channel);
    }

    /**
     * Connect using an endpoint from environment variable with fallback.
     *
     * @param env_var Environment variable name
     * @param default_endpoint Fallback endpoint if env var is not set
     * @return Unique pointer to AggregateClient
     */
    static std::unique_ptr<AggregateClient> from_env(const std::string& env_var,
                                                      const std::string& default_endpoint) {
        const char* endpoint = std::getenv(env_var.c_str());
        return connect(endpoint ? endpoint : default_endpoint);
    }

    /**
     * Create a client from an existing channel.
     *
     * @param channel Shared gRPC channel
     */
    explicit AggregateClient(std::shared_ptr<grpc::Channel> channel)
        : stub_(AggregateCoordinatorService::NewStub(channel)) {}

    /**
     * Execute a command asynchronously (fire-and-forget).
     *
     * Returns immediately after the coordinator accepts the command.
     * The command is guaranteed to be processed, but the client doesn't wait.
     *
     * @param command The command to execute
     * @return CommandResponse indicating acceptance
     * @throws GrpcError if the gRPC call fails
     */
    CommandResponse handle(const CommandBook& command) {
        CommandResponse response;
        grpc::ClientContext context;
        auto status = stub_->Handle(&context, command, &response);
        if (!status.ok()) {
            throw GrpcError(status.error_message(), status.error_code());
        }
        return response;
    }

    /**
     * Execute a command synchronously.
     *
     * Blocks until the aggregate processes the command and events are persisted.
     * The response includes the resulting events.
     *
     * @param command The sync command to execute
     * @return CommandResponse with resulting events
     * @throws GrpcError if the gRPC call fails
     */
    CommandResponse handle_sync(const SyncCommandBook& command) {
        CommandResponse response;
        grpc::ClientContext context;
        auto status = stub_->HandleSync(&context, command, &response);
        if (!status.ok()) {
            throw GrpcError(status.error_message(), status.error_code());
        }
        return response;
    }

    /**
     * Execute a command speculatively against temporal state (no persistence).
     *
     * Use for form validation, preview, or testing without polluting event store.
     * The aggregate state remains unchanged after speculative execution.
     *
     * @param request The speculative execution request
     * @return CommandResponse with projected events
     * @throws GrpcError if the gRPC call fails
     */
    CommandResponse handle_sync_speculative(const SpeculateAggregateRequest& request) {
        CommandResponse response;
        grpc::ClientContext context;
        auto status = stub_->HandleSyncSpeculative(&context, request, &response);
        if (!status.ok()) {
            throw GrpcError(status.error_message(), status.error_code());
        }
        return response;
    }

private:
    std::unique_ptr<AggregateCoordinatorService::Stub> stub_;

    static std::string format_endpoint(const std::string& endpoint) {
        if (endpoint.find("://") == std::string::npos) {
            return endpoint;
        }
        auto pos = endpoint.find("://");
        return endpoint.substr(pos + 3);
    }
};

/**
 * Combined client for aggregate commands and event queries.
 *
 * DomainClient combines QueryClient and AggregateClient into a single unified
 * interface. This is the recommended entry point for most applications because:
 *
 * - Single connection: One endpoint, one channel, reduced resource usage
 * - Unified API: Both queries and commands through one object
 * - Simpler DI: Inject one client instead of two
 *
 * For advanced use cases (separate scaling, different endpoints), use
 * QueryClient and AggregateClient directly.
 *
 * Example:
 *   auto client = DomainClient::connect("localhost:1310");
 *   // Send a command
 *   auto response = client->aggregate()->handle(cmd);
 *   // Query events
 *   auto events = client->query()->get_event_book(query);
 */
class DomainClient {
public:
    /**
     * Connect to a domain's coordinator at the given endpoint.
     *
     * @param endpoint Server endpoint (e.g., "localhost:1310")
     * @return Unique pointer to DomainClient
     * @throws ConnectionError if connection fails
     */
    static std::unique_ptr<DomainClient> connect(const std::string& endpoint) {
        auto formatted = format_endpoint(endpoint);
        auto channel = grpc::CreateChannel(formatted, grpc::InsecureChannelCredentials());
        return std::make_unique<DomainClient>(channel);
    }

    /**
     * Connect using an endpoint from environment variable with fallback.
     *
     * @param env_var Environment variable name
     * @param default_endpoint Fallback endpoint if env var is not set
     * @return Unique pointer to DomainClient
     */
    static std::unique_ptr<DomainClient> from_env(const std::string& env_var,
                                                   const std::string& default_endpoint) {
        const char* endpoint = std::getenv(env_var.c_str());
        return connect(endpoint ? endpoint : default_endpoint);
    }

    /**
     * Create a client from an existing channel.
     *
     * @param channel Shared gRPC channel
     */
    explicit DomainClient(std::shared_ptr<grpc::Channel> channel)
        : aggregate_(std::make_unique<AggregateClient>(channel))
        , query_(std::make_unique<QueryClient>(channel)) {}

    /**
     * Get the aggregate client for command execution.
     */
    AggregateClient* aggregate() { return aggregate_.get(); }

    /**
     * Get the query client for event retrieval.
     */
    QueryClient* query() { return query_.get(); }

    /**
     * Execute a command (convenience method delegating to aggregate).
     */
    CommandResponse execute(const CommandBook& command) {
        return aggregate_->handle(command);
    }

private:
    std::unique_ptr<AggregateClient> aggregate_;
    std::unique_ptr<QueryClient> query_;

    static std::string format_endpoint(const std::string& endpoint) {
        if (endpoint.find("://") == std::string::npos) {
            return endpoint;
        }
        auto pos = endpoint.find("://");
        return endpoint.substr(pos + 3);
    }
};

} // namespace angzarr
