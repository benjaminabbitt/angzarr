#include <iostream>
#include <memory>
#include <string>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <google/protobuf/any.pb.h>
#include <google/protobuf/timestamp.pb.h>

#include "hand_state.hpp"
#include "../handlers/deal_handler.hpp"
#include "../handlers/post_blind_handler.hpp"
#include "../handlers/action_handler.hpp"
#include "../handlers/deal_community_handler.hpp"
#include "../handlers/award_pot_handler.hpp"
#include "angzarr/command_router.hpp"
#include "angzarr/aggregate.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50403;
constexpr const char* HAND_DOMAIN = "hand";

/// Create the hand aggregate command router (functional style).
angzarr::CommandRouter<hand::HandState> create_router() {
    return angzarr::CommandRouter<hand::HandState>(HAND_DOMAIN, hand::HandState::from_event_book)
        .on<examples::DealCards, examples::CardsDealt>(hand::handlers::handle_deal)
        .on<examples::PostBlind, examples::BlindPosted>(hand::handlers::handle_post_blind)
        .on<examples::PlayerAction, examples::ActionTaken>(hand::handlers::handle_action)
        .on<examples::DealCommunityCards, examples::CommunityCardsDealt>(
            hand::handlers::handle_deal_community);
}

/// gRPC service implementation for hand aggregate.
class HandAggregateService final : public angzarr::AggregateService::Service {
public:
    explicit HandAggregateService(angzarr::CommandRouter<hand::HandState> router)
        : router_(std::move(router)) {}

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

            hand::HandState state = hand::HandState::from_event_book(event_book);

            auto* events = response->mutable_events();
            auto* cover = events->mutable_cover();
            cover->CopyFrom(event_book.cover());

            // Special handling for AwardPot which returns two events
            if (type_url.find("AwardPot") != std::string::npos) {
                examples::AwardPot cmd;
                command_any.UnpackTo(&cmd);
                auto [pot_event, complete_event] = hand::handlers::handle_award_pot(cmd, state);

                auto* page1 = events->add_pages();
                page1->mutable_event()->PackFrom(pot_event);

                auto* page2 = events->add_pages();
                page2->mutable_event()->PackFrom(complete_event);

                return grpc::Status::OK;
            }

            if (type_url.find("DealCards") != std::string::npos) {
                examples::DealCards cmd;
                command_any.UnpackTo(&cmd);
                auto event = hand::handlers::handle_deal(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("PostBlind") != std::string::npos) {
                examples::PostBlind cmd;
                command_any.UnpackTo(&cmd);
                auto event = hand::handlers::handle_post_blind(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("PlayerAction") != std::string::npos) {
                examples::PlayerAction cmd;
                command_any.UnpackTo(&cmd);
                auto event = hand::handlers::handle_action(cmd, state);
                auto* page = events->add_pages();
                page->mutable_event()->PackFrom(event);
            } else if (type_url.find("DealCommunityCards") != std::string::npos) {
                examples::DealCommunityCards cmd;
                command_any.UnpackTo(&cmd);
                auto event = hand::handlers::handle_deal_community(cmd, state);
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
        for (const auto& event_page : request->events()) {
            *event_book.add_pages() = event_page;
        }

        hand::HandState state = hand::HandState::from_event_book(event_book);

        examples::HandState proto_state;
        proto_state.set_hand_id(state.hand_id);
        proto_state.set_table_root(state.table_root);
        proto_state.set_hand_number(state.hand_number);
        proto_state.set_game_variant(state.game_variant);
        proto_state.set_current_phase(state.current_phase);
        proto_state.set_current_bet(state.current_bet);
        proto_state.set_min_raise(state.min_raise);
        proto_state.set_action_on_position(state.action_on_position);
        proto_state.set_dealer_position(state.dealer_position);
        proto_state.set_small_blind_position(state.small_blind_position);
        proto_state.set_big_blind_position(state.big_blind_position);
        proto_state.set_status(state.status);

        // Add pots
        for (const auto& pot : state.pots) {
            auto* proto_pot = proto_state.add_pots();
            proto_pot->set_amount(pot.amount);
            proto_pot->set_pot_type(pot.pot_type);
            for (const auto& player : pot.eligible_players) {
                proto_pot->add_eligible_players(player);
            }
        }

        response->mutable_state()->PackFrom(proto_state);
        return grpc::Status::OK;
    }

private:
    angzarr::CommandRouter<hand::HandState> router_;
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
    HandAggregateService service(std::move(router));

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Hand aggregate server listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
