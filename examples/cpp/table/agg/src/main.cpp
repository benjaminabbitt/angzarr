#include <iostream>
#include <memory>
#include <string>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <google/protobuf/any.pb.h>
#include <google/protobuf/timestamp.pb.h>

#include "table_state.hpp"
#include "../handlers/create_handler.hpp"
#include "../handlers/join_handler.hpp"
#include "../handlers/leave_handler.hpp"
#include "../handlers/start_hand_handler.hpp"
#include "../handlers/end_hand_handler.hpp"
#include "angzarr/command_router.hpp"
#include "angzarr/aggregate.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/table.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50402;
constexpr const char* TABLE_DOMAIN = "table";

/// Create the table aggregate command router (functional style).
angzarr::CommandRouter<table::TableState> create_router() {
    return angzarr::CommandRouter<table::TableState>(TABLE_DOMAIN, table::TableState::from_event_book)
        .on<examples::CreateTable, examples::TableCreated>(table::handlers::handle_create)
        .on<examples::JoinTable, examples::PlayerJoined>(table::handlers::handle_join)
        .on<examples::LeaveTable, examples::PlayerLeft>(table::handlers::handle_leave)
        .on<examples::StartHand, examples::HandStarted>(table::handlers::handle_start_hand)
        .on<examples::EndHand, examples::HandEnded>(table::handlers::handle_end_hand);
}

/// gRPC service implementation for table aggregate.
class TableAggregateService final : public angzarr::AggregateService::Service {
public:
    explicit TableAggregateService(angzarr::CommandRouter<table::TableState> router)
        : router_(std::move(router)) {}

    grpc::Status GetDescriptor(
        grpc::ServerContext* context,
        const angzarr::GetDescriptorRequest* request,
        angzarr::ComponentDescriptor* response) override {

        response->set_name(TABLE_DOMAIN);
        response->set_component_type("aggregate");

        auto* input = response->add_inputs();
        input->set_domain(TABLE_DOMAIN);
        input->add_types("CreateTable");
        input->add_types("JoinTable");
        input->add_types("LeaveTable");
        input->add_types("StartHand");
        input->add_types("EndHand");

        return grpc::Status::OK;
    }

    grpc::Status Handle(
        grpc::ServerContext* context,
        const angzarr::ContextualCommand* request,
        angzarr::BusinessResponse* response) override {

        try {
            const auto& command_book = request->command();
            const auto& event_book = request->events();

            if (command_book.pages_size() == 0) {
                return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "No command pages");
            }

            const auto& command_page = command_book.pages(0);
            const auto& command_any = command_page.command();
            const std::string& type_url = command_any.type_url();

            table::TableState state = table::TableState::from_event_book(event_book);

            auto* events = response->mutable_events();
            auto* cover = events->mutable_cover();
            cover->CopyFrom(event_book.cover());

            if (type_url.find("CreateTable") != std::string::npos) {
                examples::CreateTable cmd;
                command_any.UnpackTo(&cmd);
                auto event = table::handlers::handle_create(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("JoinTable") != std::string::npos) {
                examples::JoinTable cmd;
                command_any.UnpackTo(&cmd);
                auto event = table::handlers::handle_join(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("LeaveTable") != std::string::npos) {
                examples::LeaveTable cmd;
                command_any.UnpackTo(&cmd);
                auto event = table::handlers::handle_leave(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("StartHand") != std::string::npos) {
                examples::StartHand cmd;
                command_any.UnpackTo(&cmd);
                auto event = table::handlers::handle_start_hand(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("EndHand") != std::string::npos) {
                examples::EndHand cmd;
                command_any.UnpackTo(&cmd);
                auto event = table::handlers::handle_end_hand(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else {
                return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT,
                    "Unknown command type: " + type_url);
            }

            return grpc::Status::OK;
        } catch (const angzarr::CommandRejectedError& e) {
            return grpc::Status(e.status_code, e.what());
        } catch (const std::exception& e) {
            return grpc::Status(grpc::StatusCode::INTERNAL, e.what());
        }
    }

    grpc::Status Replay(
        grpc::ServerContext* context,
        const angzarr::ReplayRequest* request,
        angzarr::ReplayResponse* response) override {

        angzarr::EventBook event_book;
        *event_book.mutable_pages() = {request->events().begin(), request->events().end()};

        table::TableState state = table::TableState::from_event_book(event_book);

        examples::TableState proto_state;
        proto_state.set_table_id(state.table_id);
        proto_state.set_table_name(state.table_name);
        proto_state.set_game_variant(static_cast<examples::GameVariant>(state.game_variant));
        proto_state.set_small_blind(state.small_blind);
        proto_state.set_big_blind(state.big_blind);
        proto_state.set_min_buy_in(state.min_buy_in);
        proto_state.set_max_buy_in(state.max_buy_in);
        proto_state.set_max_players(state.max_players);
        proto_state.set_dealer_position(state.dealer_position);
        proto_state.set_hand_count(state.hand_count);
        proto_state.set_current_hand_root(state.current_hand_root);
        proto_state.set_status(state.status);

        response->mutable_state()->PackFrom(proto_state);
        return grpc::Status::OK;
    }

private:
    angzarr::CommandRouter<table::TableState> router_;
};

} // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    auto router = create_router();
    TableAggregateService service(std::move(router));

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Table aggregate server listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
