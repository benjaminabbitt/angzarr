#include "logging.hpp"
#include "angzarr.grpc.pb.h"
#include <grpcpp/grpcpp.h>
#include <grpcpp/health_check_service_interface.h>
#include <cstdlib>
#include <memory>
#include <string>

namespace saga_cancellation {
std::unique_ptr<angzarr::Saga::Service> create_saga_cancellation_service();
}

int main(int argc, char** argv) {
    const char* port_env = std::getenv("PORT");
    std::string port = port_env ? port_env : "51009";
    std::string server_address = "0.0.0.0:" + port;

    grpc::EnableDefaultHealthCheckService(true);

    auto service = saga_cancellation::create_saga_cancellation_service();

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(service.get());

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());

    angzarr::log_info("saga-cancellation", "saga_server_started", {{"port", port}});

    server->Wait();

    return 0;
}
