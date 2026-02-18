#include <iostream>
#include <memory>
#include <string>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <google/protobuf/any.pb.h>
#include <google/protobuf/timestamp.pb.h>

#include "player_state.hpp"
#include "angzarr/command_router.hpp"
#include "angzarr/aggregate.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/player.pb.h"

#include "../handlers/register_handler.hpp"
#include "../handlers/deposit_handler.hpp"
#include "../handlers/withdraw_handler.hpp"
#include "../handlers/reserve_handler.hpp"
#include "../handlers/release_handler.hpp"
#include "../handlers/transfer_handler.hpp"

namespace {

constexpr int DEFAULT_PORT = 50401;
constexpr const char* PLAYER_DOMAIN = "player";

/// Helper to pack a protobuf message into Any
template<typename T>
google::protobuf::Any pack_any(const T& msg) {
    google::protobuf::Any any;
    any.PackFrom(msg);
    return any;
}

/// Create functional command router for player aggregate.
angzarr::CommandRouter<player::PlayerState> create_router() {
    return angzarr::CommandRouter<player::PlayerState>(PLAYER_DOMAIN, player::PlayerState::from_event_book)
        .on<examples::RegisterPlayer, examples::PlayerRegistered>(player::handlers::handle_register)
        .on<examples::DepositFunds, examples::FundsDeposited>(player::handlers::handle_deposit)
        .on<examples::WithdrawFunds, examples::FundsWithdrawn>(player::handlers::handle_withdraw)
        .on<examples::ReserveFunds, examples::FundsReserved>(player::handlers::handle_reserve)
        .on<examples::ReleaseFunds, examples::FundsReleased>(player::handlers::handle_release)
        .on<examples::TransferFunds, examples::FundsTransferred>(player::handlers::handle_transfer);
}

/// gRPC service implementation for player aggregate.
class PlayerAggregateService final : public angzarr::AggregateService::Service {
public:
    explicit PlayerAggregateService(angzarr::CommandRouter<player::PlayerState> router)
        : router_(std::move(router)) {}

    grpc::Status GetDescriptor(
        grpc::ServerContext* context,
        const angzarr::GetDescriptorRequest* request,
        angzarr::ComponentDescriptor* response) override {

        response->set_name(PLAYER_DOMAIN);
        response->set_component_type("aggregate");

        auto* input = response->add_inputs();
        input->set_domain(PLAYER_DOMAIN);
        input->add_types("RegisterPlayer");
        input->add_types("DepositFunds");
        input->add_types("WithdrawFunds");
        input->add_types("ReserveFunds");
        input->add_types("ReleaseFunds");
        input->add_types("TransferFunds");

        return grpc::Status::OK;
    }

    grpc::Status Handle(
        grpc::ServerContext* context,
        const angzarr::ContextualCommand* request,
        angzarr::BusinessResponse* response) override {

        try {
            // Get command and events from request
            const auto& command_book = request->command();
            const auto& event_book = request->events();

            if (command_book.pages_size() == 0) {
                return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "No command pages");
            }

            // Get the command Any from first page
            const auto& command_page = command_book.pages(0);
            const auto& command_any = command_page.command();
            const std::string& type_url = command_any.type_url();

            // Build state from events
            player::PlayerState state = player::PlayerState::from_event_book(event_book);

            // Create response event book
            auto* events = response->mutable_events();
            auto* cover = events->mutable_cover();
            cover->CopyFrom(event_book.cover());

            // Dispatch based on command type
            std::unique_ptr<google::protobuf::Message> event_msg;

            if (type_url.find("RegisterPlayer") != std::string::npos) {
                examples::RegisterPlayer cmd;
                command_any.UnpackTo(&cmd);
                auto event = player::handlers::handle_register(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("DepositFunds") != std::string::npos) {
                examples::DepositFunds cmd;
                command_any.UnpackTo(&cmd);
                auto event = player::handlers::handle_deposit(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("WithdrawFunds") != std::string::npos) {
                examples::WithdrawFunds cmd;
                command_any.UnpackTo(&cmd);
                auto event = player::handlers::handle_withdraw(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("ReserveFunds") != std::string::npos) {
                examples::ReserveFunds cmd;
                command_any.UnpackTo(&cmd);
                auto event = player::handlers::handle_reserve(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("ReleaseFunds") != std::string::npos) {
                examples::ReleaseFunds cmd;
                command_any.UnpackTo(&cmd);
                auto event = player::handlers::handle_release(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("TransferFunds") != std::string::npos) {
                examples::TransferFunds cmd;
                command_any.UnpackTo(&cmd);
                auto event = player::handlers::handle_transfer(cmd, state);
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

        // Build state from events in request
        angzarr::EventBook event_book;
        *event_book.mutable_pages() = {request->events().begin(), request->events().end()};

        player::PlayerState state = player::PlayerState::from_event_book(event_book);

        // Pack state into response
        examples::PlayerState proto_state;
        proto_state.set_player_id(state.player_id);
        proto_state.set_display_name(state.display_name);
        proto_state.set_email(state.email);
        proto_state.set_player_type(static_cast<examples::PlayerType>(state.player_type));
        proto_state.set_ai_model_id(state.ai_model_id);
        proto_state.mutable_bankroll()->set_amount(state.bankroll);
        proto_state.mutable_bankroll()->set_currency_code("CHIPS");
        proto_state.mutable_reserved_funds()->set_amount(state.reserved_funds);
        proto_state.mutable_reserved_funds()->set_currency_code("CHIPS");
        proto_state.set_status(state.status);

        for (const auto& [table_hex, amount] : state.table_reservations) {
            (*proto_state.mutable_table_reservations())[table_hex] = amount;
        }

        response->mutable_state()->PackFrom(proto_state);
        return grpc::Status::OK;
    }

private:
    angzarr::CommandRouter<player::PlayerState> router_;
};

} // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    // Enable reflection for debugging
    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    // Create router and service
    auto router = create_router();
    PlayerAggregateService service(std::move(router));

    // Build and start server
    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Player aggregate server listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
