/**
 * Output projector (OO pattern) gRPC server.
 *
 * Subscribes to player, table, and hand domain events.
 * Writes formatted game logs to a file.
 *
 * This example demonstrates using the Projector base class with
 * ANGZARR_PROJECTS macro-based handler registration.
 *
 * Compare with the functional pattern in prj-output.
 */
#include <google/protobuf/any.pb.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <grpcpp/grpcpp.h>

#include <chrono>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <memory>
#include <sstream>
#include <string>

#include "angzarr/macros.hpp"
#include "angzarr/projector.grpc.pb.h"
#include "angzarr/projector.hpp"
#include "angzarr/types.pb.h"
#include "examples/hand.pb.h"
#include "examples/player.pb.h"
#include "examples/table.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50492;
const char* DEFAULT_LOG_FILE = "hand_log_oo.txt";

// Global log file
static std::ofstream g_log_file;

void write_log(const std::string& msg) {
    auto now = std::chrono::system_clock::now();
    auto time = std::chrono::system_clock::to_time_t(now);
    std::ostringstream ss;
    ss << std::put_time(std::gmtime(&time), "%Y-%m-%dT%H:%M:%SZ");

    std::string line = "[" + ss.str() + "] " + msg;
    if (g_log_file.is_open()) {
        g_log_file << line << std::endl;
        g_log_file.flush();
    }
    std::cout << line << std::endl;
}

std::string truncate_id(const std::string& root) {
    if (root.size() >= 4) {
        std::ostringstream ss;
        for (int i = 0; i < 4; ++i) {
            ss << std::hex << std::setfill('0') << std::setw(2)
               << (static_cast<unsigned int>(static_cast<unsigned char>(root[i])));
        }
        return ss.str();
    }
    return root;
}

// docs:start:projector_class_oo
/**
 * Projector: Output (OO Pattern)
 *
 * This uses the Projector base class with ANGZARR_PROJECTS macros
 * for handler registration.
 *
 * Note: C++ Projector base class currently supports single domain,
 * so this example focuses on player domain. Multi-domain support
 * would require extending the base class.
 */
class OutputProjector : public angzarr::Projector {
   public:
    ANGZARR_PROJECTOR("output", "player")

    ANGZARR_PROJECTS(PlayerRegistered)
    (const examples::PlayerRegistered& event) {
        write_log("PLAYER registered: " + event.display_name() + " (" + event.email() + ")");
        return angzarr::Projection::upsert("log", "registered");
    }

    ANGZARR_PROJECTS(FundsDeposited)
    (const examples::FundsDeposited& event) {
        int64_t amount = event.has_amount() ? event.amount().amount() : 0;
        int64_t balance = event.has_new_balance() ? event.new_balance().amount() : 0;
        write_log("PLAYER deposited " + std::to_string(amount) +
                  ", balance: " + std::to_string(balance));
        return angzarr::Projection::upsert("log", "deposited");
    }

    ANGZARR_PROJECTS(FundsWithdrawn)
    (const examples::FundsWithdrawn& event) {
        int64_t amount = event.has_amount() ? event.amount().amount() : 0;
        write_log("PLAYER withdrew " + std::to_string(amount));
        return angzarr::Projection::upsert("log", "withdrawn");
    }
};
// docs:end:projector_class_oo

// docs:start:projector_oo_service
/**
 * gRPC service for Output projector using OO pattern.
 */
class OutputProjectorService final : public angzarr::ProjectorService::Service {
   public:
    grpc::Status Handle(grpc::ServerContext* context, const angzarr::EventBook* request,
                        angzarr::Projection* response) override {
        uint32_t seq = 0;

        // Use the OO projector for player domain
        if (request->cover().domain() == "player") {
            auto projections = projector_.project(*request);
            for (const auto& page : request->pages()) {
                seq = page.sequence();
            }
        } else {
            // Handle other domains inline for this example
            for (const auto& page : request->pages()) {
                seq = page.sequence();
                const auto& event_any = page.event();
                handle_other_domain(event_any, request->cover().domain());
            }
        }

        response->mutable_cover()->CopyFrom(request->cover());
        response->set_projector("output");
        response->set_sequence(seq);

        return grpc::Status::OK;
    }

    grpc::Status HandleSpeculative(grpc::ServerContext* context, const angzarr::EventBook* request,
                                   angzarr::Projection* response) override {
        return Handle(context, request, response);
    }

   private:
    void handle_other_domain(const google::protobuf::Any& event_any, const std::string& domain) {
        const std::string& type_url = event_any.type_url();

        // Table events
        if (type_url.find("TableCreated") != std::string::npos) {
            examples::TableCreated event;
            event_any.UnpackTo(&event);
            write_log("TABLE created: " + event.table_name() + " (" +
                      examples::GameVariant_Name(event.game_variant()) + ")");
        } else if (type_url.find("PlayerJoined") != std::string::npos) {
            examples::PlayerJoined event;
            event_any.UnpackTo(&event);
            write_log("TABLE player " + truncate_id(event.player_root()) + " joined with " +
                      std::to_string(event.stack()) + " chips");
        } else if (type_url.find("HandStarted") != std::string::npos) {
            examples::HandStarted event;
            event_any.UnpackTo(&event);
            write_log("TABLE hand #" + std::to_string(event.hand_number()) + " started, " +
                      std::to_string(event.active_players_size()) +
                      " players, dealer at position " + std::to_string(event.dealer_position()));
        }
        // Hand events
        else if (type_url.find("CardsDealt") != std::string::npos) {
            examples::CardsDealt event;
            event_any.UnpackTo(&event);
            write_log("HAND cards dealt to " + std::to_string(event.player_cards_size()) +
                      " players");
        } else if (type_url.find("BlindPosted") != std::string::npos) {
            examples::BlindPosted event;
            event_any.UnpackTo(&event);
            write_log("HAND player " + truncate_id(event.player_root()) + " posted " +
                      examples::BlindType_Name(event.blind_type()) +
                      " blind: " + std::to_string(event.amount()));
        } else if (type_url.find("ActionTaken") != std::string::npos) {
            examples::ActionTaken event;
            event_any.UnpackTo(&event);
            write_log("HAND player " + truncate_id(event.player_root()) + ": " +
                      examples::ActionType_Name(event.action()) + " " +
                      std::to_string(event.amount()));
        } else if (type_url.find("PotAwarded") != std::string::npos) {
            examples::PotAwarded event;
            event_any.UnpackTo(&event);
            std::string winners;
            for (const auto& w : event.winners()) {
                if (!winners.empty()) winners += ", ";
                winners += truncate_id(w.player_root()) + " wins " + std::to_string(w.amount());
            }
            write_log("HAND pot awarded: " + winners);
        } else if (type_url.find("HandComplete") != std::string::npos) {
            examples::HandComplete event;
            event_any.UnpackTo(&event);
            write_log("HAND #" + std::to_string(event.hand_number()) + " complete");
        }
    }

    OutputProjector projector_;
};
// docs:end:projector_oo_service

}  // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    std::string log_file = DEFAULT_LOG_FILE;

    if (const char* env_port = std::getenv("GRPC_PORT")) {
        port = std::stoi(env_port);
    } else if (argc > 1) {
        port = std::stoi(argv[1]);
    }

    if (const char* env_log = std::getenv("HAND_LOG_FILE")) {
        log_file = env_log;
    }

    // Open log file
    g_log_file.open(log_file, std::ios::app);

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    OutputProjectorService service;

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Output projector (OO) listening on " << server_address << std::endl;
    std::cout << "Logging to: " << log_file << std::endl;

    server->Wait();

    g_log_file.close();
    return 0;
}
