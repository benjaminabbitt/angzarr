#include <gtest/gtest.h>
#include <string>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/aggregate.pb.h"
#include "angzarr/compensation.hpp"
#include "angzarr/router.hpp"

using namespace angzarr;

// =============================================================================
// CompensationContext Tests
// =============================================================================

struct CompensationTestState {
    bool compensated = false;
};

class CompensationContextTest : public ::testing::Test {
protected:
    Notification make_rejection_notification(const std::string& domain,
                                              const std::string& command_type,
                                              const std::string& reason) {
        // Create rejected command
        CommandBook rejected_cmd;
        auto* cover = rejected_cmd.mutable_cover();
        cover->set_domain(domain);
        cover->set_correlation_id("test-correlation");
        auto* page = rejected_cmd.add_pages();
        page->mutable_command()->set_type_url("type.googleapis.com/" + command_type);

        // Create rejection notification
        RejectionNotification rejection;
        rejection.set_rejection_reason(reason);
        *rejection.mutable_rejected_command() = rejected_cmd;
        rejection.set_issuer_name("saga-test");
        rejection.set_issuer_type("saga");

        // Wrap in notification
        Notification notification;
        notification.mutable_payload()->PackFrom(rejection);
        return notification;
    }
};

TEST_F(CompensationContextTest, FromNotification_ShouldExtractRejectedCommand) {
    // Given a rejection notification
    auto notification = make_rejection_notification("inventory", "ReserveStock", "out of stock");

    // When I create CompensationContext
    auto ctx = CompensationContext::from_notification(notification);

    // Then it should contain the rejected command info
    EXPECT_TRUE(ctx.rejected_command().has_value());
    EXPECT_EQ(ctx.rejected_command()->cover().domain(), "inventory");
    EXPECT_TRUE(ctx.rejected_command_type().find("ReserveStock") != std::string::npos);
}

TEST_F(CompensationContextTest, FromNotification_ShouldExtractRejectionReason) {
    // Given a rejection notification with reason
    auto notification = make_rejection_notification("inventory", "ReserveStock", "out of stock");

    // When I create CompensationContext
    auto ctx = CompensationContext::from_notification(notification);

    // Then it should contain the rejection reason
    EXPECT_EQ(ctx.rejection_reason(), "out of stock");
}

TEST_F(CompensationContextTest, FromNotification_ShouldExtractIssuerInfo) {
    // Given a rejection notification with issuer info
    auto notification = make_rejection_notification("inventory", "ReserveStock", "out of stock");

    // When I create CompensationContext
    auto ctx = CompensationContext::from_notification(notification);

    // Then it should contain issuer info
    EXPECT_EQ(ctx.issuer_name(), "saga-test");
    EXPECT_EQ(ctx.issuer_type(), "saga");
}

// =============================================================================
// Rejection Handler Tests
// =============================================================================

class RejectionHandlerTest : public ::testing::Test {
protected:
    ContextualCommand wrap_notification_in_command(const Notification& notification) {
        ContextualCommand ctx_cmd;
        auto* cmd_book = ctx_cmd.mutable_command();
        cmd_book->mutable_cover()->set_domain("test");
        auto* page = cmd_book->add_pages();
        page->mutable_command()->PackFrom(notification);
        return ctx_cmd;
    }

    Notification make_rejection_notification(const std::string& domain,
                                              const std::string& command_type) {
        CommandBook rejected_cmd;
        rejected_cmd.mutable_cover()->set_domain(domain);
        auto* page = rejected_cmd.add_pages();
        page->mutable_command()->set_type_url("type.googleapis.com/" + command_type);

        RejectionNotification rejection;
        rejection.set_rejection_reason("test reason");
        *rejection.mutable_rejected_command() = rejected_cmd;

        Notification notification;
        notification.mutable_payload()->PackFrom(rejection);
        return notification;
    }
};

TEST_F(RejectionHandlerTest, OnRejected_MatchingHandler_ShouldBeInvoked) {
    // Given a CommandRouter with rejection handler
    bool handler_called = false;
    auto rebuild = [](const EventBook*) { return CompensationTestState{}; };
    CommandRouter<CompensationTestState> router("test", rebuild);
    router.on_rejected("inventory", "ReserveStock",
                       [&](const Notification&, CompensationTestState&) {
        handler_called = true;
        return RejectionHandlerResponse{};
    });

    // When I dispatch a matching rejection
    auto notification = make_rejection_notification("inventory", "ReserveStock");
    auto ctx_cmd = wrap_notification_in_command(notification);
    router.dispatch(ctx_cmd);

    // Then handler should be called
    EXPECT_TRUE(handler_called);
}

TEST_F(RejectionHandlerTest, OnRejected_ReturnEvents_ShouldEmitCompensation) {
    // Given a CommandRouter with rejection handler that returns events
    auto rebuild = [](const EventBook*) { return CompensationTestState{}; };
    CommandRouter<CompensationTestState> router("test", rebuild);
    router.on_rejected("inventory", "ReserveStock",
                       [](const Notification&, CompensationTestState&) {
        RejectionHandlerResponse response;
        EventBook events;
        auto* page = events.add_pages();
        page->mutable_event()->set_type_url("type.googleapis.com/StockReleased");
        response.events = std::move(events);
        return response;
    });

    // When I dispatch a rejection
    auto notification = make_rejection_notification("inventory", "ReserveStock");
    auto ctx_cmd = wrap_notification_in_command(notification);
    auto response = router.dispatch(ctx_cmd);

    // Then compensation events should be emitted
    EXPECT_TRUE(response.has_events());
    EXPECT_EQ(response.events().pages_size(), 1);
}

TEST_F(RejectionHandlerTest, OnRejected_ReturnNotification_ShouldForward) {
    // Given a CommandRouter with rejection handler that forwards
    auto rebuild = [](const EventBook*) { return CompensationTestState{}; };
    CommandRouter<CompensationTestState> router("test", rebuild);
    router.on_rejected("inventory", "ReserveStock",
                       [](const Notification& notification, CompensationTestState&) {
        RejectionHandlerResponse response;
        response.notification = notification;
        return response;
    });

    // When I dispatch a rejection
    auto notification = make_rejection_notification("inventory", "ReserveStock");
    auto ctx_cmd = wrap_notification_in_command(notification);
    auto response = router.dispatch(ctx_cmd);

    // Then notification should be forwarded
    EXPECT_TRUE(response.has_notification());
}
