#include "angzarr/upcaster.hpp"

#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <grpcpp/grpcpp.h>

#include <iostream>
#include <string>

namespace angzarr {

void run_upcaster_server(const std::string& name, int port, UpcasterRouter router) {
    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    UpcasterGrpcHandler handler(std::move(router));

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&handler);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Upcaster '" << name << "' listening on " << server_address << std::endl;

    server->Wait();
}

}  // namespace angzarr
