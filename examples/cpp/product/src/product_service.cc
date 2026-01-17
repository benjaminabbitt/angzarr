#include "product_logic.hpp"
#include "logging.hpp"
#include "validation_error.hpp"
#include "angzarr.grpc.pb.h"
#include <grpcpp/grpcpp.h>

namespace product {

class ProductService final : public angzarr::BusinessLogic::Service {
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

            auto state = ProductLogic::rebuild_state(prior_events);
            const auto& type_url = cmd_page.command().type_url();

            google::protobuf::Any event_any;

            if (type_url.find("CreateProduct") != std::string::npos) {
                examples::CreateProduct cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("product", "creating_product",
                    {{"sku", cmd.sku()}, {"name", cmd.name()}, {"price_cents", cmd.price_cents()}});
                auto event = ProductLogic::handle_create_product(
                    state, cmd.sku(), cmd.name(), cmd.description(), cmd.price_cents());
                event_any.PackFrom(event);
            } else if (type_url.find("UpdateProduct") != std::string::npos) {
                examples::UpdateProduct cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("product", "updating_product", {{"name", cmd.name()}});
                auto event = ProductLogic::handle_update_product(state, cmd.name(), cmd.description());
                event_any.PackFrom(event);
            } else if (type_url.find("SetPrice") != std::string::npos) {
                examples::SetPrice cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("product", "setting_price", {{"price_cents", cmd.price_cents()}});
                auto event = ProductLogic::handle_set_price(state, cmd.price_cents());
                event_any.PackFrom(event);
            } else if (type_url.find("Discontinue") != std::string::npos) {
                examples::Discontinue cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("product", "discontinuing_product", {{"reason", cmd.reason()}});
                auto event = ProductLogic::handle_discontinue(state, cmd.reason());
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

std::unique_ptr<angzarr::BusinessLogic::Service> create_product_service() {
    return std::make_unique<ProductService>();
}

}  // namespace product
