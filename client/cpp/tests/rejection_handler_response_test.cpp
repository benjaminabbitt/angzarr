#include <gtest/gtest.h>
#include <string>
#include <optional>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/router.hpp"

using namespace angzarr;

// =============================================================================
// RejectionHandlerResponse Tests
// =============================================================================

class RejectionHandlerResponseTest : public ::testing::Test {
protected:
    EventBook makeEventBook() {
        EventBook book;
        auto* page = book.add_pages();
        page->mutable_event()->set_type_url("type.googleapis.com/test.TestEvent");
        return book;
    }

    Notification makeNotification(const std::string& domain,
                                  const std::string& commandType,
                                  const std::string& reason) {
        RejectionNotification rejection;
        rejection.set_issuer_name("test-saga");
        rejection.set_issuer_type("saga");
        rejection.set_rejection_reason(reason);

        auto* rejected_cmd = rejection.mutable_rejected_command();
        rejected_cmd->mutable_cover()->set_domain(domain);
        auto* cmd_page = rejected_cmd->add_pages();
        cmd_page->mutable_command()->set_type_url("type.googleapis.com/test." + commandType);

        Notification notification;
        notification.mutable_payload()->PackFrom(rejection);
        return notification;
    }
};

TEST_F(RejectionHandlerResponseTest, EmptyResponse_HasNoEventsOrNotification) {
    RejectionHandlerResponse response;

    EXPECT_FALSE(response.events.has_value());
    EXPECT_FALSE(response.notification.has_value());
}

TEST_F(RejectionHandlerResponseTest, ResponseWithEventsOnly) {
    EventBook eventBook = makeEventBook();

    RejectionHandlerResponse response;
    response.events = eventBook;

    EXPECT_TRUE(response.events.has_value());
    EXPECT_EQ(response.events->pages_size(), 1);
    EXPECT_FALSE(response.notification.has_value());
}

TEST_F(RejectionHandlerResponseTest, ResponseWithNotificationOnly) {
    Notification notification = makeNotification("inventory", "ReserveStock", "out of stock");

    RejectionHandlerResponse response;
    response.notification = notification;

    EXPECT_FALSE(response.events.has_value());
    EXPECT_TRUE(response.notification.has_value());
}

TEST_F(RejectionHandlerResponseTest, ResponseWithBothEventsAndNotification) {
    EventBook eventBook = makeEventBook();
    Notification notification = makeNotification("payment", "ProcessPayment", "declined");

    RejectionHandlerResponse response;
    response.events = eventBook;
    response.notification = notification;

    EXPECT_TRUE(response.events.has_value());
    EXPECT_TRUE(response.notification.has_value());
}

TEST_F(RejectionHandlerResponseTest, ResponseEventsAreAccessible) {
    EventBook eventBook;
    auto* page1 = eventBook.add_pages();
    page1->mutable_event()->set_type_url("type.googleapis.com/test.Event1");
    auto* page2 = eventBook.add_pages();
    page2->mutable_event()->set_type_url("type.googleapis.com/test.Event2");

    RejectionHandlerResponse response;
    response.events = eventBook;

    EXPECT_EQ(response.events->pages_size(), 2);
}

// =============================================================================
// CommandRouter OnRejected Tests
// =============================================================================

struct TestState {
    int value = 0;
};

class CommandRouterRejectionTest : public ::testing::Test {
protected:
    TestState rebuildState(const EventBook* events) {
        TestState state;
        if (events) {
            state.value = events->pages_size();
        }
        return state;
    }

    Notification makeNotification(const std::string& domain,
                                  const std::string& commandType,
                                  const std::string& reason) {
        RejectionNotification rejection;
        rejection.set_issuer_name("test-saga");
        rejection.set_issuer_type("saga");
        rejection.set_rejection_reason(reason);

        auto* rejected_cmd = rejection.mutable_rejected_command();
        rejected_cmd->mutable_cover()->set_domain(domain);
        auto* cmd_page = rejected_cmd->add_pages();
        cmd_page->mutable_command()->set_type_url("type.googleapis.com/test." + commandType);

        Notification notification;
        notification.mutable_payload()->PackFrom(rejection);
        return notification;
    }

    ContextualCommand wrapNotification(const Notification& notification) {
        ContextualCommand cmd;
        auto* page = cmd.mutable_command()->add_pages();
        page->mutable_command()->PackFrom(notification);
        return cmd;
    }
};

TEST_F(CommandRouterRejectionTest, OnRejected_ReturnsEvents) {
    CommandRouter<TestState> router("test", [this](const EventBook* events) {
        return rebuildState(events);
    });

    router.on_rejected("inventory", "ReserveStock",
        [](const Notification& notification, TestState& state) {
            RejectionHandlerResponse response;
            EventBook events;
            auto* page = events.add_pages();
            page->mutable_event()->set_type_url("type.googleapis.com/test.Compensated");
            response.events = events;
            return response;
        });

    Notification notification = makeNotification("inventory", "ReserveStock", "out of stock");
    ContextualCommand cmd = wrapNotification(notification);

    BusinessResponse response = router.dispatch(cmd);

    EXPECT_TRUE(response.has_events());
    EXPECT_EQ(response.events().pages_size(), 1);
}

TEST_F(CommandRouterRejectionTest, OnRejected_ReturnsNotification) {
    CommandRouter<TestState> router("test", [this](const EventBook* events) {
        return rebuildState(events);
    });

    router.on_rejected("payment", "Charge",
        [](const Notification& notification, TestState& state) {
            RejectionHandlerResponse response;
            response.notification = notification;
            return response;
        });

    Notification notification = makeNotification("payment", "Charge", "declined");
    ContextualCommand cmd = wrapNotification(notification);

    BusinessResponse response = router.dispatch(cmd);

    EXPECT_TRUE(response.has_notification());
}

TEST_F(CommandRouterRejectionTest, OnRejected_NoHandler_DelegatesToFramework) {
    CommandRouter<TestState> router("test", [this](const EventBook* events) {
        return rebuildState(events);
    });
    // No rejection handler registered

    Notification notification = makeNotification("unknown", "UnknownCommand", "reason");
    ContextualCommand cmd = wrapNotification(notification);

    BusinessResponse response = router.dispatch(cmd);

    EXPECT_TRUE(response.has_revocation());
    EXPECT_TRUE(response.revocation().emit_system_revocation());
}

// =============================================================================
// ProcessManager OnRejected Tests
// =============================================================================

struct PMTestState {
    std::string phase = "initial";
};

class PMTestFixture : public ProcessManager<PMTestState> {
public:
    PMTestFixture() = default;

    std::string name() const override { return "test-pm"; }

protected:
    PMTestState create_empty_state() override {
        return PMTestState{};
    }
};

class ProcessManagerRejectionTest : public ::testing::Test {
protected:
    EventBook wrapNotificationAsEvent(const Notification& notification) {
        EventBook book;
        book.mutable_cover()->set_correlation_id("test-correlation");
        auto* page = book.add_pages();
        page->mutable_event()->PackFrom(notification);
        return book;
    }

    Notification makeNotification(const std::string& domain,
                                  const std::string& commandType,
                                  const std::string& reason) {
        RejectionNotification rejection;
        rejection.set_issuer_name("test-saga");
        rejection.set_issuer_type("saga");
        rejection.set_rejection_reason(reason);

        auto* rejected_cmd = rejection.mutable_rejected_command();
        rejected_cmd->mutable_cover()->set_domain(domain);
        auto* cmd_page = rejected_cmd->add_pages();
        cmd_page->mutable_command()->set_type_url("type.googleapis.com/test." + commandType);

        Notification notification;
        notification.mutable_payload()->PackFrom(rejection);
        return notification;
    }
};

// Note: Full PM rejection tests would require setting up the macro-based
// handler registration which is more complex. These tests focus on
// RejectionHandlerResponse type behavior.

int main(int argc, char** argv) {
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
