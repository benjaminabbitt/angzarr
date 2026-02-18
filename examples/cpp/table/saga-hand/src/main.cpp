#include <iostream>
#include <memory>
#include <string>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <google/protobuf/any.pb.h>

#include "table_hand_saga.hpp"
#include "angzarr/saga.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/table.pb.h"
#include "examples/hand.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50411;
constexpr const char* SAGA_NAME = "saga-table-hand";
constexpr const char* INPUT_DOMAIN = "table";
constexpr const char* OUTPUT_DOMAIN = "hand";

/// gRPC service implementation for table-hand saga.
class TableHandSagaService final : public angzarr::SagaService::Service {
public:
    grpc::Status GetDescriptor(
        grpc::ServerContext* context,
        const angzarr::GetDescriptorRequest* request,
        angzarr::ComponentDescriptor* response) override {

        response->set_name(SAGA_NAME);
        response->set_component_type("saga");

        auto* input = response->add_inputs();
        input->set_domain(INPUT_DOMAIN);
        input->add_types("HandStarted");

        // Note: outputs not tracked in ComponentDescriptor

        return grpc::Status::OK;
    }

    grpc::Status Prepare(
        grpc::ServerContext* context,
        const angzarr::SagaPrepareRequest* request,
        angzarr::SagaPrepareResponse* response) override {

        // Table-hand saga doesn't need to read destination state
        // Hand is created fresh, no prior state needed
        return grpc::Status::OK;
    }

    grpc::Status Execute(
        grpc::ServerContext* context,
        const angzarr::SagaExecuteRequest* request,
        angzarr::SagaResponse* response) override {

        try {
            const auto& source = request->source();

            // Find HandStarted event
            for (const auto& page : source.pages()) {
                const auto& event_any = page.event();
                if (event_any.type_url().find("HandStarted") != std::string::npos) {
                    examples::HandStarted event;
                    event_any.UnpackTo(&event);

                    // Create DealCards command for hand aggregate
                    examples::DealCards deal_cmd;
                    deal_cmd.set_table_root(source.cover().root().value());
                    deal_cmd.set_hand_number(event.hand_number());
                    deal_cmd.set_game_variant(event.game_variant());
                    deal_cmd.set_small_blind(event.small_blind());
                    deal_cmd.set_big_blind(event.big_blind());
                    deal_cmd.set_dealer_position(event.dealer_position());

                    // Convert SeatSnapshot to PlayerInHand
                    for (const auto& seat : event.active_players()) {
                        auto* player = deal_cmd.add_players();
                        player->set_player_root(seat.player_root());
                        player->set_position(seat.position());
                        player->set_stack(seat.stack());
                    }

                    // Build command book
                    auto* cmd_book = response->add_commands();
                    auto* cover = cmd_book->mutable_cover();
                    cover->set_domain(OUTPUT_DOMAIN);
                    cover->mutable_root()->set_value(event.hand_root());
                    cover->set_correlation_id(source.cover().correlation_id());

                    auto* cmd_page = cmd_book->add_pages();
                    cmd_page->mutable_command()->PackFrom(deal_cmd);

                    break;
                }
            }

            return grpc::Status::OK;
        } catch (const std::exception& e) {
            return grpc::Status(grpc::StatusCode::INTERNAL, e.what());
        }
    }
};

} // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    TableHandSagaService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Table-Hand saga server listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
