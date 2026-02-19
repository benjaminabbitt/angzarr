#include <iostream>
#include <memory>
#include <string>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>

#include "hand_flow_pm.hpp"
#include "angzarr/process_manager.grpc.pb.h"
#include "angzarr/types.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50492;

/// gRPC service implementation for hand-flow-oo process manager.
class HandFlowOOService final : public angzarr::ProcessManagerService::Service {
public:
    HandFlowOOService() : pm_() {}

    grpc::Status Prepare(
        grpc::ServerContext* /* context */,
        const angzarr::ProcessManagerPrepareRequest* request,
        angzarr::ProcessManagerPrepareResponse* response) override {

        auto destinations = pm_.prepare_destinations(request->trigger());
        for (const auto& cover : destinations) {
            *response->add_destinations() = cover;
        }

        return grpc::Status::OK;
    }

    grpc::Status Handle(
        grpc::ServerContext* /* context */,
        const angzarr::ProcessManagerHandleRequest* request,
        angzarr::ProcessManagerHandleResponse* response) override {

        const angzarr::EventBook* prior_events = request->has_process_state()
            ? &request->process_state()
            : nullptr;

        std::vector<angzarr::EventBook> destinations;
        for (const auto& dest : request->destinations()) {
            destinations.push_back(dest);
        }

        auto commands = pm_.dispatch(request->trigger(), prior_events, destinations);
        for (const auto& cmd : commands) {
            *response->add_commands() = cmd;
        }

        return grpc::Status::OK;
    }

private:
    hand_flow_oo::HandFlowPM pm_;
};

} // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    HandFlowOOService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Hand-Flow-OO process manager listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
