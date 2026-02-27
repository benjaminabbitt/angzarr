#include "angzarr/cloudevents.hpp"

#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <grpcpp/grpcpp.h>

#include <iostream>
#include <string>

#include "angzarr/projector.grpc.pb.h"

namespace angzarr {
namespace {

/**
 * gRPC service implementation for CloudEvents projector.
 *
 * Wraps a CloudEventsRouter and implements ProjectorService.
 */
class CloudEventsProjectorService final : public ProjectorService::Service {
   public:
    explicit CloudEventsProjectorService(const CloudEventsRouter& router) : router_(router) {}

    grpc::Status Handle(grpc::ServerContext* context, const EventBook* request,
                        Projection* response) override {
        (void)context;

        // Transform events to CloudEvents
        CloudEventsResponse cloud_events = router_.project(*request);

        // Pack CloudEventsResponse into Projection.projection
        response->mutable_cover()->CopyFrom(request->cover());
        response->set_projector(router_.name());
        response->set_sequence(request->next_sequence());
        response->mutable_projection()->PackFrom(cloud_events);

        return grpc::Status::OK;
    }

    grpc::Status HandleSpeculative(grpc::ServerContext* context, const EventBook* request,
                                   Projection* response) override {
        // Same behavior for speculative - just transform events
        return Handle(context, request, response);
    }

   private:
    const CloudEventsRouter& router_;
};

}  // namespace

void run_cloudevents_projector(const std::string& name, int port, const CloudEventsRouter& router) {
    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    CloudEventsProjectorService service(router);

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "CloudEvents projector '" << name << "' listening on " << server_address
              << std::endl;

    server->Wait();
}

}  // namespace angzarr
