#include "cart_logic.hpp"
#include "logging.hpp"
#include "validation_error.hpp"
#include "angzarr.grpc.pb.h"
#include <grpcpp/grpcpp.h>

namespace cart {

class CartService final : public angzarr::BusinessLogic::Service {
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

            auto state = CartLogic::rebuild_state(prior_events);
            const auto& type_url = cmd_page.command().type_url();

            google::protobuf::Any event_any;

            if (type_url.find("CreateCart") != std::string::npos) {
                examples::CreateCart cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("cart", "creating_cart", {{"customer_id", cmd.customer_id()}});
                auto event = CartLogic::handle_create_cart(state, cmd.customer_id());
                event_any.PackFrom(event);
            } else if (type_url.find("AddItem") != std::string::npos) {
                examples::AddItem cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("cart", "adding_item",
                    {{"product_id", cmd.product_id()}, {"quantity", cmd.quantity()}});
                auto event = CartLogic::handle_add_item(
                    state, cmd.product_id(), cmd.quantity(), cmd.price_cents());
                event_any.PackFrom(event);
            } else if (type_url.find("UpdateQuantity") != std::string::npos) {
                examples::UpdateQuantity cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("cart", "updating_quantity",
                    {{"product_id", cmd.product_id()}, {"quantity", cmd.quantity()}});
                auto event = CartLogic::handle_update_quantity(
                    state, cmd.product_id(), cmd.quantity());
                event_any.PackFrom(event);
            } else if (type_url.find("RemoveItem") != std::string::npos) {
                examples::RemoveItem cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("cart", "removing_item", {{"product_id", cmd.product_id()}});
                auto event = CartLogic::handle_remove_item(state, cmd.product_id());
                event_any.PackFrom(event);
            } else if (type_url.find("ApplyCoupon") != std::string::npos) {
                examples::ApplyCoupon cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("cart", "applying_coupon", {{"coupon_code", cmd.coupon_code()}});
                auto event = CartLogic::handle_apply_coupon(
                    state, cmd.coupon_code(), cmd.discount_cents());
                event_any.PackFrom(event);
            } else if (type_url.find("ClearCart") != std::string::npos) {
                angzarr::log_info("cart", "clearing_cart", {});
                auto event = CartLogic::handle_clear_cart(state);
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

std::unique_ptr<angzarr::BusinessLogic::Service> create_cart_service() {
    return std::make_unique<CartService>();
}

}  // namespace cart
