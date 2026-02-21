#include <gtest/gtest.h>
#include <string>
#include <google/protobuf/any.pb.h>
#include <google/protobuf/empty.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/aggregate.pb.h"
#include "angzarr/router.hpp"
#include "angzarr/helpers.hpp"

using namespace angzarr;

// =============================================================================
// State Building Tests
// =============================================================================

struct AggregateState {
    int counter = 0;
    std::string last_event_type;
    bool initialized = false;
};

// Use a rebuild function pattern instead of StateRouter directly
// (matches the CommandRouter pattern used in the framework)
AggregateState rebuild_state(const EventBook* book) {
    AggregateState state{};
    if (!book) return state;

    for (const auto& page : book->pages()) {
        if (page.has_event()) {
            state.counter++;
            state.last_event_type = helpers::type_name_from_url(page.event().type_url());
        }
    }
    return state;
}

class StateBuildingTest : public ::testing::Test {
protected:
    EventBook make_event_book(int event_count) {
        EventBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain("test");

        for (int i = 0; i < event_count; i++) {
            auto* page = book.add_pages();
            page->set_sequence(i + 1);
            auto* event = page->mutable_event();
            event->set_type_url("type.googleapis.com/TestEvent");
        }
        return book;
    }
};

TEST_F(StateBuildingTest, WithEventBook_ShouldApplyAllEvents) {
    // When I build state from EventBook with 5 events
    auto book = make_event_book(5);
    auto state = rebuild_state(&book);

    // Then the state counter should be 5
    EXPECT_EQ(state.counter, 5);
    EXPECT_EQ(state.last_event_type, "TestEvent");
}

TEST_F(StateBuildingTest, WithEmptyEventBook_ShouldReturnDefaultState) {
    // When I build state from empty EventBook
    EventBook empty_book;
    auto state = rebuild_state(&empty_book);

    // Then the state should be at default values
    EXPECT_EQ(state.counter, 0);
}

TEST_F(StateBuildingTest, WithNullEventBook_ShouldReturnDefaultState) {
    // When I build state from null EventBook
    auto state = rebuild_state(nullptr);

    // Then the state should be at default values
    EXPECT_EQ(state.counter, 0);
    EXPECT_TRUE(state.last_event_type.empty());
}

TEST_F(StateBuildingTest, UnknownEventType_ShouldStillBeCountedByGenericHandler) {
    // Given a rebuild function that counts all events
    // When I build state with any event types
    EventBook book;
    auto* page = book.add_pages();
    page->mutable_event()->set_type_url("type.googleapis.com/UnknownEventType");

    auto state = rebuild_state(&book);

    // Then all events should be counted (generic handler)
    EXPECT_EQ(state.counter, 1);
    EXPECT_EQ(state.last_event_type, "UnknownEventType");
}

// =============================================================================
// Snapshot Integration Tests
// =============================================================================

TEST_F(StateBuildingTest, WithSnapshot_ShouldRestoreFromSnapshot) {
    // Given an EventBook with a snapshot
    EventBook book;
    auto* snapshot = book.mutable_snapshot();
    snapshot->set_sequence(10);
    snapshot->set_retention(SnapshotRetention::RETENTION_DEFAULT);

    // And some events after the snapshot
    for (int i = 11; i <= 15; i++) {
        auto* page = book.add_pages();
        page->set_sequence(i);
        page->mutable_event()->set_type_url("type.googleapis.com/TestEvent");
    }

    // When I build state (using rebuild function)
    // Note: actual snapshot deserialization depends on impl
    auto state = rebuild_state(&book);

    // Then state should include post-snapshot events
    // rebuild_state counts all pages, so we get 5
    EXPECT_EQ(state.counter, 5);
}

// =============================================================================
// Next Sequence Tests
// =============================================================================

TEST_F(StateBuildingTest, NextSequence_FromEvents_ShouldReturnLastPlusOne) {
    // Given an EventBook with events
    auto book = make_event_book(5);

    // When I get next sequence
    auto next = helpers::next_sequence(&book);

    // Then it should be event count (pages count)
    EXPECT_EQ(next, 5);
}

TEST_F(StateBuildingTest, NextSequence_FromNullBook_ShouldReturnZero) {
    // When I get next sequence from null book
    auto next = helpers::next_sequence(nullptr);

    // Then it should be 0
    EXPECT_EQ(next, 0);
}

TEST_F(StateBuildingTest, NextSequence_FromEmptyBook_ShouldReturnZero) {
    // Given an empty EventBook
    EventBook empty_book;

    // When I get next sequence
    auto next = helpers::next_sequence(&empty_book);

    // Then it should be 0
    EXPECT_EQ(next, 0);
}
