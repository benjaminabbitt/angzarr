#include "fulfillment_logic.hpp"
#include "logging.hpp"
#include "validation_error.hpp"
#include "angzarr.grpc.pb.h"
#include <grpcpp/grpcpp.h>

namespace fulfillment {

class FulfillmentService final : public angzarr::BusinessLogic::Service {
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

            auto state = FulfillmentLogic::rebuild_state(prior_events);
            const auto& type_url = cmd_page.command().type_url();

            google::protobuf::Any event_any;

            if (type_url.find("CreateShipment") != std::string::npos) {
                examples::CreateShipment cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("fulfillment", "creating_shipment",
                    {{"order_id", cmd.order_id()}, {"item_count", static_cast<int>(cmd.items_size())}});
                std::vector<FulfillmentItem> items;
                for (const auto& item : cmd.items()) {
                    items.push_back({item.product_id(), item.quantity()});
                }
                auto event = FulfillmentLogic::handle_create_shipment(state, cmd.order_id(), items);
                event_any.PackFrom(event);
            } else if (type_url.find("MarkPicked") != std::string::npos) {
                angzarr::log_info("fulfillment", "marking_picked", {});
                auto event = FulfillmentLogic::handle_mark_picked(state);
                event_any.PackFrom(event);
            } else if (type_url.find("MarkPacked") != std::string::npos) {
                angzarr::log_info("fulfillment", "marking_packed", {});
                auto event = FulfillmentLogic::handle_mark_packed(state);
                event_any.PackFrom(event);
            } else if (type_url.find("Ship") != std::string::npos && type_url.find("Shipment") == std::string::npos) {
                examples::Ship cmd;
                cmd_page.command().UnpackTo(&cmd);
                angzarr::log_info("fulfillment", "shipping",
                    {{"tracking_number", cmd.tracking_number()}, {"carrier", cmd.carrier()}});
                auto event = FulfillmentLogic::handle_ship(
                    state, cmd.tracking_number(), cmd.carrier());
                event_any.PackFrom(event);
            } else if (type_url.find("RecordDelivery") != std::string::npos) {
                angzarr::log_info("fulfillment", "recording_delivery", {});
                auto event = FulfillmentLogic::handle_record_delivery(state);
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

std::unique_ptr<angzarr::BusinessLogic::Service> create_fulfillment_service() {
    return std::make_unique<FulfillmentService>();
}

}  // namespace fulfillment
