#include <iostream>
#include <memory>
#include <string>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>

#include "hand_process.hpp"
#include "angzarr/process_manager.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/table.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50491;
constexpr const char* PM_NAME = "pmg-hand-flow";

/// gRPC service implementation for hand-flow process manager.
class HandFlowService final : public angzarr::ProcessManagerService::Service {
public:
    HandFlowService() : manager_([this](const angzarr::CommandBook& cmd) {
        // Command sender callback - would forward to aggregate coordinator
        std::cout << "Would send command to domain: " << cmd.cover().domain() << std::endl;
    }) {}

    grpc::Status GetDescriptor(
        grpc::ServerContext* context,
        const angzarr::GetDescriptorRequest* request,
        angzarr::ComponentDescriptor* response) override {

        response->set_name(PM_NAME);
        response->set_component_type("process_manager");

        // Subscribes to hand domain events
        auto* input = response->add_inputs();
        input->set_domain("hand");
        input->add_types("HandStarted");
        input->add_types("CardsDealt");
        input->add_types("BlindPosted");
        input->add_types("ActionTaken");
        input->add_types("CommunityCardsDealt");
        input->add_types("ShowdownStarted");
        input->add_types("PotAwarded");

        // Also subscribes to table events
        auto* table_input = response->add_inputs();
        table_input->set_domain("table");
        table_input->add_types("HandStarted");

        return grpc::Status::OK;
    }

    grpc::Status Prepare(
        grpc::ServerContext* context,
        const angzarr::ProcessManagerPrepareRequest* request,
        angzarr::ProcessManagerPrepareResponse* response) override {
        // No additional destinations needed beyond trigger and process state
        return grpc::Status::OK;
    }

    grpc::Status Handle(
        grpc::ServerContext* context,
        const angzarr::ProcessManagerHandleRequest* request,
        angzarr::ProcessManagerHandleResponse* response) override {

        // Process each event in the trigger
        for (const auto& page : request->trigger().pages()) {
            const auto& event_any = page.event();
            const std::string& type_url = event_any.type_url();

            std::optional<angzarr::CommandBook> cmd;

            if (type_url.find("HandStarted") != std::string::npos) {
                examples::HandStarted event;
                if (event_any.UnpackTo(&event)) {
                    cmd = manager_.start_hand(event);
                }
            } else if (type_url.find("CardsDealt") != std::string::npos) {
                examples::CardsDealt event;
                if (event_any.UnpackTo(&event)) {
                    cmd = manager_.handle_cards_dealt(event);
                }
            } else if (type_url.find("BlindPosted") != std::string::npos) {
                examples::BlindPosted event;
                if (event_any.UnpackTo(&event)) {
                    cmd = manager_.handle_blind_posted(event);
                }
            } else if (type_url.find("ActionTaken") != std::string::npos) {
                examples::ActionTaken event;
                if (event_any.UnpackTo(&event)) {
                    cmd = manager_.handle_action_taken(event);
                }
            } else if (type_url.find("CommunityCardsDealt") != std::string::npos) {
                examples::CommunityCardsDealt event;
                if (event_any.UnpackTo(&event)) {
                    cmd = manager_.handle_community_cards_dealt(event);
                }
            } else if (type_url.find("ShowdownStarted") != std::string::npos) {
                examples::ShowdownStarted event;
                if (event_any.UnpackTo(&event)) {
                    cmd = manager_.handle_showdown_started(event);
                }
            } else if (type_url.find("PotAwarded") != std::string::npos) {
                examples::PotAwarded event;
                if (event_any.UnpackTo(&event)) {
                    manager_.handle_pot_awarded(event);
                }
            }

            if (cmd.has_value()) {
                *response->add_commands() = cmd.value();
            }
        }

        return grpc::Status::OK;
    }

private:
    hand_flow::HandProcessManager manager_;
};

} // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    HandFlowService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Hand-Flow process manager listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
