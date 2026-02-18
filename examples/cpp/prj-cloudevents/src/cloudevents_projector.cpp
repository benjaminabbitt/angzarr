// CloudEvents projector - publishes player events as CloudEvents.
//
// This projector transforms internal domain events into CloudEvents 1.0 format
// for external consumption via HTTP webhooks or Kafka.

#include <optional>
#include <string>

#include "angzarr/client.hpp"
#include "angzarr/proto/angzarr/cloudevents.pb.h"
#include "angzarr/proto/examples/player.pb.h"

using namespace angzarr;
using namespace angzarr::proto::angzarr;
using namespace angzarr::proto::examples;

// docs:start:cloudevents_oo
class PlayerCloudEventsProjector : public CloudEventsProjector {
public:
    PlayerCloudEventsProjector()
        : CloudEventsProjector("prj-player-cloudevents", "player") {}

    std::optional<CloudEvent> on_player_registered(const PlayerRegistered& event) {
        // Filter sensitive fields, return public version
        PublicPlayerRegistered public_event;
        public_event.set_display_name(event.display_name());
        public_event.set_player_type(event.player_type());

        CloudEvent ce;
        ce.set_type("com.poker.player.registered");
        ce.mutable_data()->PackFrom(public_event);
        return ce;
    }

    std::optional<CloudEvent> on_funds_deposited(const FundsDeposited& event) {
        PublicFundsDeposited public_event;
        *public_event.mutable_amount() = event.amount();

        CloudEvent ce;
        ce.set_type("com.poker.player.deposited");
        ce.mutable_data()->PackFrom(public_event);
        (*ce.mutable_extensions())["priority"] = "normal";
        return ce;
    }
};
// docs:end:cloudevents_oo

// docs:start:cloudevents_router
std::optional<CloudEvent> handle_player_registered(const PlayerRegistered& event) {
    PublicPlayerRegistered public_event;
    public_event.set_display_name(event.display_name());
    public_event.set_player_type(event.player_type());

    CloudEvent ce;
    ce.set_type("com.poker.player.registered");
    ce.mutable_data()->PackFrom(public_event);
    return ce;
}

std::optional<CloudEvent> handle_funds_deposited(const FundsDeposited& event) {
    PublicFundsDeposited public_event;
    *public_event.mutable_amount() = event.amount();

    CloudEvent ce;
    ce.set_type("com.poker.player.deposited");
    ce.mutable_data()->PackFrom(public_event);
    (*ce.mutable_extensions())["priority"] = "normal";
    return ce;
}

CloudEventsRouter build_router() {
    return CloudEventsRouter("prj-player-cloudevents", "player")
        .on<PlayerRegistered>(handle_player_registered)
        .on<FundsDeposited>(handle_funds_deposited);
}
// docs:end:cloudevents_router

int main() {
    auto router = build_router();
    run_cloudevents_projector("prj-player-cloudevents", 50092, router);
    return 0;
}
