#include "logging.hpp"
#include "angzarr.grpc.pb.h"
#include "domains.pb.h"
#include <grpcpp/grpcpp.h>

namespace saga_loyalty_earn {

class LoyaltyEarnSagaService final : public angzarr::Saga::Service {
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
        std::string customer_id;
        int32_t points_earned = 0;

        for (const auto& page : request->pages()) {
            if (!page.has_event()) continue;

            const auto& type_url = page.event().type_url();

            if (type_url.find("Delivered") != std::string::npos) {
                angzarr::log_info("saga-loyalty-earn", "delivery_detected_awarding_points", {});
                points_earned = 100;
            } else if (type_url.find("OrderCreated") != std::string::npos) {
                examples::OrderCreated event;
                page.event().UnpackTo(&event);
                customer_id = event.customer_id();
            }
        }

        if (response && points_earned > 0 && !customer_id.empty()) {
            auto* cmd_book = response->add_commands();
            cmd_book->mutable_cover()->set_domain("customer");
            cmd_book->mutable_cover()->set_entity_id(customer_id);

            auto* cmd_page = cmd_book->add_pages();
            cmd_page->set_num(0);

            examples::AddLoyaltyPoints cmd;
            cmd.set_points(points_earned);
            cmd.set_reason("delivery_bonus");
            cmd_page->mutable_command()->PackFrom(cmd);

            angzarr::log_info("saga-loyalty-earn", "awarding_loyalty_points",
                {{"customer_id", customer_id}, {"points", points_earned}});
        }

        return grpc::Status::OK;
    }
};

std::unique_ptr<angzarr::Saga::Service> create_saga_loyalty_earn_service() {
    return std::make_unique<LoyaltyEarnSagaService>();
}

}  // namespace saga_loyalty_earn
