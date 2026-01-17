#include "customer_logic.hpp"
#include "logging.hpp"
#include "validation_error.hpp"
#include "angzarr.grpc.pb.h"
#include <grpcpp/grpcpp.h>

namespace customer {

class CustomerService final : public angzarr::BusinessLogic::Service {
public:
    grpc::Status Handle(grpc::ServerContext* context,
                        const angzarr::ContextualCommand* request,
                        angzarr::BusinessResponse* response) override {
        try {
            const auto& cmd_book = request->command();
            const auto* prior_events = request->has_events() ? &request->events() : nullptr;

            if (cmd_book.pages().empty()) {
                return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "CommandBook has no pages");
            }

            const auto& cmd_page = cmd_book.pages(0);
            if (!cmd_page.has_command()) {
                return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "Command page has no command");
            }

            auto state = CustomerLogic::rebuild_state(prior_events);
            const auto& type_url = cmd_page.command().type_url();

            google::protobuf::Any event_any;

            if (type_url.find("CreateCustomer") != std::string::npos) {
                examples::CreateCustomer cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("customer", "creating_customer",
                    {{"name", cmd.name()}, {"email", cmd.email()}});
                auto event = CustomerLogic::handle_create_customer(state, cmd.name(), cmd.email());
                event_any.PackFrom(event);
            } else if (type_url.find("AddLoyaltyPoints") != std::string::npos) {
                examples::AddLoyaltyPoints cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("customer", "adding_loyalty_points",
                    {{"points", cmd.points()}, {"reason", cmd.reason()}});
                auto event = CustomerLogic::handle_add_loyalty_points(state, cmd.points(), cmd.reason());
                event_any.PackFrom(event);
            } else if (type_url.find("RedeemLoyaltyPoints") != std::string::npos) {
                examples::RedeemLoyaltyPoints cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("customer", "redeeming_loyalty_points",
                    {{"points", cmd.points()}, {"redemption_type", cmd.redemption_type()}});
                auto event = CustomerLogic::handle_redeem_loyalty_points(
                    state, cmd.points(), cmd.redemption_type());
                event_any.PackFrom(event);
            } else {
                return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT,
                    "Unknown command type: " + type_url);
            }

            auto* event_book = response->mutable_events();
            if (cmd_book.has_cover()) {
                *event_book->mutable_cover() = cmd_book.cover();
            }

            auto* event_page = event_book->add_pages();
            event_page->set_num(0);
            *event_page->mutable_event() = event_any;
            event_page->mutable_created_at()->set_seconds(
                std::chrono::system_clock::to_time_t(std::chrono::system_clock::now()));

            return grpc::Status::OK;

        } catch (const angzarr::ValidationError& e) {
            return e.to_grpc_status();
        }
    }
};

std::unique_ptr<angzarr::BusinessLogic::Service> create_customer_service() {
    return std::make_unique<CustomerService>();
}

}  // namespace customer
