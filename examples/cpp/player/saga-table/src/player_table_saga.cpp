#include "player_table_saga.hpp"

#include <google/protobuf/any.pb.h>

namespace player {
namespace saga {

// Thread-local storage for source root and facts during dispatch
static thread_local std::string current_source_root;
static thread_local std::vector<angzarr::EventBook> emitted_facts;

void set_source_root(const angzarr::EventBook* source) {
    if (source && source->has_cover() && source->cover().has_root()) {
        current_source_root = source->cover().root().value();
    } else {
        current_source_root.clear();
    }
}

std::vector<angzarr::EventBook> get_emitted_facts() { return emitted_facts; }

void clear_emitted_facts() { emitted_facts.clear(); }

static void emit_fact(angzarr::EventBook fact) { emitted_facts.push_back(std::move(fact)); }

// Handle PlayerSittingOut -> PlayerSatOut fact for table.
// Sagas are stateless translators - framework handles sequence stamping.
static std::vector<angzarr::CommandBook> handle_sitting_out(
    const google::protobuf::Any& event_any, const std::string& correlation_id,
    const std::string& source_domain, const std::vector<angzarr::EventBook>& destinations) {
    (void)correlation_id;
    (void)source_domain;
    (void)destinations;  // Sagas are stateless - destinations not used

    examples::PlayerSittingOut event;
    event_any.UnpackTo(&event);

    // Create PlayerSatOut fact for the table
    examples::PlayerSatOut sat_out;
    sat_out.set_player_root(current_source_root);
    *sat_out.mutable_sat_out_at() = event.sat_out_at();

    google::protobuf::Any fact_any;
    fact_any.PackFrom(sat_out, "type.googleapis.com/");

    // Build EventBook as fact for table domain
    angzarr::EventBook fact;
    auto* cover = fact.mutable_cover();
    cover->set_domain("table");
    cover->mutable_root()->set_value(event.table_root());

    auto* page = fact.add_pages();
    // Framework handles sequence stamping
    page->mutable_header()->mutable_angzarr_deferred();
    *page->mutable_event() = fact_any;

    emit_fact(std::move(fact));

    // Return empty commands - we emit facts instead
    return {};
}

// Handle PlayerReturningToPlay -> PlayerSatIn fact for table.
// Sagas are stateless translators - framework handles sequence stamping.
static std::vector<angzarr::CommandBook> handle_returning_to_play(
    const google::protobuf::Any& event_any, const std::string& correlation_id,
    const std::string& source_domain, const std::vector<angzarr::EventBook>& destinations) {
    (void)correlation_id;
    (void)source_domain;
    (void)destinations;  // Sagas are stateless - destinations not used

    examples::PlayerReturningToPlay event;
    event_any.UnpackTo(&event);

    // Create PlayerSatIn fact for the table
    examples::PlayerSatIn sat_in;
    sat_in.set_player_root(current_source_root);
    *sat_in.mutable_sat_in_at() = event.sat_in_at();

    google::protobuf::Any fact_any;
    fact_any.PackFrom(sat_in, "type.googleapis.com/");

    // Build EventBook as fact for table domain
    angzarr::EventBook fact;
    auto* cover = fact.mutable_cover();
    cover->set_domain("table");
    cover->mutable_root()->set_value(event.table_root());

    auto* page = fact.add_pages();
    // Framework handles sequence stamping
    page->mutable_header()->mutable_angzarr_deferred();
    *page->mutable_event() = fact_any;

    emit_fact(std::move(fact));

    // Return empty commands - we emit facts instead
    return {};
}

angzarr::EventRouter create_player_table_router() {
    return angzarr::EventRouter("saga-player-table")
        .domain("player")
        .on("PlayerSittingOut", handle_sitting_out)
        .on("PlayerReturningToPlay", handle_returning_to_play);
}

}  // namespace saga
}  // namespace player
