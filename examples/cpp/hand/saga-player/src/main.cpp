#include <iostream>
#include <memory>
#include <string>
#include <map>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <google/protobuf/any.pb.h>

#include "angzarr/saga.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/player.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50414;
constexpr const char* SAGA_NAME = "saga-hand-player";
constexpr const char* INPUT_DOMAIN = "hand";
constexpr const char* OUTPUT_DOMAIN = "player";

/// gRPC service implementation for hand-player saga.
class HandPlayerSagaService final : public angzarr::SagaService::Service {
public:
    grpc::Status GetDescriptor(
        grpc::ServerContext* context,
        const angzarr::GetDescriptorRequest* request,
        angzarr::ComponentDescriptor* response) override {

        response->set_name(SAGA_NAME);
        response->set_component_type("saga");

        auto* input = response->add_inputs();
        input->set_domain(INPUT_DOMAIN);
        input->add_types("PotAwarded");

        // Note: outputs not tracked in ComponentDescriptor

        return grpc::Status::OK;
    }

    grpc::Status Prepare(
        grpc::ServerContext* context,
        const angzarr::SagaPrepareRequest* request,
        angzarr::SagaPrepareResponse* response) override {

        // Find PotAwarded event and extract player roots
        const auto& source = request->source();
        for (const auto& page : source.pages()) {
            const auto& event_any = page.event();
            if (event_any.type_url().find("PotAwarded") != std::string::npos) {
                examples::PotAwarded event;
                event_any.UnpackTo(&event);

                // Add each winner as a destination
                for (const auto& winner : event.winners()) {
                    auto* dest = response->add_destinations();
                    dest->set_domain(OUTPUT_DOMAIN);
                    dest->mutable_root()->set_value(winner.player_root());
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

            // Find PotAwarded event
            for (const auto& page : source.pages()) {
                const auto& event_any = page.event();
                if (event_any.type_url().find("PotAwarded") != std::string::npos) {
                    examples::PotAwarded event;
                    event_any.UnpackTo(&event);

                    // Create DepositFunds commands for each winner
                    for (const auto& winner : event.winners()) {
                        const std::string& player_root = winner.player_root();

                        // Convert player root to hex for map lookup
                        std::string player_hex;
                        for (unsigned char c : player_root) {
                            char buf[3];
                            snprintf(buf, sizeof(buf), "%02x", c);
                            player_hex += buf;
                        }

                        // Get sequence from destination state
                        uint64_t dest_seq = 0;
                        auto it = dest_map.find(player_hex);
                        if (it != dest_map.end() && it->second->pages_size() > 0) {
                            dest_seq = it->second->pages(it->second->pages_size() - 1).num() + 1;
                        }

                        // Create DepositFunds command
                        examples::DepositFunds deposit_funds;
                        deposit_funds.mutable_amount()->set_amount(winner.amount());

                        // Build command book
                        auto* cmd_book = response->add_commands();
                        auto* cover = cmd_book->mutable_cover();
                        cover->set_domain(OUTPUT_DOMAIN);
                        cover->mutable_root()->set_value(player_root);
                        cover->set_correlation_id(source.cover().correlation_id());

                        auto* cmd_page = cmd_book->add_pages();
                        cmd_page->set_sequence(dest_seq);
                        cmd_page->mutable_command()->PackFrom(deposit_funds);
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

    HandPlayerSagaService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Hand-Player saga server listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
