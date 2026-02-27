/**
 * Player domain upcaster gRPC server.
 *
 * Transforms old event versions to current versions during replay.
 * This is a passthrough upcaster - no transformations yet.
 *
 * Adding Transformations:
 *
 * When schema evolution is needed, add transformations to the router:
 *
 *     auto router = UpcasterRouter("player")
 *         .on("PlayerRegisteredV1", [](const Any& old) {
 *             PlayerRegisteredV1 v1;
 *             old.UnpackTo(&v1);
 *
 *             PlayerRegistered v2;
 *             v2.set_display_name(v1.display_name());
 *             v2.set_email(v1.email());
 *             v2.set_player_type(v1.player_type());
 *             v2.set_ai_model_id("");  // New field with default
 *
 *             Any result;
 *             result.PackFrom(v2, "type.googleapis.com/");
 *             return result;
 *         });
 */
#include <google/protobuf/any.pb.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <grpcpp/grpcpp.h>

#include <iostream>
#include <memory>
#include <string>

#include "angzarr/upcaster.grpc.pb.h"
#include "angzarr/upcaster.hpp"

namespace {

constexpr int DEFAULT_PORT = 50402;
constexpr const char* DOMAIN = "player";

// docs:start:upcaster_router
/**
 * Create the upcaster router for player domain.
 *
 * Currently a passthrough - add transformations as needed for schema evolution.
 */
angzarr::UpcasterRouter create_router() {
    return angzarr::UpcasterRouter(DOMAIN);
    // Example transformation (uncomment when needed):
    // .on("PlayerRegisteredV1", [](const google::protobuf::Any& old) {
    //     PlayerRegisteredV1 v1;
    //     old.UnpackTo(&v1);
    //
    //     PlayerRegistered v2;
    //     v2.set_display_name(v1.display_name());
    //     v2.set_email(v1.email());
    //     v2.set_player_type(v1.player_type());
    //     v2.set_ai_model_id("");
    //
    //     google::protobuf::Any result;
    //     result.PackFrom(v2, "type.googleapis.com/");
    //     return result;
    // });
}
// docs:end:upcaster_router

// docs:start:upcaster_service
/**
 * gRPC service for Player domain upcaster.
 */
class PlayerUpcasterService final : public angzarr::UpcasterService::Service {
   public:
    explicit PlayerUpcasterService(angzarr::UpcasterRouter router) : router_(std::move(router)) {}

    grpc::Status Upcast(grpc::ServerContext* context, const angzarr::UpcastRequest* request,
                        angzarr::UpcastResponse* response) override {
        std::vector<angzarr::EventPage> events(request->events().begin(), request->events().end());

        auto transformed = router_.upcast(events);

        for (const auto& page : transformed) {
            *response->add_events() = page;
        }

        return grpc::Status::OK;
    }

   private:
    angzarr::UpcasterRouter router_;
};
// docs:end:upcaster_service

}  // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (const char* env_port = std::getenv("GRPC_PORT")) {
        port = std::stoi(env_port);
    } else if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    auto router = create_router();
    PlayerUpcasterService service(std::move(router));

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Player upcaster listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
