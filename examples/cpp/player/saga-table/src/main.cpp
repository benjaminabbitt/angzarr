#include <google/protobuf/any.pb.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <grpcpp/grpcpp.h>

#include <iostream>
#include <memory>
#include <string>

#include "angzarr/saga.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/player.pb.h"
#include "examples/table.pb.h"
#include "player_table_saga.hpp"

namespace {

constexpr int DEFAULT_PORT = 50214;

/// gRPC service implementation for player-table saga using EventRouter.
/// Sagas are stateless translators - framework handles sequence stamping.
///
/// Propagates player sit-out/sit-in intent as facts to the table domain.
class PlayerTableSagaService final : public angzarr::SagaService::Service {
   public:
    PlayerTableSagaService() : router_(player::saga::create_player_table_router()) {}

    grpc::Status Handle(grpc::ServerContext* context, const angzarr::SagaHandleRequest* request,
                        angzarr::SagaResponse* response) override {
        (void)context;

        try {
            // Clear any previously emitted facts
            player::saga::clear_emitted_facts();

            // Set source root for handler access
            player::saga::set_source_root(&request->source());

            // Dispatch events through the router - no destinations, framework handles sequences
            std::vector<angzarr::EventBook> destinations;
            auto commands = router_.dispatch(request->source(), destinations);

            // Add commands to response
            for (auto& cmd : commands) {
                *response->add_commands() = std::move(cmd);
            }

            // Add emitted facts to response
            for (auto& fact : player::saga::get_emitted_facts()) {
                *response->add_events() = std::move(fact);
            }

            return grpc::Status::OK;
        } catch (const std::exception& e) {
            return grpc::Status(grpc::StatusCode::INTERNAL, e.what());
        }
    }

   private:
    angzarr::EventRouter router_;
};

}  // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    PlayerTableSagaService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Player-Table saga server listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
