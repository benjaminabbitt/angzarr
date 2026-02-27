// DOC: This file is referenced in docs/docs/examples/sagas.mdx
//      Update documentation when making changes to saga patterns.
/**
 * Spring Boot application for Table -> Hand saga using OO pattern.
 *
 * This example demonstrates using the Saga base class with
 * macro-based handler registration (ANGZARR_PREPARES, ANGZARR_REACTS_TO).
 *
 * Compare with the functional EventRouter pattern in table/saga-hand.
 */
#include <google/protobuf/any.pb.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <grpcpp/grpcpp.h>

#include <iostream>
#include <memory>
#include <string>

#include "angzarr/macros.hpp"
#include "angzarr/saga.grpc.pb.h"
#include "angzarr/saga.hpp"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/table.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50412;

// docs:start:saga_oo
/**
 * Saga: Table -> Hand (OO Pattern)
 *
 * Reacts to HandStarted events from Table domain.
 * Sends DealCards commands to Hand domain.
 *
 * Uses macro-based handler registration with:
 * - ANGZARR_PREPARES(EventType) for prepare phase handlers
 * - ANGZARR_REACTS_TO(EventType) for execute phase handlers
 */
class TableHandSaga : public angzarr::Saga {
   public:
    ANGZARR_SAGA("saga-table-hand", "table", "hand")

    /**
     * Prepare phase: declare which destination aggregates we need to read.
     *
     * Called during the prepare phase of the two-phase saga protocol.
     * Returns a list of Cover objects identifying the destination aggregates
     * needed for the execute phase.
     */
    ANGZARR_PREPARES(HandStarted)
    (const examples::HandStarted& event) {
        angzarr::Cover cover;
        cover.set_domain("hand");
        cover.mutable_root()->set_value(event.hand_root());
        return {cover};
    }

    /**
     * Execute phase: translate Table.HandStarted -> Hand.DealCards.
     *
     * Called during the execute phase with the source event and
     * fetched destination EventBooks. Returns the command to send.
     */
    ANGZARR_REACTS_TO(HandStarted)
    (const examples::HandStarted& event) {
        examples::DealCards deal_cmd;
        deal_cmd.set_table_root(event.hand_root());
        deal_cmd.set_hand_number(event.hand_number());
        deal_cmd.set_game_variant(event.game_variant());
        deal_cmd.set_dealer_position(event.dealer_position());
        deal_cmd.set_small_blind(event.small_blind());
        deal_cmd.set_big_blind(event.big_blind());

        // Convert SeatSnapshot to PlayerInHand
        for (const auto& seat : event.active_players()) {
            auto* player = deal_cmd.add_players();
            player->set_player_root(seat.player_root());
            player->set_position(seat.position());
            player->set_stack(seat.stack());
        }

        return deal_cmd;
    }
};
// docs:end:saga_oo

// docs:start:saga_oo_service
/**
 * gRPC service for Table->Hand saga using OO pattern.
 */
class TableHandSagaService final : public angzarr::SagaService::Service {
   public:
    grpc::Status Prepare(grpc::ServerContext* context, const angzarr::SagaPrepareRequest* request,
                         angzarr::SagaPrepareResponse* response) override {
        auto destinations = saga_.prepare_destinations(request->source());
        for (const auto& dest : destinations) {
            *response->add_destinations() = dest;
        }
        return grpc::Status::OK;
    }

    grpc::Status Execute(grpc::ServerContext* context, const angzarr::SagaExecuteRequest* request,
                         angzarr::SagaResponse* response) override {
        std::vector<angzarr::EventBook> destinations(request->destinations().begin(),
                                                     request->destinations().end());

        auto result = saga_.dispatch(request->source(), destinations);

        for (const auto& cmd : result.commands) {
            *response->add_commands() = cmd;
        }
        for (const auto& fact : result.facts) {
            *response->add_events() = fact;
        }

        return grpc::Status::OK;
    }

   private:
    TableHandSaga saga_;
};
// docs:end:saga_oo_service

}  // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    if (const char* env_port = std::getenv("GRPC_PORT")) {
        port = std::stoi(env_port);
    } else if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    TableHandSagaService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Table->Hand saga (OO) listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
