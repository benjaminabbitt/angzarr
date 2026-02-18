#include <iostream>
#include <fstream>
#include <memory>
#include <string>
#include <chrono>
#include <iomanip>
#include <sstream>
#include <grpcpp/grpcpp.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>
#include <google/protobuf/any.pb.h>

#include "angzarr/projector.grpc.pb.h"
#include "angzarr/types.pb.h"
#include "examples/player.pb.h"
#include "examples/table.pb.h"
#include "examples/hand.pb.h"

namespace {

constexpr int DEFAULT_PORT = 50490;
const char* DEFAULT_LOG_FILE = "hand_log.txt";
constexpr const char* PROJECTOR_NAME = "output";

/// gRPC service implementation for output projector.
class OutputProjectorService final : public angzarr::ProjectorService::Service {
public:
    explicit OutputProjectorService(const std::string& log_path, bool show_timestamps = true)
        : log_path_(log_path)
        , log_file_(log_path, std::ios::app)
        , show_timestamps_(show_timestamps) {}

    ~OutputProjectorService() {
        if (log_file_.is_open()) {
            log_file_.close();
        }
    }

    grpc::Status GetDescriptor(
        grpc::ServerContext* context,
        const angzarr::GetDescriptorRequest* request,
        angzarr::ComponentDescriptor* response) override {

        response->set_name(PROJECTOR_NAME);
        response->set_component_type("projector");

        // Subscribe to all poker domains
        auto* player_input = response->add_inputs();
        player_input->set_domain("player");

        auto* table_input = response->add_inputs();
        table_input->set_domain("table");

        auto* hand_input = response->add_inputs();
        hand_input->set_domain("hand");

        return grpc::Status::OK;
    }

    grpc::Status Handle(
        grpc::ServerContext* context,
        const angzarr::EventBook* request,
        angzarr::Projection* response) override {

        return process_event_book(*request, response);
    }

    grpc::Status HandleSpeculative(
        grpc::ServerContext* context,
        const angzarr::EventBook* request,
        angzarr::Projection* response) override {

        // Speculative mode - don't write to file
        uint32_t seq = 0;
        for (const auto& page : request->pages()) {
            seq = page.num();
        }

        response->mutable_cover()->CopyFrom(request->cover());
        response->set_projector(PROJECTOR_NAME);
        response->set_sequence(seq);

        return grpc::Status::OK;
    }

private:
    grpc::Status process_event_book(const angzarr::EventBook& event_book,
                                     angzarr::Projection* response) {
        uint32_t seq = 0;

        for (const auto& page : event_book.pages()) {
            const auto& event_any = page.event();
            seq = page.num();

            // Format and write event
            std::string formatted = format_event(event_any, event_book.cover().domain());
            if (!formatted.empty()) {
                write_line(formatted);
            }
        }

        response->mutable_cover()->CopyFrom(event_book.cover());
        response->set_projector(PROJECTOR_NAME);
        response->set_sequence(seq);

        return grpc::Status::OK;
    }

    std::string format_event(const google::protobuf::Any& event_any, const std::string& domain) {
        std::string prefix;
        if (show_timestamps_) {
            auto now = std::chrono::system_clock::now();
            auto time = std::chrono::system_clock::to_time_t(now);
            std::ostringstream ss;
            ss << std::put_time(std::localtime(&time), "%H:%M:%S");
            prefix = "[" + ss.str() + "] ";
        }

        const std::string& type_url = event_any.type_url();

        // Player events
        if (type_url.find("PlayerRegistered") != std::string::npos) {
            examples::PlayerRegistered event;
            event_any.UnpackTo(&event);
            return prefix + "Player registered: " + event.display_name();
        }

        if (type_url.find("FundsDeposited") != std::string::npos) {
            examples::FundsDeposited event;
            event_any.UnpackTo(&event);
            return prefix + "Funds deposited: " + std::to_string(event.new_balance().amount());
        }

        // Table events
        if (type_url.find("TableCreated") != std::string::npos) {
            examples::TableCreated event;
            event_any.UnpackTo(&event);
            return prefix + "Table created: " + event.table_name();
        }

        if (type_url.find("PlayerJoined") != std::string::npos) {
            examples::PlayerJoined event;
            event_any.UnpackTo(&event);
            return prefix + "Player joined at position " + std::to_string(event.seat_position());
        }

        if (type_url.find("HandStarted") != std::string::npos) {
            examples::HandStarted event;
            event_any.UnpackTo(&event);
            return prefix + "Hand started: dealer position " + std::to_string(event.dealer_position());
        }

        // Hand events
        if (type_url.find("CardsDealt") != std::string::npos) {
            examples::CardsDealt event;
            event_any.UnpackTo(&event);
            return prefix + "Cards dealt to " + std::to_string(event.players_size()) + " players";
        }

        if (type_url.find("BlindPosted") != std::string::npos) {
            examples::BlindPosted event;
            event_any.UnpackTo(&event);
            return prefix + "Blind posted: " + std::to_string(event.amount());
        }

        if (type_url.find("ActionTaken") != std::string::npos) {
            examples::ActionTaken event;
            event_any.UnpackTo(&event);
            return prefix + "Action: " + examples::ActionType_Name(event.action());
        }

        if (type_url.find("CommunityCardsDealt") != std::string::npos) {
            examples::CommunityCardsDealt event;
            event_any.UnpackTo(&event);
            return prefix + "Community cards dealt: " + std::to_string(event.cards_size()) + " cards";
        }

        if (type_url.find("PotAwarded") != std::string::npos) {
            examples::PotAwarded event;
            event_any.UnpackTo(&event);
            int64_t total = 0;
            for (const auto& winner : event.winners()) {
                total += winner.amount();
            }
            return prefix + "Pot awarded: " + std::to_string(total);
        }

        if (type_url.find("HandComplete") != std::string::npos) {
            examples::HandComplete event;
            event_any.UnpackTo(&event);
            return prefix + "Hand complete";
        }

        // Return empty for unknown event types
        return "";
    }

    void write_line(const std::string& text) {
        if (log_file_.is_open()) {
            log_file_ << text << std::endl;
            log_file_.flush();
        }
        std::cout << text << std::endl;
    }

    std::string log_path_;
    std::ofstream log_file_;
    bool show_timestamps_;
};

} // anonymous namespace

int main(int argc, char** argv) {
    int port = DEFAULT_PORT;
    std::string log_file = DEFAULT_LOG_FILE;

    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg.find("--port=") == 0) {
            port = std::stoi(arg.substr(7));
        } else if (arg.find("--log=") == 0) {
            log_file = arg.substr(6);
        } else {
            port = std::stoi(arg);
        }
    }

    // Check for HAND_LOG_FILE environment variable
    const char* env_log = std::getenv("HAND_LOG_FILE");
    if (env_log) {
        log_file = env_log;
    }

    std::string server_address = "0.0.0.0:" + std::to_string(port);

    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    OutputProjectorService service(log_file);

    grpc::ServerBuilder builder;
    builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Output projector listening on " << server_address << std::endl;
    std::cout << "Logging to: " << log_file << std::endl;

    server->Wait();
    return 0;
}
