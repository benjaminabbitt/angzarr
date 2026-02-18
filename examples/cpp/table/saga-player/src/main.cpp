#include <iostream>
#include <memory>
#include <string>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <google/protobuf/any.pb.h>

#include "angzarr/saga.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/table.pb.h"
#include "examples/player.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50413;
constexpr const char* SAGA_NAME = "saga-table-player";
constexpr const char* INPUT_DOMAIN = "table";
constexpr const char* OUTPUT_DOMAIN = "player";

/// gRPC service implementation for table-player saga.
class TablePlayerSagaService final : public angzarr::SagaService::Service {
public:
    grpc::Status GetDescriptor(
        grpc::ServerContext* context,
        const angzarr::GetDescriptorRequest* request,
        angzarr::ComponentDescriptor* response) override {

        response->set_name(SAGA_NAME);
        response->set_component_type("saga");

        auto* input = response->add_inputs();
        input->set_domain(INPUT_DOMAIN);
        input->add_types("HandEnded");

        // Note: outputs not tracked in ComponentDescriptor

        return grpc::Status::OK;
    }

    grpc::Status Prepare(
        grpc::ServerContext* context,
        const angzarr::SagaPrepareRequest* request,
        angzarr::SagaPrepareResponse* response) override {

        // Find HandEnded event and extract player roots
        const auto& source = request->source();
        for (const auto& page : source.pages()) {
            const auto& event_any = page.event();
            if (event_any.type_url().find("HandEnded") != std::string::npos) {
                examples::HandEnded event;
                event_any.UnpackTo(&event);

                // Add each player in stack_changes as a destination
                for (const auto& [player_hex, stack_change] : event.stack_changes()) {
                    auto* dest = response->add_destinations();
                    dest->set_domain(OUTPUT_DOMAIN);

                    // Convert hex string to bytes
                    std::string bytes;
                    for (size_t i = 0; i < player_hex.length(); i += 2) {
                        bytes += static_cast<char>(std::stoi(player_hex.substr(i, 2), nullptr, 16));
                    }
                    dest->mutable_root()->set_value(bytes);
                }
                break;
            }
        }

        return grpc::Status::OK;
    }

    grpc::Status Execute(
        grpc::ServerContext* context,
        const angzarr::SagaExecuteRequest* request,
        angzarr::SagaResponse* response) override {

        try {
            const auto& source = request->source();

            // Build map from player root to destination EventBook for sequence lookup
            std::map<std::string, const angzarr::EventBook*> dest_map;
            for (const auto& dest : request->destinations()) {
                if (dest.has_cover() && dest.cover().has_root()) {
                    std::string key;
                    for (unsigned char c : dest.cover().root().value()) {
                        char buf[3];
                        snprintf(buf, sizeof(buf), "%02x", c);
                        key += buf;
                    }
                    dest_map[key] = &dest;
                }
            }

            // Find HandEnded event
            for (const auto& page : source.pages()) {
                const auto& event_any = page.event();
                if (event_any.type_url().find("HandEnded") != std::string::npos) {
                    examples::HandEnded event;
                    event_any.UnpackTo(&event);

                    // Create ReleaseFunds commands for each player
                    for (const auto& [player_hex, stack_change] : event.stack_changes()) {
                        // Convert hex string to bytes
                        std::string player_bytes;
                        for (size_t i = 0; i < player_hex.length(); i += 2) {
                            player_bytes += static_cast<char>(
                                std::stoi(player_hex.substr(i, 2), nullptr, 16));
                        }

                        // Get sequence from destination state
                        uint64_t dest_seq = 0;
                        auto it = dest_map.find(player_hex);
                        if (it != dest_map.end() && it->second->pages_size() > 0) {
                            dest_seq = it->second->pages(it->second->pages_size() - 1).num() + 1;
                        }

                        // Create ReleaseFunds command
                        examples::ReleaseFunds release_funds;
                        release_funds.set_table_root(event.hand_root());

                        // Build command book
                        auto* cmd_book = response->add_commands();
                        auto* cover = cmd_book->mutable_cover();
                        cover->set_domain(OUTPUT_DOMAIN);
                        cover->mutable_root()->set_value(player_bytes);
                        cover->set_correlation_id(source.cover().correlation_id());

                        auto* cmd_page = cmd_book->add_pages();
                        cmd_page->set_sequence(dest_seq);
                        cmd_page->mutable_command()->PackFrom(release_funds);
                    }

                    break;
                }
            }

            return grpc::Status::OK;
        } catch (const std::exception& e) {
            return grpc::Status(grpc::StatusCode::INTERNAL, e.what());
        }
    }
};

} // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    TablePlayerSagaService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Table-Player saga server listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
