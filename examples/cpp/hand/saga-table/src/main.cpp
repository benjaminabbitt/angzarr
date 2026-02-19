#include <iostream>
#include <memory>
#include <string>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <google/protobuf/any.pb.h>

#include "angzarr/saga.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/table.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50412;
constexpr const char* SAGA_NAME = "saga-hand-table";
constexpr const char* INPUT_DOMAIN = "hand";
constexpr const char* OUTPUT_DOMAIN = "table";

/// gRPC service implementation for hand-table saga.
class HandTableSagaService final : public angzarr::SagaService::Service {
public:
    grpc::Status Prepare(
        grpc::ServerContext* context,
        const angzarr::SagaPrepareRequest* request,
        angzarr::SagaPrepareResponse* response) override {

        // Find HandComplete event and extract table root
        const auto& source = request->source();
        for (const auto& page : source.pages()) {
            const auto& event_any = page.event();
            if (event_any.type_url().find("HandComplete") != std::string::npos) {
                examples::HandComplete event;
                event_any.UnpackTo(&event);

                // Add table as destination
                auto* dest = response->add_destinations();
                dest->set_domain(OUTPUT_DOMAIN);
                dest->mutable_root()->set_value(event.table_root());

                break;
            }
        }

        return grpc::Status::OK;
    }

    grpc::Status Execute(
        grpc::ServerContext* context,
        const angzarr::SagaExecuteRequest* request,
        angzarr::SagaResponse* response) override {

        try {
            const auto& source = request->source();

            // Find HandComplete event
            for (const auto& page : source.pages()) {
                const auto& event_any = page.event();
                if (event_any.type_url().find("HandComplete") != std::string::npos) {
                    examples::HandComplete event;
                    event_any.UnpackTo(&event);

                    // Get hand_root from source
                    std::string hand_root;
                    if (source.has_cover() && source.cover().has_root()) {
                        hand_root = source.cover().root().value();
                    }

                    // Get sequence from destination state
                    uint64_t dest_seq = 0;
                    if (request->destinations_size() > 0) {
                        const auto& dest = request->destinations(0);
                        if (dest.pages_size() > 0) {
                            dest_seq = dest.pages(dest.pages_size() - 1).sequence() + 1;
                        }
                    }

                    // Build EndHand command
                    examples::EndHand end_hand;
                    end_hand.set_hand_root(hand_root);

                    // Convert PotWinner to PotResult
                    for (const auto& winner : event.winners()) {
                        auto* result = end_hand.add_results();
                        result->set_winner_root(winner.player_root());
                        result->set_amount(winner.amount());
                        result->set_pot_type(winner.pot_type());
                        *result->mutable_winning_hand() = winner.winning_hand();
                    }

                    // Build command book
                    auto* cmd_book = response->add_commands();
                    auto* cover = cmd_book->mutable_cover();
                    cover->set_domain(OUTPUT_DOMAIN);
                    cover->mutable_root()->set_value(event.table_root());
                    cover->set_correlation_id(source.cover().correlation_id());

                    auto* cmd_page = cmd_book->add_pages();
                    cmd_page->set_sequence(dest_seq);
                    cmd_page->mutable_command()->PackFrom(end_hand);

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

    HandTableSagaService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Hand-Table saga server listening on " << server_address << std::endl;

    server->Wait();
    return 0;
}
