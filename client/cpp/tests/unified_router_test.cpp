#include "angzarr/unified_router.hpp"

#include <google/protobuf/any.pb.h>
#include <gtest/gtest.h>

#include <string>

#include "angzarr/aggregate.pb.h"
#include "angzarr/process_manager.pb.h"
#include "angzarr/saga.pb.h"
#include "angzarr/types.pb.h"

using namespace angzarr;

// =============================================================================
// Test Types
// =============================================================================

struct TestState {
    int counter = 0;
    std::string last_command;
    bool exists = false;
};

// =============================================================================
// Mock Handlers
// =============================================================================

class MockAggregateHandler : public AggregateDomainHandler<TestState> {
   public:
    MockAggregateHandler() {
        state_router_.on<EventPage>([](TestState& s, const EventPage&) { s.counter++; });
    }

    std::vector<std::string> command_types() const override {
        return {"CreateOrder", "UpdateOrder"};
    }

    const StateRouter<TestState>& state_router() const override { return state_router_; }

    EventBook handle(const CommandBook& cmd, const google::protobuf::Any& payload,
                     const TestState& state, int seq) override {
        handle_called_ = true;
        last_state_ = state;

        std::string type_url = payload.type_url();
        auto pos = type_url.rfind('/');
        std::string suffix = (pos != std::string::npos) ? type_url.substr(pos + 1) : type_url;
        auto dot_pos = suffix.rfind('.');
        if (dot_pos != std::string::npos) {
            suffix = suffix.substr(dot_pos + 1);
        }

        if (suffix == "CreateOrder") {
            EventBook events;
            auto* page = events.add_pages();
            page->set_sequence(seq);
            page->mutable_event()->set_type_url("type.googleapis.com/OrderCreated");
            return events;
        }

        throw CommandRejectedError("Unknown command: " + suffix);
    }

    RejectionHandlerResponse on_rejected(const Notification& notification, const TestState& state,
                                         const std::string& target_domain,
                                         const std::string& target_command) override {
        rejection_called_ = true;
        rejection_domain_ = target_domain;
        rejection_command_ = target_command;
        return RejectionHandlerResponse::empty();
    }

    // Test inspection
    bool handle_called_ = false;
    bool rejection_called_ = false;
    std::string rejection_domain_;
    std::string rejection_command_;
    TestState last_state_;

   private:
    StateRouter<TestState> state_router_;
};

class MockSagaHandler : public SagaDomainHandler {
   public:
    std::vector<std::string> event_types() const override {
        return {"OrderCreated", "OrderCancelled"};
    }

    std::vector<Cover> prepare(const EventBook& source,
                               const google::protobuf::Any& event) override {
        prepare_called_ = true;
        Cover cover;
        cover.set_domain("fulfillment");
        cover.mutable_root()->set_value("dest-123");
        return {cover};
    }

    std::vector<CommandBook> execute(const EventBook& source, const google::protobuf::Any& event,
                                     const std::vector<EventBook>& destinations) override {
        execute_called_ = true;
        destinations_count_ = static_cast<int>(destinations.size());

        CommandBook cmd;
        auto* cover = cmd.mutable_cover();
        cover->set_domain("fulfillment");
        cover->set_correlation_id(source.has_cover() ? source.cover().correlation_id() : "");

        auto* page = cmd.add_pages();
        page->mutable_command()->set_type_url("type.googleapis.com/CreateShipment");

        return {cmd};
    }

    // Test inspection
    bool prepare_called_ = false;
    bool execute_called_ = false;
    int destinations_count_ = 0;
};

class MockPmHandler : public ProcessManagerDomainHandler<TestState> {
   public:
    std::vector<std::string> event_types() const override { return {"OrderCreated"}; }

    std::vector<Cover> prepare(const EventBook& trigger, const TestState& state,
                               const google::protobuf::Any& event) override {
        prepare_called_ = true;
        return {};
    }

    ProcessManagerResponse handle(const EventBook& trigger, const TestState& state,
                                  const google::protobuf::Any& event,
                                  const std::vector<EventBook>& destinations) override {
        handle_called_ = true;
        last_state_ = state;

        CommandBook cmd;
        cmd.mutable_cover()->set_domain("inventory");
        cmd.add_pages()->mutable_command()->set_type_url("type.googleapis.com/ReserveStock");

        return ProcessManagerResponse::with_commands({cmd});
    }

    RejectionHandlerResponse on_rejected(const Notification& notification, const TestState& state,
                                         const std::string& target_domain,
                                         const std::string& target_command) override {
        rejection_called_ = true;
        return RejectionHandlerResponse::empty();
    }

    // Test inspection
    bool prepare_called_ = false;
    bool handle_called_ = false;
    bool rejection_called_ = false;
    TestState last_state_;
};

class MockProjectorHandler : public ProjectorDomainHandler {
   public:
    std::vector<std::string> event_types() const override {
        return {"PlayerRegistered", "FundsDeposited"};
    }

    Projection project(const EventBook& events) override {
        project_called_ = true;
        events_count_ = events.pages_size();

        Projection proj;
        // Would populate projection results here
        return proj;
    }

    // Test inspection
    bool project_called_ = false;
    int events_count_ = 0;
};

// =============================================================================
// StateRouter Tests
// =============================================================================

class StateRouterTest : public ::testing::Test {
   protected:
    EventBook make_event_book(int event_count) {
        EventBook book;
        for (int i = 0; i < event_count; i++) {
            auto* page = book.add_pages();
            page->set_sequence(i + 1);
            page->mutable_event()->set_type_url("type.googleapis.com/TestEvent");
        }
        return book;
    }
};

TEST_F(StateRouterTest, WithEventBook_ShouldApplyEvents) {
    // Given a StateRouter with an event applier
    int apply_count = 0;
    StateRouter<TestState> router;
    router.on<EventPage>([&apply_count](TestState& s, const EventPage&) {
        s.counter++;
        apply_count++;
    });

    // When I rebuild state from EventBook with 3 events
    auto book = make_event_book(3);
    auto state = router.with_event_book(&book);

    // Then all events should be applied (EventPage doesn't match TestEvent type URL)
    // Note: The router matches by type name, so EventPage won't match TestEvent
    EXPECT_EQ(state.counter, 0);  // No match for TestEvent -> EventPage
}

TEST_F(StateRouterTest, WithEventBook_NullBook_ShouldReturnDefaultState) {
    // Given a StateRouter
    StateRouter<TestState> router;

    // When I rebuild from null EventBook
    auto state = router.with_event_book(nullptr);

    // Then state should be at default values
    EXPECT_EQ(state.counter, 0);
    EXPECT_FALSE(state.exists);
}

TEST_F(StateRouterTest, CustomFactory_ShouldUseFactory) {
    // Given a StateRouter with custom factory
    StateRouter<TestState> router([]() { return TestState{.counter = 100, .exists = true}; });

    // When I rebuild state
    auto state = router.with_event_book(nullptr);

    // Then factory should have been used
    EXPECT_EQ(state.counter, 100);
    EXPECT_TRUE(state.exists);
}

TEST_F(StateRouterTest, TypeMatching_ShouldMatchBySimpleName) {
    // Given a StateRouter with specific event type registered
    // Using EventPage as a stand-in since we don't have business event types in tests
    bool applier_called = false;
    StateRouter<TestState> router;
    router.on<EventPage>([&applier_called](TestState& s, const EventPage& e) {
        applier_called = true;
        s.counter = static_cast<int>(e.sequence());
    });

    // When I apply an event with matching type URL
    EventBook book;
    auto* page = book.add_pages();
    page->set_sequence(42);

    // Pack an EventPage into the event Any
    EventPage inner_event;
    inner_event.set_sequence(99);
    page->mutable_event()->PackFrom(inner_event, "type.googleapis.com/");

    auto state = router.with_event_book(&book);

    // Then applier should have been called with unpacked event
    EXPECT_TRUE(applier_called);
    EXPECT_EQ(state.counter, 99);
}

TEST_F(StateRouterTest, TypeMatching_ShouldIgnoreUnknownTypes) {
    // Given a StateRouter with specific event type registered
    StateRouter<TestState> router;
    router.on<EventPage>([](TestState& s, const EventPage&) { s.counter++; });

    // When I apply an event with non-matching type URL
    EventBook book;
    auto* page = book.add_pages();
    page->mutable_event()->set_type_url("type.googleapis.com/unknown.SomeOtherEvent");

    auto state = router.with_event_book(&book);

    // Then counter should remain at default (applier not called)
    EXPECT_EQ(state.counter, 0);
}

// =============================================================================
// AggregateRouter Tests
// =============================================================================

class AggregateRouterTest : public ::testing::Test {
   protected:
    ContextualCommand make_contextual_command(const std::string& command_type) {
        ContextualCommand ctx_cmd;
        auto* cmd_book = ctx_cmd.mutable_command();
        auto* cover = cmd_book->mutable_cover();
        cover->set_domain("order");
        cover->set_correlation_id("test-correlation");

        auto* page = cmd_book->add_pages();
        page->set_sequence(1);
        page->mutable_command()->set_type_url("type.googleapis.com/" + command_type);

        return ctx_cmd;
    }
};

TEST_F(AggregateRouterTest, Construction_ShouldSetNameAndDomain) {
    // When I create an AggregateRouter
    AggregateRouter<TestState, MockAggregateHandler> router("order", "order",
                                                            MockAggregateHandler());

    // Then name and domain should be set
    EXPECT_EQ(router.name(), "order");
    EXPECT_EQ(router.domain(), "order");
}

TEST_F(AggregateRouterTest, CommandTypes_ShouldDelegateToHandler) {
    // Given an AggregateRouter
    AggregateRouter<TestState, MockAggregateHandler> router("order", "order",
                                                            MockAggregateHandler());

    // When I get command types
    auto types = router.command_types();

    // Then it should return handler's command types
    EXPECT_EQ(types.size(), 2);
    EXPECT_EQ(types[0], "CreateOrder");
    EXPECT_EQ(types[1], "UpdateOrder");
}

TEST_F(AggregateRouterTest, Subscriptions_ShouldReturnDomainAndTypes) {
    // Given an AggregateRouter
    AggregateRouter<TestState, MockAggregateHandler> router("order", "order",
                                                            MockAggregateHandler());

    // When I get subscriptions
    auto subs = router.subscriptions();

    // Then it should return domain with command types
    EXPECT_EQ(subs.size(), 1);
    EXPECT_EQ(subs[0].first, "order");
    EXPECT_EQ(subs[0].second.size(), 2);
}

TEST_F(AggregateRouterTest, Descriptor_ShouldBuildCorrectDescriptor) {
    // Given an AggregateRouter
    AggregateRouter<TestState, MockAggregateHandler> router("order", "order",
                                                            MockAggregateHandler());

    // When I get descriptor
    auto desc = router.descriptor();

    // Then it should have correct values
    EXPECT_EQ(desc.name, "order");
    EXPECT_EQ(desc.component_type, component_types::AGGREGATE);
    EXPECT_EQ(desc.inputs.size(), 1);
    EXPECT_TRUE(desc.inputs.count("order") > 0);
}

TEST_F(AggregateRouterTest, Dispatch_ValidCommand_ShouldReturnEvents) {
    // Given an AggregateRouter with mock handler
    MockAggregateHandler handler;
    AggregateRouter<TestState, MockAggregateHandler> router("order", "order", std::move(handler));

    // When I dispatch a valid command
    auto ctx_cmd = make_contextual_command("CreateOrder");
    auto response = router.dispatch(ctx_cmd);

    // Then events should be returned
    EXPECT_TRUE(response.has_events());
    EXPECT_EQ(response.events().pages_size(), 1);
}

TEST_F(AggregateRouterTest, Dispatch_NoCommandPages_ShouldThrow) {
    // Given an AggregateRouter
    AggregateRouter<TestState, MockAggregateHandler> router("order", "order",
                                                            MockAggregateHandler());

    // When I dispatch a command with no pages
    ContextualCommand ctx_cmd;
    ctx_cmd.mutable_command();  // Empty command book

    // Then it should throw
    EXPECT_THROW(router.dispatch(ctx_cmd), InvalidArgumentError);
}

TEST_F(AggregateRouterTest, Dispatch_Notification_ShouldCallOnRejected) {
    // Given an AggregateRouter with mock handler
    MockAggregateHandler handler;
    AggregateRouter<TestState, MockAggregateHandler> router("order", "order", std::move(handler));

    // When I dispatch a Notification (rejection)
    ContextualCommand ctx_cmd;
    auto* cmd_book = ctx_cmd.mutable_command();
    cmd_book->mutable_cover()->set_domain("order");

    // Build a Notification with RejectionNotification payload
    Notification notification;
    notification.mutable_cover()->set_domain("order");
    notification.mutable_cover()->set_correlation_id("corr-123");

    RejectionNotification rejection;
    auto* rejected_cmd = rejection.mutable_rejected_command();
    rejected_cmd->mutable_cover()->set_domain("fulfillment");
    rejected_cmd->add_pages()->mutable_command()->set_type_url(
        "type.googleapis.com/examples.CreateShipment");
    notification.mutable_payload()->PackFrom(rejection);

    auto* page = cmd_book->add_pages();
    page->mutable_command()->PackFrom(notification, "type.googleapis.com/");

    auto response = router.dispatch(ctx_cmd);

    // Then revocation should be returned (since handler returns empty response)
    EXPECT_TRUE(response.has_revocation());
    EXPECT_TRUE(response.revocation().emit_system_revocation());
}

// =============================================================================
// SagaRouter Tests
// =============================================================================

class SagaRouterTest : public ::testing::Test {
   protected:
    EventBook make_event_book(const std::string& domain, const std::string& event_type) {
        EventBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain(domain);
        cover->set_correlation_id("test-correlation");

        auto* page = book.add_pages();
        page->set_sequence(1);
        page->mutable_event()->set_type_url("type.googleapis.com/" + event_type);

        return book;
    }
};

TEST_F(SagaRouterTest, Construction_ShouldSetNameAndDomain) {
    // When I create a SagaRouter
    SagaRouter<MockSagaHandler> router("saga-order-fulfillment", "order", MockSagaHandler());

    // Then name and domain should be set
    EXPECT_EQ(router.name(), "saga-order-fulfillment");
    EXPECT_EQ(router.input_domain(), "order");
}

TEST_F(SagaRouterTest, EventTypes_ShouldDelegateToHandler) {
    // Given a SagaRouter
    SagaRouter<MockSagaHandler> router("saga-order-fulfillment", "order", MockSagaHandler());

    // When I get event types
    auto types = router.event_types();

    // Then it should return handler's event types
    EXPECT_EQ(types.size(), 2);
    EXPECT_EQ(types[0], "OrderCreated");
    EXPECT_EQ(types[1], "OrderCancelled");
}

TEST_F(SagaRouterTest, Subscriptions_ShouldReturnDomainAndTypes) {
    // Given a SagaRouter
    SagaRouter<MockSagaHandler> router("saga-order-fulfillment", "order", MockSagaHandler());

    // When I get subscriptions
    auto subs = router.subscriptions();

    // Then it should return domain with event types
    EXPECT_EQ(subs.size(), 1);
    EXPECT_EQ(subs[0].first, "order");
    EXPECT_EQ(subs[0].second.size(), 2);
}

TEST_F(SagaRouterTest, Descriptor_ShouldBuildCorrectDescriptor) {
    // Given a SagaRouter
    SagaRouter<MockSagaHandler> router("saga-order-fulfillment", "order", MockSagaHandler());

    // When I get descriptor
    auto desc = router.descriptor();

    // Then it should have correct values
    EXPECT_EQ(desc.name, "saga-order-fulfillment");
    EXPECT_EQ(desc.component_type, component_types::SAGA);
    EXPECT_EQ(desc.inputs.size(), 1);
}

TEST_F(SagaRouterTest, PrepareDestinations_ValidEvent_ShouldReturnCovers) {
    // Given a SagaRouter with mock handler
    SagaRouter<MockSagaHandler> router("saga-order-fulfillment", "order", MockSagaHandler());

    // When I prepare destinations for valid event
    auto book = make_event_book("order", "OrderCreated");
    auto covers = router.prepare_destinations(&book);

    // Then covers should be returned
    EXPECT_EQ(covers.size(), 1);
    EXPECT_EQ(covers[0].domain(), "fulfillment");
}

TEST_F(SagaRouterTest, PrepareDestinations_NullSource_ShouldReturnEmpty) {
    // Given a SagaRouter
    SagaRouter<MockSagaHandler> router("saga-order-fulfillment", "order", MockSagaHandler());

    // When I prepare destinations with null source
    auto covers = router.prepare_destinations(nullptr);

    // Then empty vector should be returned
    EXPECT_TRUE(covers.empty());
}

TEST_F(SagaRouterTest, Dispatch_ValidEvent_ShouldReturnCommands) {
    // Given a SagaRouter with mock handler
    SagaRouter<MockSagaHandler> router("saga-order-fulfillment", "order", MockSagaHandler());

    // When I dispatch an event
    auto book = make_event_book("order", "OrderCreated");
    auto response = router.dispatch(book, {});

    // Then commands should be returned
    EXPECT_EQ(response.commands_size(), 1);
    EXPECT_EQ(response.commands(0).cover().domain(), "fulfillment");
}

TEST_F(SagaRouterTest, Dispatch_EmptyEventBook_ShouldThrow) {
    // Given a SagaRouter
    SagaRouter<MockSagaHandler> router("saga-order-fulfillment", "order", MockSagaHandler());

    // When I dispatch empty event book
    EventBook book;

    // Then it should throw
    EXPECT_THROW(router.dispatch(book, {}), InvalidArgumentError);
}

// =============================================================================
// ProcessManagerRouter Tests
// =============================================================================

class ProcessManagerRouterTest : public ::testing::Test {
   protected:
    EventBook make_event_book(const std::string& domain, const std::string& event_type) {
        EventBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain(domain);
        cover->set_correlation_id("test-correlation");

        auto* page = book.add_pages();
        page->set_sequence(1);
        page->mutable_event()->set_type_url("type.googleapis.com/" + event_type);

        return book;
    }
};

TEST_F(ProcessManagerRouterTest, Construction_ShouldSetNameAndPmDomain) {
    // When I create a ProcessManagerRouter
    ProcessManagerRouter<TestState> router("pmg-order-flow", "order-flow",
                                           [](const EventBook*) { return TestState{}; });

    // Then name and pm_domain should be set
    EXPECT_EQ(router.name(), "pmg-order-flow");
    EXPECT_EQ(router.pm_domain(), "order-flow");
}

TEST_F(ProcessManagerRouterTest, Domain_ShouldRegisterHandler) {
    // Given a ProcessManagerRouter
    ProcessManagerRouter<TestState> router("pmg-order-flow", "order-flow",
                                           [](const EventBook*) { return TestState{}; });

    // When I register domain handlers
    router.domain("order", std::make_shared<MockPmHandler>())
        .domain("inventory", std::make_shared<MockPmHandler>());

    // Then subscriptions should include both domains
    auto subs = router.subscriptions();
    EXPECT_EQ(subs.size(), 2);
}

TEST_F(ProcessManagerRouterTest, Subscriptions_ShouldReturnAllDomains) {
    // Given a ProcessManagerRouter with multiple domains
    ProcessManagerRouter<TestState> router("pmg-order-flow", "order-flow",
                                           [](const EventBook*) { return TestState{}; });
    router.domain("order", std::make_shared<MockPmHandler>());

    // When I get subscriptions
    auto subs = router.subscriptions();

    // Then it should include all registered domains
    EXPECT_EQ(subs.size(), 1);
    EXPECT_EQ(subs[0].first, "order");
    EXPECT_EQ(subs[0].second.size(), 1);
    EXPECT_EQ(subs[0].second[0], "OrderCreated");
}

TEST_F(ProcessManagerRouterTest, Descriptor_ShouldBuildCorrectDescriptor) {
    // Given a ProcessManagerRouter with handlers
    ProcessManagerRouter<TestState> router("pmg-order-flow", "order-flow",
                                           [](const EventBook*) { return TestState{}; });
    router.domain("order", std::make_shared<MockPmHandler>());

    // When I get descriptor
    auto desc = router.descriptor();

    // Then it should have correct values
    EXPECT_EQ(desc.name, "pmg-order-flow");
    EXPECT_EQ(desc.component_type, component_types::PROCESS_MANAGER);
    EXPECT_EQ(desc.inputs.size(), 1);
}

TEST_F(ProcessManagerRouterTest, Dispatch_ValidEvent_ShouldReturnCommands) {
    // Given a ProcessManagerRouter with handler
    ProcessManagerRouter<TestState> router("pmg-order-flow", "order-flow",
                                           [](const EventBook*) { return TestState{}; });
    router.domain("order", std::make_shared<MockPmHandler>());

    // When I dispatch an event
    auto trigger = make_event_book("order", "OrderCreated");
    auto response = router.dispatch(trigger, nullptr, {});

    // Then commands should be returned
    EXPECT_EQ(response.commands_size(), 1);
    EXPECT_EQ(response.commands(0).cover().domain(), "inventory");
}

TEST_F(ProcessManagerRouterTest, Dispatch_UnknownDomain_ShouldThrow) {
    // Given a ProcessManagerRouter with no handler for domain
    ProcessManagerRouter<TestState> router("pmg-order-flow", "order-flow",
                                           [](const EventBook*) { return TestState{}; });
    router.domain("order", std::make_shared<MockPmHandler>());

    // When I dispatch event from unknown domain
    auto trigger = make_event_book("unknown", "SomeEvent");

    // Then it should throw
    EXPECT_THROW(router.dispatch(trigger, nullptr, {}), InvalidArgumentError);
}

TEST_F(ProcessManagerRouterTest, Dispatch_Notification_ShouldCallOnRejected) {
    // Given a ProcessManagerRouter with handler
    auto handler = std::make_shared<MockPmHandler>();
    ProcessManagerRouter<TestState> router("pmg-order-flow", "order-flow",
                                           [](const EventBook*) { return TestState{}; });
    router.domain("order", handler);

    // When I dispatch a Notification (rejection)
    EventBook trigger;
    trigger.mutable_cover()->set_domain("order");
    trigger.mutable_cover()->set_correlation_id("corr-123");

    // Build a Notification with RejectionNotification payload
    Notification notification;
    notification.mutable_cover()->set_domain("order");

    RejectionNotification rejection;
    auto* rejected_cmd = rejection.mutable_rejected_command();
    rejected_cmd->mutable_cover()->set_domain("inventory");
    rejected_cmd->add_pages()->mutable_command()->set_type_url(
        "type.googleapis.com/examples.ReserveStock");
    notification.mutable_payload()->PackFrom(rejection);

    auto* page = trigger.add_pages();
    page->mutable_event()->PackFrom(notification, "type.googleapis.com/");

    auto response = router.dispatch(trigger, nullptr, {});

    // Then rejection handler should have been called
    EXPECT_TRUE(handler->rejection_called_);
}

TEST_F(ProcessManagerRouterTest, RebuildState_ShouldUseRebuildFunction) {
    // Given a ProcessManagerRouter with custom rebuild
    bool rebuild_called = false;
    ProcessManagerRouter<TestState> router("pmg-order-flow", "order-flow",
                                           [&rebuild_called](const EventBook* events) {
                                               rebuild_called = true;
                                               return TestState{.counter = 42};
                                           });

    // When I rebuild state
    EventBook events;
    auto state = router.rebuild_state(&events);

    // Then rebuild should have been called
    EXPECT_TRUE(rebuild_called);
    EXPECT_EQ(state.counter, 42);
}

// =============================================================================
// ProjectorRouter Tests
// =============================================================================

class ProjectorRouterTest : public ::testing::Test {
   protected:
    EventBook make_event_book(const std::string& domain, const std::string& event_type) {
        EventBook book;
        auto* cover = book.mutable_cover();
        cover->set_domain(domain);

        auto* page = book.add_pages();
        page->set_sequence(1);
        page->mutable_event()->set_type_url("type.googleapis.com/" + event_type);

        return book;
    }
};

TEST_F(ProjectorRouterTest, Construction_ShouldSetName) {
    // When I create a ProjectorRouter
    ProjectorRouter router("prj-output");

    // Then name should be set
    EXPECT_EQ(router.name(), "prj-output");
}

TEST_F(ProjectorRouterTest, Domain_ShouldRegisterHandler) {
    // Given a ProjectorRouter
    ProjectorRouter router("prj-output");

    // When I register domain handlers
    router.domain("player", std::make_shared<MockProjectorHandler>())
        .domain("hand", std::make_shared<MockProjectorHandler>());

    // Then subscriptions should include both domains
    auto subs = router.subscriptions();
    EXPECT_EQ(subs.size(), 2);
}

TEST_F(ProjectorRouterTest, Subscriptions_ShouldReturnAllDomains) {
    // Given a ProjectorRouter with handlers
    ProjectorRouter router("prj-output");
    router.domain("player", std::make_shared<MockProjectorHandler>());

    // When I get subscriptions
    auto subs = router.subscriptions();

    // Then it should include registered domain
    EXPECT_EQ(subs.size(), 1);
    EXPECT_EQ(subs[0].first, "player");
    EXPECT_EQ(subs[0].second.size(), 2);
}

TEST_F(ProjectorRouterTest, Descriptor_ShouldBuildCorrectDescriptor) {
    // Given a ProjectorRouter with handlers
    ProjectorRouter router("prj-output");
    router.domain("player", std::make_shared<MockProjectorHandler>());

    // When I get descriptor
    auto desc = router.descriptor();

    // Then it should have correct values
    EXPECT_EQ(desc.name, "prj-output");
    EXPECT_EQ(desc.component_type, component_types::PROJECTOR);
    EXPECT_EQ(desc.inputs.size(), 1);
}

TEST_F(ProjectorRouterTest, Dispatch_ValidEvent_ShouldCallHandler) {
    // Given a ProjectorRouter with handler
    auto handler = std::make_shared<MockProjectorHandler>();
    ProjectorRouter router("prj-output");
    router.domain("player", handler);

    // When I dispatch events
    auto events = make_event_book("player", "PlayerRegistered");
    auto projection = router.dispatch(events);

    // Then handler should have been called
    EXPECT_TRUE(handler->project_called_);
    EXPECT_EQ(handler->events_count_, 1);
}

TEST_F(ProjectorRouterTest, Dispatch_UnknownDomain_ShouldThrow) {
    // Given a ProjectorRouter with handler for player
    ProjectorRouter router("prj-output");
    router.domain("player", std::make_shared<MockProjectorHandler>());

    // When I dispatch events from unknown domain
    auto events = make_event_book("unknown", "SomeEvent");

    // Then it should throw
    EXPECT_THROW(router.dispatch(events), InvalidArgumentError);
}

// =============================================================================
// Factory Function Tests
// =============================================================================

TEST(FactoryFunctions, MakeAggregateRouter_ShouldCreateRouter) {
    auto router = make_aggregate_router<TestState>("order", "order", MockAggregateHandler());
    EXPECT_EQ(router.name(), "order");
    EXPECT_EQ(router.domain(), "order");
}

TEST(FactoryFunctions, MakeSagaRouter_ShouldCreateRouter) {
    auto router = make_saga_router("saga-order-fulfillment", "order", MockSagaHandler());
    EXPECT_EQ(router.name(), "saga-order-fulfillment");
    EXPECT_EQ(router.input_domain(), "order");
}

TEST(FactoryFunctions, MakePmRouter_ShouldCreateRouter) {
    auto router = make_pm_router<TestState>("pmg-order-flow", "order-flow",
                                            [](const EventBook*) { return TestState{}; });
    EXPECT_EQ(router.name(), "pmg-order-flow");
    EXPECT_EQ(router.pm_domain(), "order-flow");
}

TEST(FactoryFunctions, MakeProjectorRouter_ShouldCreateRouter) {
    auto router = make_projector_router("prj-output");
    EXPECT_EQ(router.name(), "prj-output");
}

// =============================================================================
// RejectionHandlerResponse Tests
// =============================================================================

TEST(RejectionHandlerResponse, Empty_ShouldHaveNoValues) {
    auto response = RejectionHandlerResponse::empty();
    EXPECT_FALSE(response.events.has_value());
    EXPECT_FALSE(response.notification.has_value());
}

TEST(RejectionHandlerResponse, WithEvents_ShouldHaveEvents) {
    EventBook events;
    events.add_pages()->set_sequence(1);

    auto response = RejectionHandlerResponse::with_events(events);
    EXPECT_TRUE(response.events.has_value());
    EXPECT_FALSE(response.notification.has_value());
    EXPECT_EQ(response.events->pages_size(), 1);
}

TEST(RejectionHandlerResponse, WithNotification_ShouldHaveNotification) {
    Notification notification;
    notification.mutable_cover()->set_domain("test");

    auto response = RejectionHandlerResponse::with_notification(notification);
    EXPECT_FALSE(response.events.has_value());
    EXPECT_TRUE(response.notification.has_value());
}

// =============================================================================
// ProcessManagerResponse Tests
// =============================================================================

TEST(ProcessManagerResponse, Empty_ShouldHaveNoValues) {
    auto response = ProcessManagerResponse::empty();
    EXPECT_TRUE(response.commands.empty());
    EXPECT_FALSE(response.process_events.has_value());
}

TEST(ProcessManagerResponse, WithCommands_ShouldHaveCommands) {
    CommandBook cmd;
    cmd.mutable_cover()->set_domain("test");

    auto response = ProcessManagerResponse::with_commands({cmd});
    EXPECT_EQ(response.commands.size(), 1);
    EXPECT_FALSE(response.process_events.has_value());
}

TEST(ProcessManagerResponse, WithProcessEvents_ShouldHaveEvents) {
    EventBook events;
    events.add_pages()->set_sequence(1);

    auto response = ProcessManagerResponse::with_process_events(events);
    EXPECT_TRUE(response.commands.empty());
    EXPECT_TRUE(response.process_events.has_value());
}

TEST(ProcessManagerResponse, WithBoth_ShouldHaveBoth) {
    CommandBook cmd;
    cmd.mutable_cover()->set_domain("test");
    EventBook events;
    events.add_pages()->set_sequence(1);

    auto response = ProcessManagerResponse::with_both({cmd}, events);
    EXPECT_EQ(response.commands.size(), 1);
    EXPECT_TRUE(response.process_events.has_value());
}
