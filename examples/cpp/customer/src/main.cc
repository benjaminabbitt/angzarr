#include "logging.hpp"
#include "angzarr.grpc.pb.h"
#include <grpcpp/grpcpp.h>
#include <grpcpp/health_check_service_interface.h>
#include <cstdlib>
#include <memory>
#include <string>

namespace customer {
std::unique_ptr<angzarr::BusinessLogic::Service> create_customer_service();
}

int main(int argc, char** argv) {
    const char* port_env = std::getenv("PORT");
    std::string port = port_env ? port_env : "51000";
    std::string server_address = "0.0.0.0:" + port;

    grpc::EnableDefaultHealthCheckService(true);

    auto service = customer::create_customer_service();

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(service.get());

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());

    angzarr::log_info("customer", "business_logic_server_started", {{"port", port}});

    server->Wait();

    return 0;
}
