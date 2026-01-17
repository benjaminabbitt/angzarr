#include "logging.hpp"
#include "angzarr.grpc.pb.h"
#include "domains.pb.h"
#include <grpcpp/grpcpp.h>

namespace saga_cancellation {

class CancellationSagaService final : public angzarr::Saga::Service {
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

            if (type_url.find("OrderCancelled") != std::string::npos) {
                examples::OrderCancelled event;
                page.event().UnpackTo(&event);

                angzarr::log_info("saga-cancellation", "order_cancelled_compensating",
                    {{"loyalty_points_refunded", event.loyalty_points_refunded()}});

                if (response) {
                    if (event.loyalty_points_refunded() > 0) {
                        auto* cmd_book = response->add_commands();
                        cmd_book->mutable_cover()->set_domain("customer");
                        cmd_book->mutable_cover()->set_entity_id(request->cover().entity_id());

                        auto* cmd_page = cmd_book->add_pages();
                        cmd_page->set_num(0);

                        examples::AddLoyaltyPoints cmd;
                        cmd.set_points(event.loyalty_points_refunded());
                        cmd.set_reason("order_cancellation_refund");
                        cmd_page->mutable_command()->PackFrom(cmd);
                    }

                    {
                        auto* cmd_book = response->add_commands();
                        cmd_book->mutable_cover()->set_domain("inventory");
                        cmd_book->mutable_cover()->set_entity_id(request->cover().entity_id());

                        auto* cmd_page = cmd_book->add_pages();
                        cmd_page->set_num(0);

                        examples::ReleaseReservation cmd;
                        cmd.set_order_id(request->cover().entity_id());
                        cmd_page->mutable_command()->PackFrom(cmd);
                    }
                }
            }
        }

        return grpc::Status::OK;
    }
};

std::unique_ptr<angzarr::Saga::Service> create_saga_cancellation_service() {
    return std::make_unique<CancellationSagaService>();
}

}  // namespace saga_cancellation
