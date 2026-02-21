#include <gtest/gtest.h>
#include <string>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/aggregate.pb.h"
#include "angzarr/router.hpp"

using namespace angzarr;

// =============================================================================
// Test State for Router Tests
// =============================================================================

struct TestState {
    int counter = 0;
    std::string last_event_type;
};

// =============================================================================
// EventRouter Tests
// =============================================================================

class EventRouterTest : public ::testing::Test {
protected:
    EventBook make_event_book(const std::string& domain, const std::string& event_type) {
        EventBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain(domain);
        cover->set_correlation_id("test-correlation");

        auto* page = book.add_pages();
        page->set_sequence(1);
        auto* event = page->mutable_event();
        event->set_type_url("type.googleapis.com/" + event_type);

        return book;
    }
};

TEST_F(EventRouterTest, Dispatch_RegisteredHandler_ShouldInvokeHandler) {
    // Given an EventRouter with a registered handler
    bool handler_called = false;
    EventRouter router("saga-test");
    router.domain("orders")
          .on("OrderCreated", [&](const google::protobuf::Any&,
                                  const std::string&,
                                  const std::string&,
                                  const std::vector<EventBook>&) {
              handler_called = true;
              return std::vector<CommandBook>{};
          });

    // When I dispatch an EventBook with that event type
    auto book = make_event_book("orders", "OrderCreated");
    auto commands = router.dispatch(book);

    // Then the handler should be invoked
    EXPECT_TRUE(handler_called);
}

TEST_F(EventRouterTest, Subscriptions_ShouldListRegisteredDomains) {
    // Given an EventRouter with multiple domain handlers
    EventRouter router("pmg-test");
    router.domain("orders")
          .on("OrderCreated", [](const google::protobuf::Any&, const std::string&,
                                 const std::string&, const std::vector<EventBook>&) {
              return std::vector<CommandBook>{};
          })
          .domain("inventory")
          .on("StockReserved", [](const google::protobuf::Any&, const std::string&,
                                  const std::string&, const std::vector<EventBook>&) {
              return std::vector<CommandBook>{};
          });

    // When I get subscriptions
    auto subs = router.subscriptions();

    // Then it should include all registered domains
    EXPECT_EQ(subs.size(), 2);
    EXPECT_TRUE(subs.count("orders") > 0);
    EXPECT_TRUE(subs.count("inventory") > 0);
}

TEST_F(EventRouterTest, Dispatch_UnregisteredEvent_ShouldReturnEmpty) {
    // Given an EventRouter without handler for event type
    EventRouter router("saga-test");
    router.domain("orders")
          .on("OrderCreated", [](const google::protobuf::Any&, const std::string&,
                                 const std::string&, const std::vector<EventBook>&) {
              return std::vector<CommandBook>{};
          });

    // When I dispatch an unregistered event
    auto book = make_event_book("orders", "OrderShipped");
    auto commands = router.dispatch(book);

    // Then no commands should be returned
    EXPECT_TRUE(commands.empty());
}

// =============================================================================
// CommandRouter Tests
// =============================================================================

class CommandRouterTest : public ::testing::Test {
protected:
    ContextualCommand make_contextual_command(const std::string& command_type) {
        ContextualCommand ctx_cmd;
        auto* cmd_book = ctx_cmd.mutable_command();
        auto* cover = cmd_book->mutable_cover();
        cover->set_domain("test");
        cover->set_correlation_id("test-correlation");

        auto* page = cmd_book->add_pages();
        page->set_sequence(1);
        auto* command = page->mutable_command();
        command->set_type_url("type.googleapis.com/" + command_type);

        return ctx_cmd;
    }
};

TEST_F(CommandRouterTest, Dispatch_RegisteredCommand_ShouldReturnEvents) {
    // Given a CommandRouter with registered handler
    auto rebuild = [](const EventBook*) { return TestState{}; };
    CommandRouter<TestState> router("test", rebuild);
    router.on("CreateOrder", [](const CommandBook&, const google::protobuf::Any&,
                                TestState&, int seq) {
        EventBook events;
        auto* page = events.add_pages();
        page->set_sequence(seq);
        auto* event = page->mutable_event();
        event->set_type_url("type.googleapis.com/OrderCreated");
        return events;
    });

    // When I dispatch a command
    auto ctx_cmd = make_contextual_command("CreateOrder");
    auto response = router.dispatch(ctx_cmd);

    // Then events should be returned
    EXPECT_TRUE(response.has_events());
    EXPECT_EQ(response.events().pages_size(), 1);
}

TEST_F(CommandRouterTest, Dispatch_UnknownCommand_ShouldThrow) {
    // Given a CommandRouter with no handler for command
    auto rebuild = [](const EventBook*) { return TestState{}; };
    CommandRouter<TestState> router("test", rebuild);
    router.on("CreateOrder", [](const CommandBook&, const google::protobuf::Any&,
                                TestState&, int) {
        return EventBook{};
    });

    // When I dispatch an unknown command
    auto ctx_cmd = make_contextual_command("UnknownCommand");

    // Then it should throw
    EXPECT_THROW(router.dispatch(ctx_cmd), InvalidArgumentError);
}

TEST_F(CommandRouterTest, Dispatch_RebuildState_ShouldUseRebuildFunction) {
    // Given a CommandRouter with state rebuild function
    bool rebuild_called = false;
    auto rebuild = [&](const EventBook*) {
        rebuild_called = true;
        return TestState{.counter = 5};
    };
    CommandRouter<TestState> router("test", rebuild);
    router.on("UpdateOrder", [](const CommandBook&, const google::protobuf::Any&,
                                TestState& state, int) {
        // Verify state was rebuilt
        EXPECT_EQ(state.counter, 5);
        return EventBook{};
    });

    // When I dispatch a command
    auto ctx_cmd = make_contextual_command("UpdateOrder");
    router.dispatch(ctx_cmd);

    // Then rebuild should have been called
    EXPECT_TRUE(rebuild_called);
}

// =============================================================================
// State Building Tests (using rebuild function pattern)
// =============================================================================

class StateBuildingTest : public ::testing::Test {
protected:
    EventBook make_event_book_with_events(int count) {
        EventBook book;
        for (int i = 0; i < count; i++) {
            auto* page = book.add_pages();
            page->set_sequence(i + 1);
            auto* event = page->mutable_event();
            event->set_type_url("type.googleapis.com/TestEvent");
        }
        return book;
    }

    TestState rebuild_state(const EventBook* event_book) {
        TestState state{};
        if (event_book == nullptr) return state;

        for (const auto& page : event_book->pages()) {
            if (page.has_event()) {
                state.counter++;
                state.last_event_type = helpers::type_name_from_url(page.event().type_url());
            }
        }
        return state;
    }
};

TEST_F(StateBuildingTest, Rebuild_FromEventBook_ShouldApplyEvents) {
    // When I rebuild state from EventBook with 3 events
    auto book = make_event_book_with_events(3);
    auto state = rebuild_state(&book);

    // Then all events should be applied
    EXPECT_EQ(state.counter, 3);
}

TEST_F(StateBuildingTest, Rebuild_FromNullEventBook_ShouldReturnDefaultState) {
    // When I rebuild from null EventBook
    auto state = rebuild_state(nullptr);

    // Then state should be at default values
    EXPECT_EQ(state.counter, 0);
}

TEST_F(StateBuildingTest, Rebuild_ShouldTrackLastEventType) {
    // When I rebuild state with events
    auto book = make_event_book_with_events(1);
    auto state = rebuild_state(&book);

    // Then last event type should be captured
    EXPECT_EQ(state.last_event_type, "TestEvent");
}
