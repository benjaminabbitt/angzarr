#include "order_logic.hpp"
#include "logging.hpp"
#include "validation_error.hpp"
#include "angzarr.grpc.pb.h"
#include <grpcpp/grpcpp.h>

namespace order {

class OrderService final : public angzarr::BusinessLogic::Service {
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

            auto state = OrderLogic::rebuild_state(prior_events);
            const auto& type_url = cmd_page.command().type_url();

            google::protobuf::Any event_any;

            if (type_url.find("CreateOrder") != std::string::npos) {
                examples::CreateOrder cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("order", "creating_order",
                    {{"customer_id", cmd.customer_id()}, {"item_count", static_cast<int>(cmd.items_size())}});
                std::vector<OrderItem> items;
                for (const auto& item : cmd.items()) {
                    items.push_back({item.product_id(), item.quantity(), item.price_cents()});
                }
                auto event = OrderLogic::handle_create_order(
                    state, cmd.customer_id(), items, cmd.subtotal_cents(),
                    cmd.discount_cents(), cmd.total_cents());
                event_any.PackFrom(event);
            } else if (type_url.find("ApplyLoyaltyDiscount") != std::string::npos) {
                examples::ApplyLoyaltyDiscount cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("order", "applying_loyalty_discount",
                    {{"points_used", cmd.points_used()}});
                auto event = OrderLogic::handle_apply_loyalty_discount(
                    state, cmd.points_used(), cmd.discount_cents());
                event_any.PackFrom(event);
            } else if (type_url.find("SubmitPayment") != std::string::npos) {
                examples::SubmitPayment cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("order", "submitting_payment",
                    {{"payment_method", cmd.payment_method()}, {"amount_cents", cmd.amount_cents()}});
                auto event = OrderLogic::handle_submit_payment(
                    state, cmd.payment_method(), cmd.payment_reference(), cmd.amount_cents());
                event_any.PackFrom(event);
            } else if (type_url.find("CompleteOrder") != std::string::npos) {
                examples::CompleteOrder cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("order", "completing_order",
                    {{"loyalty_points_earned", cmd.loyalty_points_earned()}});
                auto event = OrderLogic::handle_complete_order(state, cmd.loyalty_points_earned());
                event_any.PackFrom(event);
            } else if (type_url.find("CancelOrder") != std::string::npos) {
                examples::CancelOrder cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("order", "cancelling_order", {{"reason", cmd.reason()}});
                auto event = OrderLogic::handle_cancel_order(state, cmd.reason());
                event_any.PackFrom(event);
            } else {
                return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "Unknown command type: " + type_url);
            }

            auto* event_book = response->mutable_events();
            if (cmd_book.has_cover()) *event_book->mutable_cover() = cmd_book.cover();

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

std::unique_ptr<angzarr::BusinessLogic::Service> create_order_service() {
    return std::make_unique<OrderService>();
}

}  // namespace order
