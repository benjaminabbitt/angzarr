#include "inventory_logic.hpp"
#include "logging.hpp"
#include "validation_error.hpp"
#include "angzarr.grpc.pb.h"
#include <grpcpp/grpcpp.h>

namespace inventory {

class InventoryService final : public angzarr::BusinessLogic::Service {
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

            auto state = InventoryLogic::rebuild_state(prior_events);
            const auto& type_url = cmd_page.command().type_url();

            google::protobuf::Any event_any;

            if (type_url.find("InitializeStock") != std::string::npos) {
                examples::InitializeStock cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("inventory", "initializing_stock",
                    {{"product_id", cmd.product_id()}, {"quantity", cmd.initial_quantity()}});
                auto event = InventoryLogic::handle_initialize_stock(
                    state, cmd.product_id(), cmd.initial_quantity());
                event_any.PackFrom(event);
            } else if (type_url.find("ReceiveStock") != std::string::npos) {
                examples::ReceiveStock cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("inventory", "receiving_stock", {{"quantity", cmd.quantity()}});
                auto event = InventoryLogic::handle_receive_stock(
                    state, cmd.quantity(), cmd.reference());
                event_any.PackFrom(event);
            } else if (type_url.find("ReserveStock") != std::string::npos) {
                examples::ReserveStock cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("inventory", "reserving_stock",
                    {{"order_id", cmd.order_id()}, {"quantity", cmd.quantity()}});
                auto event = InventoryLogic::handle_reserve_stock(
                    state, cmd.order_id(), cmd.quantity());
                event_any.PackFrom(event);
            } else if (type_url.find("ReleaseReservation") != std::string::npos) {
                examples::ReleaseReservation cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("inventory", "releasing_reservation", {{"order_id", cmd.order_id()}});
                auto event = InventoryLogic::handle_release_reservation(state, cmd.order_id());
                event_any.PackFrom(event);
            } else if (type_url.find("CommitReservation") != std::string::npos) {
                examples::CommitReservation cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("inventory", "committing_reservation", {{"order_id", cmd.order_id()}});
                auto event = InventoryLogic::handle_commit_reservation(state, cmd.order_id());
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

std::unique_ptr<angzarr::BusinessLogic::Service> create_inventory_service() {
    return std::make_unique<InventoryService>();
}

}  // namespace inventory
