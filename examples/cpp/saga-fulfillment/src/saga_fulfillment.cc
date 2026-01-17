#include "logging.hpp"
#include "angzarr.grpc.pb.h"
#include "domains.pb.h"
#include <grpcpp/grpcpp.h>

namespace saga_fulfillment {

class FulfillmentSagaService final : public angzarr::Saga::Service {
public:
    grpc::Status Handle(grpc::ServerContext* context,
                        const angzarr::EventBook* request,
                        google::protobuf::Empty* response) override {
        return process_events(request, nullptr);
    }

    grpc::Status HandleSync(grpc::ServerContext* context,
                            const angzarr::EventBook* request,
                            angzarr::SagaResponse* response) override {
        return process_events(request, response);
    }

private:
    grpc::Status process_events(const angzarr::EventBook* request, angzarr::SagaResponse* response) {
        for (const auto& page : request->pages()) {
            if (!page.has_event()) continue;

            const auto& type_url = page.event().type_url();

            if (type_url.find("PaymentSubmitted") != std::string::npos) {
                examples::PaymentSubmitted event;
                page.event().UnpackTo(&event);

                angzarr::log_info("saga-fulfillment", "payment_submitted_creating_shipment", {});

                if (response) {
                    auto* cmd_book = response->add_commands();
                    cmd_book->mutable_cover()->set_domain("fulfillment");
                    cmd_book->mutable_cover()->set_entity_id(request->cover().entity_id());

                    auto* cmd_page = cmd_book->add_pages();
                    cmd_page->set_num(0);

                    examples::CreateShipment cmd;
                    cmd.set_order_id(request->cover().entity_id());
                    cmd_page->mutable_command()->PackFrom(cmd);
                }
            }
        }

        return grpc::Status::OK;
    }
};

std::unique_ptr<angzarr::Saga::Service> create_saga_fulfillment_service() {
    return std::make_unique<FulfillmentSagaService>();
}

}  // namespace saga_fulfillment
